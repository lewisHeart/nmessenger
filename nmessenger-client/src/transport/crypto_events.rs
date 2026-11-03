use super::{Api, api::checked};
use crate::encryption::Binding;
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::*;
use serde_json::Value;
use std::time::{Duration, Instant};
use uuid::Uuid;

impl Api {
    async fn event_keys(&self, activity: Uuid) -> Result<Vec<EncryptionKey>, String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia indisponível")?;
        let conversation = crypto
            .activities
            .lock()
            .map_err(|_| "Estado de atividade indisponível")?
            .get(&activity)
            .ok_or("Atividade não autorizada")?
            .conversation;
        let mut cache = crypto.key_cache.lock().await;
        if let Some((time, keys)) = cache.get(&activity)
            && time.elapsed() < Duration::from_secs(5)
        {
            crypto.check_roster(conversation, keys, false)?;
            return Ok(keys.clone());
        }
        let response = checked(
            self.request(
                reqwest::Method::GET,
                &format!("/activities/{activity}/encryption/keys"),
            )
            .send()
            .await
            .map_err(|error| error.to_string())?,
        )
        .await?;
        let keys: Vec<EncryptionKey> = self.decode_response(response).await?;
        if keys.len() > 256 {
            return Err("Muitos dispositivos na atividade".into());
        }
        crypto.check_keys(&keys).await?;
        crypto.check_roster(conversation, &keys, false)?;
        cache.insert(activity, (Instant::now(), keys.clone()));
        Ok(keys)
    }

    fn event_binding(
        &self,
        activity: Uuid,
        sender: Uuid,
        stream: Option<MediaStream>,
        sequence: Option<u64>,
    ) -> Result<Binding, String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia indisponível")?;
        let channel = *crypto
            .activities
            .lock()
            .map_err(|_| "Estado de atividade indisponível")?
            .get(&activity)
            .ok_or("Atividade não autorizada neste cliente")?;
        Ok(Binding {
            purpose: if stream.is_some() { "media" } else { "signal" }.into(),
            conversation: channel.conversation,
            sender,
            object: activity,
            kind: serde_json::to_string(&(channel.kind, stream))
                .map_err(|error| error.to_string())?,
            sequence,
            attachment: None,
        })
    }

    pub(super) async fn seal_event(&self, mut event: ClientEvent) -> Result<ClientEvent, String> {
        let Some(crypto) = &self.crypto else {
            return Ok(event);
        };
        match &mut event {
            ClientEvent::Media {
                activity_id,
                stream,
                sequence,
                data,
            } => {
                let clear = zeroize::Zeroizing::new(
                    STANDARD
                        .decode(data.as_bytes())
                        .map_err(|_| "Quadro inválido")?,
                );
                if clear.len() > MAX_MEDIA_FRAME_BYTES {
                    return Err("Quadro excede o limite".into());
                }
                let keys = self.event_keys(*activity_id).await?;
                let binding =
                    self.event_binding(*activity_id, self.user_id, Some(*stream), Some(*sequence))?;
                let envelope = crypto.seal(&clear, &binding, &keys)?;
                *data = format!(
                    "{ENCRYPTED_PREFIX}{}",
                    serde_json::to_string(&envelope).map_err(|error| error.to_string())?
                );
            }
            ClientEvent::Signal { activity_id, data } => {
                let clear = zeroize::Zeroizing::new(
                    serde_json::to_vec(data).map_err(|error| error.to_string())?,
                );
                if clear.len() > 16 * 1024 {
                    return Err("Sinal excede o limite".into());
                }
                let keys = self.event_keys(*activity_id).await?;
                let binding = self.event_binding(*activity_id, self.user_id, None, None)?;
                let envelope = crypto.seal(&clear, &binding, &keys)?;
                *data = serde_json::json!({"e2ee": envelope});
            }
            _ => {}
        }
        Ok(event)
    }

    pub(super) async fn open_media(
        &self,
        activity: Uuid,
        sender: Uuid,
        stream: MediaStream,
        sequence: u64,
        data: &mut String,
    ) -> Result<(), String> {
        let Some(crypto) = &self.crypto else {
            return Ok(());
        };
        let encoded = data
            .strip_prefix(ENCRYPTED_PREFIX)
            .ok_or("Mídia sem criptografia rejeitada")?;
        if encoded.len() > 480 * 1024 {
            return Err("Quadro excede o limite".into());
        }
        let envelope = serde_json::from_str(encoded).map_err(|_| "Quadro protegido inválido")?;
        let binding = self.event_binding(activity, sender, Some(stream), Some(sequence))?;
        let keys = self.event_keys(activity).await?;
        let clear = crypto.open(&envelope, &binding, &keys)?;
        if clear.is_empty() || clear.len() > MAX_MEDIA_FRAME_BYTES {
            return Err("Quadro excede o limite".into());
        }
        match stream {
            MediaStream::Audio if clear.len() > 32_000 || clear.len() % 2 != 0 => {
                return Err("Bloco PCM protegido inválido".into());
            }
            MediaStream::Camera | MediaStream::Screen
                if clear.len() < 4
                    || !clear.starts_with(&[0xff, 0xd8, 0xff])
                    || !clear.ends_with(&[0xff, 0xd9]) =>
            {
                return Err("Quadro JPEG protegido inválido".into());
            }
            _ => {}
        }
        *data = STANDARD.encode(clear.as_slice());
        Ok(())
    }

    pub(super) async fn open_signal(
        &self,
        activity: Uuid,
        sender: Uuid,
        data: &mut Value,
    ) -> Result<(), String> {
        let Some(crypto) = &self.crypto else {
            return Ok(());
        };
        let envelope = serde_json::from_value(
            data.get("e2ee")
                .ok_or("Sinal sem criptografia rejeitado")?
                .clone(),
        )
        .map_err(|_| "Sinal protegido inválido")?;
        let binding = self.event_binding(activity, sender, None, None)?;
        let keys = self.event_keys(activity).await?;
        let clear = crypto.open(&envelope, &binding, &keys)?;
        if clear.len() > 16 * 1024 {
            return Err("Sinal excede o limite".into());
        }
        *data = serde_json::from_slice(&clear).map_err(|_| "Sinal protegido inválido")?;
        Ok(())
    }
}
