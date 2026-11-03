use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ENCRYPTED_PREFIX: &str = "nm:e2ee:1:";
pub const MAX_ENCRYPTED_BODY: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterEncryptionKey {
    pub device_id: Uuid,
    pub public_key: String,
    pub signing_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptionKey {
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub public_key: String,
    pub signing_key: String,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WrappedMessageKey {
    pub device_id: Uuid,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedMessage {
    pub sender_device: Uuid,
    pub nonce: String,
    pub ciphertext: String,
    pub recipients: Vec<WrappedMessageKey>,
    pub signature: String,
}
