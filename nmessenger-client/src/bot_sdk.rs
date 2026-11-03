//! E2EE client for NMessenger bots. Private keys remain in the bot's local vault.
//!
//! The bot token authenticates requests; `vault_password` protects its local
//! encryption keys and must be kept separately and backed up by the operator.
use crate::{encryption::Crypto, transport::{api, Api}};
use nm_protocol::{Bootstrap, Conversation, EncryptionKey, FileInfo, Message, MessageKind, MessageStyle, SendMessageRequest, SessionInfo};
use reqwest::Method;
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Authenticated bot endpoint. Construct one instance per independent device.
pub struct BotClient {
    api: Api,
}

impl BotClient {
    /// Connect using the bot ID and token issued to its owner. The vault password
    /// is never sent to the server and is required to reuse this device's keys.
    pub async fn connect(server: &str, bot_id: Uuid, token: String, vault_password: &str) -> Result<Self, String> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Token de bot inválido".into());
        }
        if vault_password.len() < 8 {
            return Err("A senha do cofre deve ter pelo menos 8 bytes".into());
        }
        let mut api = Api {
            client: api::client()?, base: api::base(server)?,
            token: Arc::new(Zeroizing::new(token)), user_id: bot_id,
            session_id: Uuid::nil(), crypto: None,
        };
        let bootstrap: Bootstrap = api.get("/bootstrap").await?;
        if bootstrap.me.id != bot_id { return Err("Token não pertence ao bot informado".into()); }
        let sessions: Vec<SessionInfo> = api.get("/sessions").await?;
        api.session_id = sessions.iter().find(|session| session.current)
            .ok_or("Sessão do bot não encontrada")?.id;
        let crypto = Arc::new(Crypto::load(&api.base, bot_id, vault_password).await?);
        api.crypto = Some(crypto.clone());
        api.json::<EncryptionKey, _>(Method::POST, "/encryption/keys", &crypto.registration()).await?;
        // Loads and pins the actual participant roster before encrypting messages.
        let _: Bootstrap = api.get("/bootstrap").await?;
        Ok(Self { api })
    }

    pub fn id(&self) -> Uuid { self.api.user_id }

    /// Refresh conversations before polling, especially after group membership changes.
    pub async fn conversations(&self) -> Result<Vec<Conversation>, String> {
        self.api.get("/conversations").await
    }

    /// Read and authenticate encrypted messages; use `before` for older pages.
    pub async fn history(&self, conversation: Uuid, before: Option<Uuid>) -> Result<Vec<Message>, String> {
        self.api.history(conversation, before, "").await
    }

    /// Encrypt and send a text message to an existing conversation.
    pub async fn send_text(&self, conversation: Uuid, text: &str) -> Result<Message, String> {
        self.send_text_with_id(conversation, Uuid::new_v4(), text).await
    }

    /// Reuse `client_id` when retrying the same message after a connection failure.
    pub async fn send_text_with_id(&self, conversation: Uuid, client_id: Uuid, text: &str) -> Result<Message, String> {
        self.api.json(Method::POST, &format!("/conversations/{conversation}/messages"), &SendMessageRequest {
            client_id, kind: MessageKind::Text, body: text.into(),
            style: MessageStyle::default(), attachment_id: None,
        }).await
    }

    /// Encrypt and upload a file for a conversation before sending it as a message.
    pub async fn upload_file(&self, conversation: Uuid, name: String, mime: &str, bytes: Vec<u8>) -> Result<FileInfo, String> {
        self.api.upload(Some(conversation), name, mime, bytes).await
    }

    /// Send a previously uploaded file. Reuse `client_id` for retries.
    pub async fn send_file(&self, conversation: Uuid, client_id: Uuid, file_id: Uuid, caption: &str) -> Result<Message, String> {
        self.api.json(Method::POST, &format!("/conversations/{conversation}/messages"), &SendMessageRequest {
            client_id, kind: MessageKind::File, body: caption.into(),
            style: MessageStyle::default(), attachment_id: Some(file_id),
        }).await
    }

    /// Download an attachment after reading its authenticated message via `history`.
    pub async fn download_file(&self, file_id: Uuid) -> Result<Vec<u8>, String> {
        self.api.bytes(file_id).await
    }

    /// Mark a conversation read through the specified message.
    pub async fn mark_read(&self, conversation: Uuid, message_id: Option<Uuid>) -> Result<(), String> {
        self.api.read(conversation, message_id).await
    }
}
