use super::{Api, api::checked};
use crate::encryption::Binding;
use nm_protocol::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};
use uuid::Uuid;
use base64::{Engine, engine::general_purpose::STANDARD};

#[derive(Serialize, Deserialize)]
struct Content {
    body: String,
    style: MessageStyle,
    #[serde(default)]
    attachment: Option<crate::encryption::ProtectedAttachment>,
}

#[derive(Serialize, Deserialize)]
struct CloudHistoryBackup { revision: i64, ciphertext: String }

impl Api {
    /// Merge the server's encrypted archive before publishing this device's key.
    /// Conflicts are retried by reading and merging the newer archive; the
    /// server never receives the OPAQUE export key or plaintext key material.
    #[allow(dead_code)]
    pub async fn sync_cloud_history(&self, export_key: &[u8]) -> Result<(), String> {
        self.sync_cloud_history_with_previous(export_key, None).await
    }

    pub async fn sync_cloud_history_with_previous(&self, export_key: &[u8], previous_key: Option<&[u8]>) -> Result<(), String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia não iniciada")?;
        for _ in 0..5 {
            let response = checked(self.request(reqwest::Method::GET, "/encryption/history-backup")
                .send().await.map_err(|error| error.to_string())?).await?;
            let revision = if response.status() == reqwest::StatusCode::NO_CONTENT {
                0
            } else {
                let backup: CloudHistoryBackup = super::api::bounded_json(response).await?;
                if backup.revision <= 0 || backup.ciphertext.len() > 131072 { return Err("Backup de histórico inválido".into()); }
                let bytes = STANDARD.decode(&backup.ciphertext).map_err(|_| "Backup de histórico inválido")?;
                if let Err(error) = crypto.merge_cloud_history(&bytes, export_key).await {
                    if let Some(previous) = previous_key {
                        crypto.merge_cloud_history(&bytes, previous).await?;
                    } else {
                        return Err(format!("Não foi possível desbloquear a identidade local. Informe a senha anterior para recuperar o histórico. {error}"));
                    }
                }
                backup.revision
            };
            let ciphertext = STANDARD.encode(crypto.seal_cloud_history(export_key)?);
            let response = self.request(reqwest::Method::PUT, "/encryption/history-backup")
                .json(&CloudHistoryBackup { revision, ciphertext }).send().await.map_err(|error| error.to_string())?;
            if response.status() == reqwest::StatusCode::CONFLICT { continue; }
            checked(response).await?;
            return Ok(());
        }
        Err("Não foi possível sincronizar o histórico após alterações simultâneas".into())
    }

    pub(super) async fn raw_encryption_keys(
        &self,
        conversation: Uuid,
    ) -> Result<Vec<EncryptionKey>, String> {
        self.get(&format!("/conversations/{conversation}/encryption/keys"))
            .await
    }
    pub(super) async fn decode_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, String> {
        let mut value: Value = super::api::bounded_json(response).await?;
        self.open_value(&mut value).await?;
        serde_json::from_value(value).map_err(|error| error.to_string())
    }

    fn open_value<'a>(
        &'a self,
        value: &'a mut Value,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            let Some(crypto) = &self.crypto else {
                return Ok(());
            };
            if value.get("sender_id").is_some()
                && value.get("client_id").is_some()
                && value.get("conversation_id").is_some()
            {
                let mut message: Message =
                    serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
                if !message.body.starts_with(ENCRYPTED_PREFIX)
                    && !matches!(message.kind, MessageKind::System | MessageKind::Activity)
                {
                    message.kind = MessageKind::Text;
                    message.body = format!(
                        "[Histórico legado, sem autenticação ponta a ponta]\n{}",
                        message.body
                    );
                    message.attachment_id = None;
                    message.style = MessageStyle::default();
                } else if let Err(error) = self.open_message(&mut message).await {
                    message.kind = MessageKind::Text;
                    message.body = format!("[Mensagem protegida indisponível: {error}]");
                    message.style = MessageStyle::default();
                    message.attachment_id = None;
                }
                *value = serde_json::to_value(message).map_err(|error| error.to_string())?;
                return Ok(());
            }
            if value.get("members").is_some()
                && value.get("id").is_some()
                && let Ok(conversation) = serde_json::from_value::<Conversation>(value.clone())
            {
                crypto.observe_conversation(&conversation).await?;
            }
            if value.get("initiator_id").is_some()
                && let Ok(activity) = serde_json::from_value::<Activity>(value.clone())
            {
                crypto.remember_activity(&activity)?;
            }
            if value.get("filename").is_some()
                && value.get("sha256").is_some()
                && let Ok(mut info) = serde_json::from_value::<FileInfo>(value.clone())
                && let Some(meta) = crypto
                    .files
                    .lock()
                    .map_err(|_| "Estado de arquivos indisponível")?
                    .get(&info.id)
            {
                info.filename = meta.filename.clone();
                info.mime = meta.mime.clone();
                info.size = meta.size;
                *value = serde_json::to_value(info).map_err(|error| error.to_string())?;
            }
            match value {
                Value::Array(values) => {
                    for value in values {
                        self.open_value(value).await?;
                    }
                }
                Value::Object(values) => {
                    for value in values.values_mut() {
                        self.open_value(value).await?;
                    }
                }
                _ => {}
            }
            Ok(())
        })
    }

    pub(super) async fn encryption_keys(
        &self,
        conversation: Uuid,
        sending: bool,
    ) -> Result<Vec<EncryptionKey>, String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia não iniciada")?;
        let mut cache = crypto.key_cache.lock().await;
        if let Some((time, keys)) = cache.get(&conversation)
            && time.elapsed() < Duration::from_secs(5)
        {
            if sending {
                self.check_recipients(conversation, keys)?;
            }
            return Ok(keys.clone());
        }
        let response = checked(
            self.request(
                reqwest::Method::GET,
                &format!("/conversations/{conversation}/encryption/keys"),
            )
            .send()
            .await
            .map_err(|error| error.to_string())?,
        )
        .await?;
        let keys: Vec<EncryptionKey> = self.decode_response(response).await?;
        if keys.len() > 512 {
            return Err("Número de identidades excede o limite".into());
        }
        crypto.check_keys(&keys).await?;
        if sending {
            self.check_recipients(conversation, &keys)?;
        }
        cache.insert(conversation, (Instant::now(), keys.clone()));
        Ok(keys)
    }

    fn check_recipients(&self, conversation: Uuid, keys: &[EncryptionKey]) -> Result<(), String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia não iniciada")?;
        crypto.check_roster(conversation, keys, true)
    }

    pub(super) async fn seal_message(
        &self,
        conversation: Uuid,
        request: &mut SendMessageRequest,
    ) -> Result<(), String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia não iniciada")?;
        if request.body.chars().count() > MAX_MESSAGE_CHARS {
            return Err("Mensagem excede o limite".into());
        }
        let keys = self.encryption_keys(conversation, true).await?;
        let binding = Binding::message(
            conversation,
            self.user_id,
            request.client_id,
            request.kind,
            request.attachment_id,
        );
        let attachment = request
            .attachment_id
            .and_then(|id| crypto.files.lock().ok()?.get(&id).cloned());
        if request.attachment_id.is_some() && attachment.is_none() {
            return Err("Este anexo não foi criptografado. Envie o arquivo novamente pelo cliente atualizado.".into());
        }
        let content = zeroize::Zeroizing::new(
            serde_json::to_vec(&Content {
                body: request.body.clone(),
                style: request.style.clone(),
                attachment,
            })
            .map_err(|error| error.to_string())?,
        );
        let envelope = crypto.seal(&content, &binding, &keys)?;
        request.body = format!(
            "{ENCRYPTED_PREFIX}{}",
            serde_json::to_string(&envelope).map_err(|error| error.to_string())?
        );
        if request.body.len() > MAX_ENCRYPTED_BODY {
            return Err("Mensagem protegida excede o limite".into());
        }
        request.style = MessageStyle::default();
        Ok(())
    }

    pub(super) async fn open_message(&self, message: &mut Message) -> Result<(), String> {
        let Some(crypto) = &self.crypto else {
            return Ok(());
        };
        let Some(body) = message.body.strip_prefix(ENCRYPTED_PREFIX) else {
            return if matches!(message.kind, MessageKind::System | MessageKind::Activity) {
                Ok(())
            } else {
                Err("Mensagem nova sem criptografia ponta a ponta rejeitada".into())
            };
        };
        if body.len() > MAX_ENCRYPTED_BODY {
            return Err("Mensagem protegida excede o limite".into());
        }
        let envelope: EncryptedMessage =
            serde_json::from_str(body).map_err(|_| "Mensagem protegida inválida")?;
        let keys = self.encryption_keys(message.conversation_id, false).await?;
        let binding = Binding::message(
            message.conversation_id,
            message.sender_id,
            message.client_id,
            message.kind,
            message.attachment_id,
        );
        let clear = crypto.open(&envelope, &binding, &keys)?;
        let content: Content =
            serde_json::from_slice(&clear).map_err(|_| "Conteúdo de mensagem inválido")?;
        if content.body.chars().count() > MAX_MESSAGE_CHARS
            || content.body.contains('\0')
            || content.style.font_family.len() > 100
            || !content.style.font_size.is_finite()
            || !(6.0..=72.0).contains(&content.style.font_size)
        {
            return Err("Conteúdo de mensagem inválido".into());
        }
        message.body = content.body;
        message.style = content.style;
        if let Some(attachment) = content.attachment {
            if message.attachment_id != Some(attachment.file_id)
                || attachment.conversation != message.conversation_id
                || attachment.owner != message.sender_id
            {
                return Err("Anexo pertence a outra mensagem ou remetente".into());
            }
            crypto
                .files
                .lock()
                .map_err(|_| "Estado de arquivos indisponível")?
                .insert(attachment.file_id, attachment);
        }
        Ok(())
    }

    pub(super) async fn open_event(&self, mut event: ServerEvent) -> Result<ServerEvent, String> {
        match &mut event {
            ServerEvent::Media {
                activity_id,
                from_user_id,
                stream,
                sequence,
                data,
            } => {
                self.open_media(*activity_id, *from_user_id, *stream, *sequence, data)
                    .await?
            }
            ServerEvent::Signal {
                activity_id,
                from_user_id,
                data,
            } => self.open_signal(*activity_id, *from_user_id, data).await?,
            ServerEvent::Message { message } | ServerEvent::MessageUpdated { message } => self.open_message(message).await?,
            ServerEvent::Activity { activity } => {
                if let Some(crypto) = &self.crypto {
                    crypto.remember_activity(activity)?;
                }
            }
            ServerEvent::ConversationsChanged => {
                if let Some(crypto) = &self.crypto {
                    crypto.key_cache.lock().await.clear();
                }
            }
            _ => {}
        }
        Ok(event)
    }
}
