use super::Api;
use chrono::{DateTime, Utc};
use nm_protocol::SendMessageRequest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Serialize, Deserialize)]
pub struct Pending {
    pub conversation: Uuid,
    pub request: SendMessageRequest,
    pub queued_at: DateTime<Utc>,
}

fn directory(api: &Api) -> PathBuf {
    let key: String = Sha256::digest(api.base.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    crate::storage::directory()
        .join("outbox")
        .join(key)
        .join(api.user_id.to_string())
}

pub async fn store(
    api: &Api,
    conversation: Uuid,
    request: SendMessageRequest,
) -> Result<Pending, String> {
    let _lock = LOCK.lock().await;
    let dir = directory(api);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| error.to_string())?;
    let path = dir.join(format!("{}.json", request.client_id));
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return decode(api, &bytes);
    }
    let pending = Pending {
        conversation,
        request,
        queued_at: Utc::now(),
    };
    let clear =
        zeroize::Zeroizing::new(serde_json::to_vec(&pending).map_err(|error| error.to_string())?);
    let bytes = match &api.crypto {
        Some(crypto) => crypto.protect_local(&clear)?,
        None => clear.to_vec(),
    };
    crate::secure::write_private(&path, &bytes).await?;
    Ok(pending)
}

pub async fn acknowledged(api: &Api, id: Uuid) {
    let _lock = LOCK.lock().await;
    let _ = tokio::fs::remove_file(directory(api).join(format!("{id}.json"))).await;
}

pub async fn load(api: &Api) -> Result<Vec<Pending>, String> {
    let _lock = LOCK.lock().await;
    let mut entries = match tokio::fs::read_dir(directory(api)).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut messages = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        if entry
            .path()
            .extension()
            .is_some_and(|suffix| suffix == "json")
        {
            let bytes = tokio::fs::read(entry.path())
                .await
                .map_err(|error| error.to_string())?;
            if let Ok(message) = decode(api, &bytes) {
                if let Some(crypto) = &api.crypto
                    && !bytes.starts_with(b"NML1")
                {
                    let encoded = crypto.protect_local(&bytes)?;
                    crate::secure::write_private(&entry.path(), &encoded).await?;
                }
                messages.push(message);
            }
        }
    }
    messages.sort_by_key(|message| (message.queued_at, message.request.client_id));
    Ok(messages)
}

fn decode(api: &Api, bytes: &[u8]) -> Result<Pending, String> {
    let clear = if bytes.starts_with(b"NML1") {
        api.crypto
            .as_ref()
            .ok_or("Fila protegida exige a identidade local")?
            .open_local(bytes)?
    } else {
        zeroize::Zeroizing::new(bytes.to_vec())
    };
    serde_json::from_slice(&clear).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn pending_messages_recover_per_account_and_are_removed_only_on_ack() {
        let api = Api {
            base: "http://127.0.0.1:1/v1".into(),
            token: std::sync::Arc::new(zeroize::Zeroizing::new("not-written-to-disk".into())),
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            crypto: None,
            client: reqwest::Client::new(),
        };
        let conversation = Uuid::new_v4();
        let request = SendMessageRequest {
            client_id: Uuid::new_v4(),
            kind: nm_protocol::MessageKind::Text,
            body: "Olá, mensagem ainda pendente 👋".into(),
            style: Default::default(),
            attachment_id: None,
        };
        store(&api, conversation, request.clone()).await.unwrap();
        let mut changed = request.clone();
        changed.body = "This retry must not rewrite the original message".into();
        assert_eq!(
            store(&api, conversation, changed)
                .await
                .unwrap()
                .request
                .body,
            request.body
        );
        let recovered = load(&api).await.unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].request.client_id, request.client_id);
        let bytes = tokio::fs::read(directory(&api).join(format!("{}.json", request.client_id)))
            .await
            .unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("not-written-to-disk"));
        let other = Api {
            user_id: Uuid::new_v4(),
            ..api.clone()
        };
        assert!(load(&other).await.unwrap().is_empty());
        acknowledged(&api, request.client_id).await;
        assert!(load(&api).await.unwrap().is_empty());
        let _ = tokio::fs::remove_dir(directory(&api)).await;
    }
}
