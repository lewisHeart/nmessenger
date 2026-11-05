//! Authenticated E2EE envelopes. Keys never leave the client in cleartext.
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead as ChaChaAead, Generate, KeyInit, Payload as ChaChaPayload},
};
use crypto_box::{
    PublicKey, SalsaBox, SecretKey,
    aead::{Aead as BoxAead, AeadCore as BoxAeadCore, OsRng},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use nm_protocol::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Binding {
    pub purpose: String,
    pub conversation: Uuid,
    pub sender: Uuid,
    pub object: Uuid,
    pub kind: String,
    pub sequence: Option<u64>,
    pub attachment: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity(user: Uuid) -> Crypto {
        Crypto {
            user,
            device: Uuid::new_v4(),
            secret: SecretKey::generate(&mut OsRng),
            signing: SigningKey::from_bytes(&SecretKey::generate(&mut OsRng).to_bytes()),
            account: Uuid::new_v4().to_string(),
            pins: tokio::sync::Mutex::new(HashMap::new()),
            conversations: Mutex::new(HashMap::new()),
            observed_conversations: Mutex::new(HashMap::new()),
            activities: Mutex::new(HashMap::new()),
            key_cache: tokio::sync::Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
            historical: Mutex::new(Vec::new()),
            historical_write: tokio::sync::Mutex::new(()),
            cloud_sync: tokio::sync::Mutex::new(()),
            history_export_key: Mutex::new(None),
        }
    }
    fn public(crypto: &Crypto) -> EncryptionKey {
        let key = crypto.registration();
        EncryptionKey {
            user_id: crypto.user,
            device_id: key.device_id,
            public_key: key.public_key,
            signing_key: key.signing_key,
            active: true,
        }
    }

    #[test]
    fn imported_historical_key_opens_old_messages_without_replacing_current_identity() {
        let sender = identity(Uuid::new_v4());
        let user = Uuid::new_v4();
        let old = identity(user);
        let current = identity(user);
        let conversation = Uuid::new_v4();
        let binding = Binding::message(conversation, sender.user, Uuid::new_v4(), MessageKind::Text, None);
        let old_keys = vec![public(&sender), public(&old)];
        let envelope = sender.seal(b"old message", &binding, &old_keys).unwrap();
        let mut all_keys = old_keys.clone();
        all_keys.push(public(&current));
        assert!(current.open(&envelope, &binding, &all_keys).is_err());
        current.historical.lock().unwrap().push(HistoricalKey { device: old.device, secret: old.secret.to_bytes() });
        assert_eq!(current.open(&envelope, &binding, &all_keys).unwrap().as_slice(), b"old message");
        assert_ne!(current.device, old.device);
        all_keys.retain(|key| key.device_id != old.device);
        assert!(current.open(&envelope, &binding, &all_keys).is_err());
    }

    #[tokio::test]
    async fn encrypted_cloud_archive_restores_old_device_messages_and_rejects_tampering() {
        let user = Uuid::new_v4();
        let sender = identity(Uuid::new_v4());
        let old = identity(user);
        let current = identity(user);
        let binding = Binding::message(Uuid::new_v4(), sender.user, Uuid::new_v4(), MessageKind::Text, None);
        let mut keys = vec![public(&sender), public(&old)];
        let envelope = sender.seal(b"group history", &binding, &keys).unwrap();
        keys.push(public(&current));
        let archive = old.seal_cloud_history(&[7; 64]).unwrap();
        assert!(current.merge_cloud_history(&archive, &[8; 64]).await.is_err());
        let mut tampered = archive.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(current.merge_cloud_history(&tampered, &[7; 64]).await.is_err());
        current.merge_cloud_history(&archive, &[7; 64]).await.unwrap();
        assert_eq!(current.open(&envelope, &binding, &keys).unwrap().as_slice(), b"group history");
        assert_ne!(current.device, old.device);
    }

    #[tokio::test]
    async fn encrypted_backup_recovers_history_after_reload_and_rejects_wrong_password() {
        let user = Uuid::new_v4();
        let base = "https://recovery.example.test/v1";
        let current = Crypto::load(base, user, "new device password").await.unwrap();
        let sender = identity(Uuid::new_v4());
        let old = identity(user);
        let old_identity = crate::identity_vault::Identity {
            device: old.device, encryption: old.secret.to_bytes(), signing: old.signing.to_bytes(),
        };
        let old_account = crate::secure::account("https://old-server-alias.example.test/v1", &user.to_string());
        let vault = crate::identity_vault::seal_for_test(&old_identity, "old device password", &old_account).unwrap();
        let backup = serde_json::to_vec(&IdentityBackup {
            format: "nmessenger-identity-v1".into(), user, account: old_account, vault: STANDARD.encode(vault),
        }).unwrap();
        let binding = Binding::message(Uuid::new_v4(), sender.user, Uuid::new_v4(), MessageKind::Text, None);
        let mut keys = vec![public(&sender), public(&old)];
        let envelope = sender.seal(b"recoverable history", &binding, &keys).unwrap();
        keys.push(public(&current));
        assert!(current.open(&envelope, &binding, &keys).is_err());
        assert!(current.import_identity_backup(backup.clone(), "wrong password".into()).await.is_err());
        current.import_identity_backup(backup, "old device password".into()).await.unwrap();
        let reloaded = Crypto::load(base, user, "new device password").await.unwrap();
        assert_eq!(reloaded.open(&envelope, &binding, &keys).unwrap().as_slice(), b"recoverable history");
        assert_ne!(reloaded.device, old.device);
    }

    #[tokio::test]
    async fn authenticated_envelopes_reject_tampering_outsiders_replay_and_low_order_keys() {
        let alice = identity(Uuid::new_v4());
        let bob = identity(Uuid::new_v4());
        let outsider = identity(Uuid::new_v4());
        let keys = vec![public(&alice), public(&bob)];
        let binding = Binding::message(
            Uuid::new_v4(),
            alice.user,
            Uuid::new_v4(),
            MessageKind::Text,
            None,
        );
        let envelope = alice
            .seal("segredo 👋".as_bytes(), &binding, &keys)
            .unwrap();
        assert_eq!(
            &*bob.open(&envelope, &binding, &keys).unwrap(),
            "segredo 👋".as_bytes()
        );
        assert!(outsider.open(&envelope, &binding, &keys).is_err());
        let mut tampered = envelope.clone();
        tampered.ciphertext.push('A');
        assert!(bob.open(&tampered, &binding, &keys).is_err());
        let mut another_conversation = binding.clone();
        another_conversation.conversation = Uuid::new_v4();
        assert!(bob.open(&envelope, &another_conversation, &keys).is_err());
        let mut another_client = binding.clone();
        another_client.object = Uuid::new_v4();
        assert!(bob.open(&envelope, &another_client, &keys).is_err());
        let mut another_sender = binding.clone();
        another_sender.sender = bob.user;
        assert!(bob.open(&envelope, &another_sender, &keys).is_err());
        let mut removed = envelope.clone();
        removed.recipients.pop();
        assert!(bob.open(&removed, &binding, &keys).is_err());
        assert!(alice.box_for(&STANDARD.encode([0u8; 32])).is_err());
        let mut low_order = [0u8; 32];
        low_order[0] = 1;
        assert!(alice.box_for(&STANDARD.encode(low_order)).is_err());
        let local = alice.protect_local(b"private pending message").unwrap();
        assert!(!local.windows(7).any(|bytes| bytes == b"private"));
        assert_eq!(
            &*alice.open_local(&local).unwrap(),
            b"private pending message"
        );
        assert!(bob.open_local(&local).is_err());
        alice.check_keys(&keys).await.unwrap();
        let new_device = identity(bob.user);
        let mut changed = keys.clone();
        changed.push(public(&new_device));
        alice.check_keys(&changed).await.unwrap();
        assert!(
            alice
                .verify_identity(public(&new_device), "not-verified")
                .await
                .is_err()
        );
        alice
            .verify_identity(
                public(&new_device),
                &Crypto::fingerprint(&public(&new_device)),
            )
            .await
            .unwrap();
        let mut tampered = changed.clone();
        tampered[0].public_key = STANDARD.encode([99u8; 32]);
        assert!(alice.check_keys(&tampered).await.is_err());
    }

    #[tokio::test]
    async fn group_membership_updates_automatically_and_call_type_remains_bound() {
        let alice = identity(Uuid::new_v4());
        let bob = identity(Uuid::new_v4());
        let outsider = identity(Uuid::new_v4());
        let profile = |id| Profile {
            id,
            email: format!("{id}@example.test"),
            display_name: id.to_string(),
            personal_message: String::new(),
            presence: Presence::Available,
            avatar_id: None,
            scene: String::new(),
            now_playing: None,
            created_at: chrono::Utc::now(),
            bio: String::new(),
            badges: Vec::new(),
        };
        let mut conversation = Conversation {
            id: Uuid::new_v4(),
            owner_id: None,
            title: Some("Private group".into()),
            kind: ConversationKind::Group,
            members: vec![profile(alice.user), profile(bob.user)],
            members_can_invite: true,
            updated_at: chrono::Utc::now(),
            unread: 0,
            last_message: None,
        };
        let keys = vec![public(&alice), public(&bob)];
        alice.observe_conversation(&conversation).await.unwrap();
        alice.check_roster(conversation.id, &keys, true).unwrap();
        conversation.members.push(profile(outsider.user));
        alice.observe_conversation(&conversation).await.unwrap();
        let mut changed = keys;
        changed.push(public(&outsider));
        alice.check_roster(conversation.id, &changed, true).unwrap();
        assert!(alice.check_roster(conversation.id, &changed[..2], true).is_err());
        conversation.members.pop();
        alice.observe_conversation(&conversation).await.unwrap();
        alice.check_roster(conversation.id, &changed[..2], true).unwrap();
        assert!(alice.check_roster(conversation.id, &changed, true).is_err());
        let mut activity = Activity {
            id: Uuid::new_v4(),
            conversation_id: conversation.id,
            initiator_id: alice.user,
            kind: ActivityKind::AudioCall,
            status: ActivityStatus::Accepted,
            payload: serde_json::json!({}),
            created_at: chrono::Utc::now(),
        };
        alice.remember_activity(&activity).unwrap();
        activity.kind = ActivityKind::RemoteAssistance;
        assert!(alice.remember_activity(&activity).is_err());
        activity.kind = ActivityKind::AudioCall;
        activity.status = ActivityStatus::Ended;
        alice.remember_activity(&activity).unwrap();
        activity.kind = ActivityKind::RemoteAssistance;
        activity.status = ActivityStatus::Offered;
        assert!(alice.remember_activity(&activity).is_err());
    }
}

impl Binding {
    pub fn message(
        conversation: Uuid,
        sender: Uuid,
        client: Uuid,
        kind: MessageKind,
        attachment: Option<Uuid>,
    ) -> Self {
        Self {
            purpose: "message".into(),
            conversation,
            sender,
            object: client,
            kind: serde_json::to_string(&kind).expect("message kind"),
            sequence: None,
            attachment,
        }
    }
}

pub struct Crypto {
    pub user: Uuid,
    pub device: Uuid,
    secret: SecretKey,
    signing: SigningKey,
    account: String,
    pins: tokio::sync::Mutex<HashMap<Uuid, EncryptionKey>>,
    pub conversations: Mutex<HashMap<Uuid, Vec<Uuid>>>,
    pub observed_conversations: Mutex<HashMap<Uuid, Vec<Uuid>>>,
    pub activities: Mutex<HashMap<Uuid, ChannelIdentity>>,
    pub key_cache: tokio::sync::Mutex<HashMap<Uuid, (std::time::Instant, Vec<EncryptionKey>)>>,
    pub files: Mutex<HashMap<Uuid, ProtectedAttachment>>,
    historical: Mutex<Vec<HistoricalKey>>,
    historical_write: tokio::sync::Mutex<()>,
    pub cloud_sync: tokio::sync::Mutex<()>,
    history_export_key: Mutex<Option<Zeroizing<Vec<u8>>>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct HistoricalKey {
    device: Uuid,
    secret: [u8; 32],
}

#[derive(Serialize, Deserialize)]
struct IdentityBackup {
    format: String,
    user: Uuid,
    account: String,
    vault: String,
}

#[derive(Serialize, Deserialize)]
struct HistoryArchive {
    version: u8,
    user: Uuid,
    keys: Vec<HistoricalKey>,
}

fn history_archive_key(export_key: &[u8], user: Uuid) -> Result<Zeroizing<[u8; 32]>, String> {
    use sha2::{Digest, Sha256};
    if export_key.len() < 32 { return Err("Chave de recuperação inválida".into()); }
    let mut hash = Sha256::new();
    hash.update(b"nmessenger-history-archive-v1\0");
    hash.update(user.as_bytes());
    hash.update(export_key);
    Ok(Zeroizing::new(hash.finalize().into()))
}

fn seal_history_archive(archive: &HistoryArchive, export_key: &[u8]) -> Result<Vec<u8>, String> {
    let key = history_archive_key(export_key, archive.user)?;
    let nonce = XNonce::generate();
    let clear = Zeroizing::new(serde_json::to_vec(archive).map_err(|e| e.to_string())?);
    let encrypted = XChaCha20Poly1305::new(&Key::from(*key))
        .encrypt(&nonce, ChaChaPayload { msg: &clear, aad: archive.user.as_bytes() })
        .map_err(|_| "Não foi possível proteger o histórico")?;
    let mut output = nonce.to_vec();
    output.extend_from_slice(&encrypted);
    Ok(output)
}

fn open_history_archive(bytes: &[u8], export_key: &[u8], user: Uuid) -> Result<HistoryArchive, String> {
    if !(40..=65536).contains(&bytes.len()) { return Err("Backup de histórico inválido".into()); }
    let key = history_archive_key(export_key, user)?;
    let nonce: [u8; 24] = bytes[..24].try_into().map_err(|_| "Backup inválido")?;
    let clear = Zeroizing::new(XChaCha20Poly1305::new(&Key::from(*key))
        .decrypt(&XNonce::from(nonce), ChaChaPayload { msg: &bytes[24..], aad: user.as_bytes() })
        .map_err(|_| "Não foi possível autenticar o backup de histórico")?);
    let archive: HistoryArchive = serde_json::from_slice(&clear).map_err(|_| "Backup de histórico inválido")?;
    if archive.version != 1 || archive.user != user || archive.keys.len() > 16 {
        return Err("Backup de histórico incompatível".into());
    }
    let mut seen = std::collections::HashSet::new();
    if archive.keys.iter().any(|key| key.device.is_nil() || !seen.insert(key.device)) {
        return Err("Identidades duplicadas no backup".into());
    }
    Ok(archive)
}

impl Drop for HistoricalKey {
    fn drop(&mut self) { use zeroize::Zeroize; self.secret.zeroize(); }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ChannelIdentity {
    pub conversation: Uuid,
    pub initiator: Uuid,
    pub kind: ActivityKind,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProtectedAttachment {
    pub file_id: Uuid,
    pub conversation: Uuid,
    pub owner: Uuid,
    pub digest: String,
    pub filename: String,
    pub mime: String,
    pub size: i64,
}

fn fixed<const N: usize>(text: &str) -> Result<[u8; N], String> {
    STANDARD
        .decode(text)
        .map_err(|_| "Codificação criptográfica inválida")?
        .try_into()
        .map_err(|_| "Tamanho criptográfico inválido".into())
}

impl Crypto {
    pub fn set_history_export_key(&self, key: &[u8]) -> Result<(), String> {
        *self.history_export_key.lock().map_err(|_| "Estado de chaves indisponível")? = Some(Zeroizing::new(key.to_vec()));
        Ok(())
    }

    pub fn history_export_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        Ok(self.history_export_key.lock().map_err(|_| "Estado de chaves indisponível")?.clone())
    }
    #[allow(dead_code)]
    pub fn seal_cloud_history(&self, export_key: &[u8]) -> Result<Vec<u8>, String> {
        let mut keys = self.historical.lock().map_err(|_| "Estado de chaves indisponível")?.clone();
        keys.retain(|key| key.device != self.device);
        if keys.len() >= 16 { return Err("Limite de identidades antigas atingido".into()); }
        keys.push(HistoricalKey { device: self.device, secret: self.secret.to_bytes() });
        seal_history_archive(&HistoryArchive { version: 1, user: self.user, keys }, export_key)
    }

    pub fn cloud_history_contains_local(&self, bytes: &[u8], export_key: &[u8]) -> Result<bool, String> {
        let archive = open_history_archive(bytes, export_key, self.user)?;
        let keys = self.historical.lock().map_err(|_| "Estado de chaves indisponível")?;
        let own = archive.keys.iter().any(|key| key.device == self.device && key.secret == self.secret.to_bytes());
        Ok(own && keys.iter().all(|local| archive.keys.iter().any(|key| key.device == local.device && key.secret == local.secret)))
    }

    pub fn historical_key_count(&self) -> Result<usize, String> {
        Ok(self.historical.lock().map_err(|_| "Estado de chaves indisponível")?.len())
    }

    #[allow(dead_code)]
    pub async fn merge_cloud_history(&self, bytes: &[u8], export_key: &[u8]) -> Result<(), String> {
        let _write = self.historical_write.lock().await;
        let archive = open_history_archive(bytes, export_key, self.user)?;
        let mut keys = self.historical.lock().map_err(|_| "Estado de chaves indisponível")?.clone();
        let original_count = keys.len();
        for incoming in archive.keys {
            if incoming.device == self.device {
                if incoming.secret != self.secret.to_bytes() { return Err("Chave de dispositivo divergente no backup".into()); }
            } else if let Some(existing) = keys.iter().find(|key| key.device == incoming.device) {
                if existing.secret != incoming.secret { return Err("Chave histórica divergente no backup".into()); }
            } else {
                if keys.len() >= 16 { return Err("Limite de identidades antigas atingido".into()); }
                keys.push(incoming.clone());
            }
        }
        if keys.len() == original_count { return Ok(()); }
        let clear = Zeroizing::new(serde_json::to_vec(&keys).map_err(|e| e.to_string())?);
        let protected = self.protect_local(&clear)?;
        let path = crate::storage::directory().join("identities").join(format!("{}.history", self.account));
        crate::secure::write_private(&path, &protected).await?;
        *self.historical.lock().map_err(|_| "Estado de chaves indisponível")? = keys;
        Ok(())
    }

    pub fn remember_activity(&self, activity: &Activity) -> Result<(), String> {
        let identity = ChannelIdentity {
            conversation: activity.conversation_id,
            initiator: activity.initiator_id,
            kind: activity.kind,
        };
        let mut channels = self
            .activities
            .lock()
            .map_err(|_| "Estado de atividade indisponível")?;
        if channels
            .get(&activity.id)
            .is_some_and(|old| *old != identity)
        {
            return Err(
                "Tipo, conversa ou origem da atividade foram alterados; mídia bloqueada".into(),
            );
        }
        if channels.len() >= 4096 && !channels.contains_key(&activity.id) {
            return Err("Limite de atividades desta sessão atingido. Reconecte antes de iniciar outra chamada.".into());
        }
        channels.insert(activity.id, identity);
        Ok(())
    }
    pub async fn load(base: &str, user: Uuid, password: &str) -> Result<Self, String> {
        let account = crate::secure::account(base, &user.to_string());
        let identity = crate::identity_vault::load(&account, password).await?;
        let pins = match tokio::fs::read(
            crate::storage::directory()
                .join("identities")
                .join(format!("{account}.pins.json")),
        )
        .await
        {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Registro de identidades conhecidas corrompido")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(error) => return Err(error.to_string()),
        };
        let value = Self {
            user,
            device: identity.device,
            secret: SecretKey::from(identity.encryption),
            signing: SigningKey::from_bytes(&identity.signing),
            account,
            pins: tokio::sync::Mutex::new(pins),
            conversations: Mutex::new(HashMap::new()),
            observed_conversations: Mutex::new(HashMap::new()),
            activities: Mutex::new(HashMap::new()),
            key_cache: tokio::sync::Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
            historical: Mutex::new(Vec::new()),
            historical_write: tokio::sync::Mutex::new(()),
            cloud_sync: tokio::sync::Mutex::new(()),
            history_export_key: Mutex::new(None),
        };
        let path = crate::storage::directory()
            .join("identities")
            .join(format!("{}.files", value.account));
        match tokio::fs::read(path).await {
            Ok(bytes) => {
                *value
                    .files
                    .lock()
                    .map_err(|_| "Estado de arquivos indisponível")? =
                    serde_json::from_slice(&value.open_local(&bytes)?)
                        .map_err(|_| "Registro de anexos corrompido")?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let path = crate::storage::directory()
            .join("identities")
            .join(format!("{}.rosters", value.account));
        match tokio::fs::read(path).await {
            Ok(bytes) => {
                *value
                    .conversations
                    .lock()
                    .map_err(|_| "Estado de participantes indisponível")? =
                    serde_json::from_slice(&value.open_local(&bytes)?)
                        .map_err(|_| "Registro de participantes corrompido")?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let path = crate::storage::directory().join("identities").join(format!("{}.history", value.account));
        match tokio::fs::read(path).await {
            Ok(bytes) => {
                if bytes.len() > 65536 { return Err("Registro de chaves antigas inválido".into()); }
                let clear = Zeroizing::new(value.open_local(&bytes)?);
                let keys: Vec<HistoricalKey> = serde_json::from_slice(&clear).map_err(|_| "Registro de chaves antigas corrompido")?;
                if keys.len() > 16 { return Err("Número de chaves antigas excede o limite".into()); }
                *value.historical.lock().map_err(|_| "Estado de chaves indisponível")? = keys;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(value)
    }

    pub async fn export_identity_backup(&self) -> Result<Vec<u8>, String> {
        let path = crate::storage::directory().join("identities").join(format!("{}.json", self.account));
        let bytes = tokio::fs::read(path).await.map_err(|error| error.to_string())?;
        if bytes.len() > 8192 { return Err("Arquivo de identidade inválido".into()); }
        serde_json::to_vec(&IdentityBackup {
            format: "nmessenger-identity-v1".into(), user: self.user,
            account: self.account.clone(), vault: STANDARD.encode(bytes),
        }).map_err(|error| error.to_string())
    }

    pub async fn import_identity_backup(&self, bytes: Vec<u8>, password: String) -> Result<(), String> {
        let _write = self.historical_write.lock().await;
        if bytes.len() > 32768 { return Err("Arquivo de identidade inválido".into()); }
        let (bytes, account) = match serde_json::from_slice::<IdentityBackup>(&bytes) {
            Ok(backup) => {
                if backup.format != "nmessenger-identity-v1" || backup.user != self.user
                    || backup.account.len() != 64 || !backup.account.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Identidade pertence a outra conta ou arquivo inválido".into());
                }
                (STANDARD.decode(backup.vault).map_err(|_| "Arquivo de identidade inválido")?, backup.account)
            }
            Err(_) => (bytes, self.account.clone()),
        };
        let password = Zeroizing::new(password);
        let identity = tokio::task::spawn_blocking(move || crate::identity_vault::open_backup(&bytes, &password, &account))
            .await.map_err(|error| error.to_string())??;
        if identity.device == self.device { return Err("Esta é a identidade do dispositivo atual".into()); }
        let mut keys = self.historical.lock().map_err(|_| "Estado de chaves indisponível")?.clone();
        if keys.iter().any(|key| key.device == identity.device) { return Ok(()); }
        if keys.len() >= 16 { return Err("Limite de identidades antigas atingido".into()); }
        keys.push(HistoricalKey { device: identity.device, secret: identity.encryption });
        let clear = Zeroizing::new(serde_json::to_vec(&keys).map_err(|error| error.to_string())?);
        let protected = self.protect_local(&clear)?;
        let path = crate::storage::directory().join("identities").join(format!("{}.history", self.account));
        crate::secure::write_private(&path, &protected).await?;
        *self.historical.lock().map_err(|_| "Estado de chaves indisponível")? = keys;
        Ok(())
    }

    pub fn registration(&self) -> RegisterEncryptionKey {
        RegisterEncryptionKey {
            device_id: self.device,
            public_key: STANDARD.encode(self.secret.public_key().as_bytes()),
            signing_key: STANDARD.encode(self.signing.verifying_key().as_bytes()),
        }
    }

    pub async fn observe_conversation(&self, conversation: &Conversation) -> Result<(), String> {
        let mut members: Vec<_> = conversation
            .members
            .iter()
            .map(|profile| profile.id)
            .collect();
        members.sort_unstable();
        members.dedup();
        if !members.contains(&self.user) { return Err("Conversa não pertence a esta conta".into()); }
        let changed = self.conversations.lock().map_err(|_| "Estado de participantes indisponível")?
            .get(&conversation.id).is_some_and(|previous| previous != &members);
        self.observed_conversations
            .lock()
            .map_err(|_| "Estado de participantes indisponível")?
            .insert(conversation.id, members.clone());
        self.commit_roster(conversation.id, members, conversation.kind == ConversationKind::Group).await?;
        if changed { self.key_cache.lock().await.remove(&conversation.id); }
        Ok(())
    }

    async fn commit_roster(
        &self,
        conversation: Uuid,
        members: Vec<Uuid>,
        replace: bool,
    ) -> Result<(), String> {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _lock = LOCK.lock().await;
        let mut candidate = self
            .conversations
            .lock()
            .map_err(|_| "Estado de participantes indisponível")?
            .clone();
        if candidate.get(&conversation) == Some(&members) || (!replace && candidate.contains_key(&conversation)) {
            return Ok(());
        }
        candidate.insert(conversation, members);
        let clear = zeroize::Zeroizing::new(
            serde_json::to_vec(&candidate).map_err(|error| error.to_string())?,
        );
        let bytes = self.protect_local(&clear)?;
        let path = crate::storage::directory()
            .join("identities")
            .join(format!("{}.rosters", self.account));
        crate::secure::write_private(&path, &bytes).await?;
        *self
            .conversations
            .lock()
            .map_err(|_| "Estado de participantes indisponível")? = candidate;
        Ok(())
    }

    pub fn check_roster(
        &self,
        conversation: Uuid,
        keys: &[EncryptionKey],
        all_members: bool,
    ) -> Result<(), String> {
        let trusted = self
            .conversations
            .lock()
            .map_err(|_| "Estado de participantes indisponível")?;
        let members = trusted
            .get(&conversation)
            .ok_or("Atualize a conversa antes de usar criptografia")?;
        let observed = self
            .observed_conversations
            .lock()
            .map_err(|_| "Estado de participantes indisponível")?;
        if observed.get(&conversation) != Some(members)
            || keys.iter().any(|key| !members.contains(&key.user_id))
        {
            return Err("Os participantes da conversa mudaram. Confira o código da conversa em Opções > Privacidade antes de continuar.".into());
        }
        if all_members
            && members
                .iter()
                .any(|user| !keys.iter().any(|key| key.user_id == *user && key.active))
        {
            return Err("Todos os participantes precisam entrar em um cliente atualizado. Nenhum conteúdo foi enviado sem proteção.".into());
        }
        Ok(())
    }

    pub fn roster_fingerprint(conversation: Uuid, keys: &[EncryptionKey]) -> String {
        use sha2::{Digest, Sha256};
        let mut identities: Vec<_> = keys
            .iter()
            .map(|key| {
                (
                    key.user_id,
                    key.device_id,
                    &key.public_key,
                    &key.signing_key,
                )
            })
            .collect();
        identities.sort_unstable();
        Sha256::digest(serde_json::to_vec(&(conversation, identities)).expect("public roster"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub async fn verify_roster(
        &self,
        conversation: Uuid,
        keys: &[EncryptionKey],
        supplied: &str,
    ) -> Result<(), String> {
        let supplied: String = supplied
            .chars()
            .filter(|character| !character.is_whitespace() && *character != '-')
            .collect();
        if supplied.to_ascii_lowercase() != Self::roster_fingerprint(conversation, keys) {
            return Err(
                "O código recebido não corresponde aos participantes desta conversa".into(),
            );
        }
        self.check_keys(keys).await?;
        let members = self
            .observed_conversations
            .lock()
            .map_err(|_| "Estado de participantes indisponível")?
            .get(&conversation)
            .cloned()
            .ok_or("Atualize os participantes antes de verificar")?;
        if members
            .iter()
            .any(|user| !keys.iter().any(|key| key.user_id == *user))
            || keys.iter().any(|key| !members.contains(&key.user_id))
        {
            return Err("A lista de chaves não corresponde aos participantes".into());
        }
        self.commit_roster(conversation, members, true).await?;
        self.key_cache.lock().await.clear();
        Ok(())
    }

    fn box_for(&self, public: &str) -> Result<SalsaBox, String> {
        Self::box_for_secret(&self.secret, public)
    }

    fn box_for_secret(secret: &SecretKey, public: &str) -> Result<SalsaBox, String> {
        let bytes = fixed::<32>(public)?;
        if x25519_dalek::x25519(secret.to_bytes(), bytes)
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err("Chave pública de baixa ordem rejeitada".into());
        }
        Ok(SalsaBox::new(&PublicKey::from(bytes), secret))
    }

    pub async fn check_keys(&self, keys: &[EncryptionKey]) -> Result<(), String> {
        let mut pins = self.pins.lock().await;
        for key in keys {
            if key.device_id == self.device
                && (key.user_id != self.user
                    || key.public_key != self.registration().public_key
                    || key.signing_key != self.registration().signing_key)
            {
                return Err(
                    "O servidor retornou uma identidade diferente da chave privada local".into(),
                );
            }
            self.box_for(&key.public_key)?;
            VerifyingKey::from_bytes(&fixed(&key.signing_key)?)
                .map_err(|_| "Chave de assinatura inválida")?;
            if let Some(old) = pins.get(&key.device_id) {
                if old.user_id != key.user_id
                    || old.public_key != key.public_key
                    || old.signing_key != key.signing_key
                {
                    return Err("A identidade criptográfica de um dispositivo mudou. Comunicação bloqueada.".into());
                }
            }
        }
        let mut changed = false;
        let mut candidate = pins.clone();
        for key in keys {
            if let std::collections::hash_map::Entry::Vacant(entry) = candidate.entry(key.device_id)
            {
                entry.insert(key.clone());
                changed = true;
            }
        }
        if changed {
            let path = crate::storage::directory()
                .join("identities")
                .join(format!("{}.pins.json", self.account));
            crate::secure::write_private(
                &path,
                &serde_json::to_vec(&candidate).map_err(|error| error.to_string())?,
            )
            .await?;
            *pins = candidate;
        }
        Ok(())
    }

    pub fn seal(
        &self,
        clear: &[u8],
        binding: &Binding,
        keys: &[EncryptionKey],
    ) -> Result<EncryptedMessage, String> {
        if binding.sender != self.user {
            return Err("Remetente criptográfico incorreto".into());
        }
        let active: Vec<_> = keys.iter().filter(|key| key.active).collect();
        if active.is_empty() || active.len() > 256 {
            return Err("Destinatários criptográficos indisponíveis".into());
        }
        let content_key: Zeroizing<[u8; 32]> =
            Zeroizing::new(Key::generate().into());
        let nonce = XNonce::generate();
        let aad = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
        let ciphertext = XChaCha20Poly1305::new(&Key::from(*content_key))
            .encrypt(
                &nonce,
                ChaChaPayload {
                    msg: clear,
                    aad: &aad,
                },
            )
            .map_err(|_| "Falha de criptografia")?;
        let mut recipients = Vec::new();
        for key in active {
            let box_ = self.box_for(&key.public_key)?;
            let nonce = SalsaBox::generate_nonce(&mut OsRng);
            let ciphertext = box_
                .encrypt(&nonce, content_key.as_ref())
                .map_err(|_| "Falha ao proteger a chave da mensagem")?;
            recipients.push(WrappedMessageKey {
                device_id: key.device_id,
                nonce: STANDARD.encode(nonce),
                ciphertext: STANDARD.encode(ciphertext),
            });
        }
        let mut envelope = EncryptedMessage {
            sender_device: self.device,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
            recipients,
            signature: String::new(),
        };
        let signed =
            serde_json::to_vec(&(binding, &envelope)).map_err(|error| error.to_string())?;
        envelope.signature = STANDARD.encode(self.signing.sign(&signed).to_bytes());
        Ok(envelope)
    }

    pub fn open(
        &self,
        envelope: &EncryptedMessage,
        binding: &Binding,
        keys: &[EncryptionKey],
    ) -> Result<Zeroizing<Vec<u8>>, String> {
        let sender = keys
            .iter()
            .find(|key| key.device_id == envelope.sender_device && key.user_id == binding.sender)
            .ok_or("Identidade do remetente desconhecida")?;
        let verifier = VerifyingKey::from_bytes(&fixed(&sender.signing_key)?)
            .map_err(|_| "Chave de assinatura inválida")?;
        let signature = Signature::from_bytes(&fixed(&envelope.signature)?);
        let mut unsigned = envelope.clone();
        unsigned.signature.clear();
        let signed =
            serde_json::to_vec(&(binding, &unsigned)).map_err(|error| error.to_string())?;
        verifier
            .verify_strict(&signed, &signature)
            .map_err(|_| "Assinatura da mensagem inválida")?;
        let mut old_secret = None;
        let wrapped = if let Some(wrapped) = envelope.recipients.iter().find(|key| key.device_id == self.device) {
            wrapped
        } else {
            let keys_guard = self.historical.lock().map_err(|_| "Estado de chaves indisponível")?;
            let (wrapped, secret) = keys_guard.iter().find_map(|old| {
                let secret = SecretKey::from(old.secret);
                let registered = keys.iter().any(|key| key.user_id == self.user && key.device_id == old.device
                    && key.public_key == STANDARD.encode(secret.public_key().as_bytes()));
                registered.then(|| envelope.recipients.iter().find(|key| key.device_id == old.device).map(|wrapped| (wrapped, secret))).flatten()
            }).ok_or("Esta mensagem foi enviada antes de este dispositivo ser autorizado. Importe a identidade do dispositivo original para recuperar o histórico.")?;
            old_secret = Some(secret);
            wrapped
        };
        let nonce = fixed::<24>(&wrapped.nonce)?;
        let wrapped_bytes = STANDARD
            .decode(&wrapped.ciphertext)
            .map_err(|_| "Chave de mensagem inválida")?;
        let key = Zeroizing::new(
            Self::box_for_secret(old_secret.as_ref().unwrap_or(&self.secret), &sender.public_key)?
                .decrypt((&nonce).into(), wrapped_bytes.as_slice())
                .map_err(|_| "Chave de mensagem não autenticada")?,
        );
        if key.len() != 32 {
            return Err("Chave de mensagem inválida".into());
        }
        let nonce = fixed::<24>(&envelope.nonce)?;
        let ciphertext = STANDARD
            .decode(&envelope.ciphertext)
            .map_err(|_| "Conteúdo criptográfico inválido")?;
        let aad = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
        let key_array: [u8; 32] = key.as_slice().try_into().map_err(|_| "Chave de mensagem inválida")?;
        XChaCha20Poly1305::new(&Key::from(key_array))
            .decrypt(
                &XNonce::from(nonce),
                ChaChaPayload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| "Conteúdo alterado ou identidade incorreta".into())
    }

    pub fn protect_local(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let box_ = SalsaBox::new(&self.secret.public_key(), &self.secret);
        let nonce = SalsaBox::generate_nonce(&mut OsRng);
        let clear = Zeroizing::new(
            serde_json::to_vec(&(&self.account, bytes)).map_err(|error| error.to_string())?,
        );
        let encrypted = box_
            .encrypt(&nonce, clear.as_slice())
            .map_err(|_| "Falha de criptografia local")?;
        Ok([b"NML1".as_slice(), nonce.as_slice(), &encrypted].concat())
    }

    pub async fn save_files(&self) -> Result<(), String> {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _lock = LOCK.lock().await;
        let clear = Zeroizing::new(
            serde_json::to_vec(
                &*self
                    .files
                    .lock()
                    .map_err(|_| "Estado de arquivos indisponível")?,
            )
            .map_err(|error| error.to_string())?,
        );
        let encoded = self.protect_local(&clear)?;
        let path = crate::storage::directory()
            .join("identities")
            .join(format!("{}.files", self.account));
        crate::secure::write_private(&path, &encoded).await
    }

    pub fn fingerprint(key: &EncryptionKey) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(
            serde_json::to_vec(&(
                key.user_id,
                key.device_id,
                &key.public_key,
                &key.signing_key,
            ))
            .expect("public identity"),
        )
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
    }

    pub async fn verify_identity(
        &self,
        key: EncryptionKey,
        fingerprint: &str,
    ) -> Result<(), String> {
        let supplied: String = fingerprint
            .chars()
            .filter(|character| !character.is_whitespace() && *character != '-')
            .collect();
        if supplied.to_ascii_lowercase() != Self::fingerprint(&key) {
            return Err("O código informado não corresponde à identidade deste dispositivo".into());
        }
        self.box_for(&key.public_key)?;
        VerifyingKey::from_bytes(&fixed(&key.signing_key)?)
            .map_err(|_| "Chave de assinatura inválida")?;
        let mut pins = self.pins.lock().await;
        let mut candidate = pins.clone();
        candidate.insert(key.device_id, key);
        let path = crate::storage::directory()
            .join("identities")
            .join(format!("{}.pins.json", self.account));
        crate::secure::write_private(
            &path,
            &serde_json::to_vec(&candidate).map_err(|error| error.to_string())?,
        )
        .await?;
        *pins = candidate;
        drop(pins);
        self.key_cache.lock().await.clear();
        Ok(())
    }

    pub fn open_local(&self, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        if bytes.len() < 44 || !bytes.starts_with(b"NML1") {
            return Err("Arquivo local sem criptografia".into());
        }
        let box_ = SalsaBox::new(&self.secret.public_key(), &self.secret);
        let clear = Zeroizing::new(
            box_.decrypt(bytes[4..28].into(), &bytes[28..])
                .map_err(|_| "Arquivo local alterado ou chave incorreta")?,
        );
        let (account, bytes): (String, Vec<u8>) =
            serde_json::from_slice(&clear).map_err(|_| "Arquivo local inválido")?;
        if account != self.account {
            return Err("Arquivo local pertence a outra conta".into());
        }
        Ok(Zeroizing::new(bytes))
    }
}
