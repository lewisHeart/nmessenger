use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, Generate, KeyInit, Payload},
};
use crypto_box::aead::{OsRng, rand_core::RngCore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize, Deserialize)]
pub struct Identity {
    pub device: Uuid,
    pub encryption: [u8; 32],
    pub signing: [u8; 32],
}

impl Drop for Identity {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.encryption.zeroize();
        self.signing.zeroize();
    }
}

#[derive(Serialize, Deserialize)]
struct Vault {
    version: u8,
    salt: String,
    nonce: String,
    ciphertext: String,
}

fn key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let mut key = Zeroizing::new([0; 32]);
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| "Falha ao proteger a identidade local")?;
    Ok(key)
}

fn seal(identity: &Identity, password: &str, account: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0; 16];
    OsRng.fill_bytes(&mut salt);
    let key = key(password, &salt)?;
    let nonce = XNonce::generate();
    let clear = Zeroizing::new(serde_json::to_vec(identity).map_err(|error| error.to_string())?);
    let ciphertext = XChaCha20Poly1305::new(&Key::from(*key))
        .encrypt(
            &nonce,
            Payload {
                msg: &clear,
                aad: account.as_bytes(),
            },
        )
        .map_err(|_| "Falha ao proteger a identidade local")?;
    serde_json::to_vec(&Vault {
        version: 1,
        salt: STANDARD.encode(salt),
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
    .map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) fn seal_for_test(identity: &Identity, password: &str, account: &str) -> Result<Vec<u8>, String> {
    seal(identity, password, account)
}

fn open(bytes: &[u8], password: &str, account: &str) -> Result<Identity, String> {
    let vault: Vault = serde_json::from_slice(bytes).map_err(|_| "Identidade local corrompida")?;
    let salt = STANDARD
        .decode(&vault.salt)
        .map_err(|_| "Identidade local corrompida")?;
    let nonce = STANDARD
        .decode(&vault.nonce)
        .map_err(|_| "Identidade local corrompida")?;
    let ciphertext = STANDARD
        .decode(&vault.ciphertext)
        .map_err(|_| "Identidade local corrompida")?;
    if vault.version != 1 || salt.len() != 16 || nonce.len() != 24 || ciphertext.len() > 2048 {
        return Err("Identidade local inválida".into());
    }
    let nonce_array: [u8; 24] = nonce.try_into().map_err(|_| "Identidade local corrompida")?;
    let key = key(password, &salt)?;
    let clear = Zeroizing::new(
        XChaCha20Poly1305::new(&Key::from(*key))
            .decrypt(
                &XNonce::from(nonce_array),
                Payload {
                    msg: &ciphertext,
                    aad: account.as_bytes(),
                },
            )
            .map_err(|_| "Não foi possível desbloquear a identidade local. Preserve o arquivo de identidade e verifique a senha original.")?,
    );
    serde_json::from_slice(&clear).map_err(|_| "Identidade local inválida".into())
}

pub(crate) fn open_backup(bytes: &[u8], password: &str, account: &str) -> Result<Identity, String> {
    if bytes.len() > 8192 { return Err("Arquivo de identidade inválido".into()); }
    open(bytes, password, account)
}

pub async fn load(account: &str, password: &str) -> Result<Identity, String> {
    let _lock = LOCK.lock().await;
    let path = crate::storage::directory()
        .join("identities")
        .join(format!("{account}.json"));
    let existing = match tokio::fs::read(&path).await {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let password = Zeroizing::new(password.to_owned());
    let account = account.to_owned();
    let (identity, encoded) = tokio::task::spawn_blocking(move || {
        if let Some(bytes) = existing {
            return open(&bytes, &password, &account).map(|identity| (identity, None));
        }
        let mut identity = Identity {
            device: Uuid::new_v4(),
            encryption: [0; 32],
            signing: [0; 32],
        };
        OsRng.fill_bytes(&mut identity.encryption);
        OsRng.fill_bytes(&mut identity.signing);
        let encoded = seal(&identity, &password, &account)?;
        Ok::<_, String>((identity, Some(encoded)))
    })
    .await
    .map_err(|error| error.to_string())??;
    if let Some(encoded) = encoded {
        // A second process must never overwrite a newly-created private identity.
        let parent = path.parent().unwrap();
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|error| error.to_string())?;
        }
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        use tokio::io::AsyncWriteExt;
        let mut file = options.open(&path).await.map_err(
            |_| "Identidade criada por outra instância. Entre novamente para usar a mesma chave.",
        )?;
        file.write_all(&encoded)
            .await
            .map_err(|error| error.to_string())?;
        file.sync_all().await.map_err(|error| error.to_string())?;
    }
    Ok(identity)
}

/// Re-encrypt the existing identity without replacing its keys after a server password reset.
pub async fn rewrap(account: &str, old_password: &str, new_password: &str) -> Result<(), String> {
    let _lock = LOCK.lock().await;
    let path = crate::storage::directory().join("identities").join(format!("{account}.json"));
    let bytes = tokio::fs::read(&path).await.map_err(|_| "Identidade local não encontrada")?;
    let account = account.to_owned();
    let old_password = Zeroizing::new(old_password.to_owned());
    let new_password = Zeroizing::new(new_password.to_owned());
    let encoded = tokio::task::spawn_blocking(move || {
        let identity = open(&bytes, &old_password, &account)?;
        seal(&identity, &new_password, &account)
    }).await.map_err(|error| error.to_string())??;
    crate::secure::write_private(&path, &encoded).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vault_is_authenticated_password_protected_and_bound_to_account() {
        let identity = Identity {
            device: Uuid::new_v4(),
            encryption: [17; 32],
            signing: [29; 32],
        };
        let encoded = seal(&identity, "a strong local password", "account-a").unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("encryption"));
        assert_eq!(
            open(&encoded, "a strong local password", "account-a")
                .unwrap()
                .device,
            identity.device
        );
        assert!(open(&encoded, "wrong password", "account-a").is_err());
        assert!(open(&encoded, "a strong local password", "account-b").is_err());
    }

    #[tokio::test]
    async fn password_reset_rewraps_existing_keys_only_with_original_password() {
        let account = format!("test-{}", Uuid::new_v4());
        let original = load(&account, "old password").await.unwrap();
        assert!(rewrap(&account, "wrong password", "new password").await.is_err());
        assert_eq!(load(&account, "old password").await.unwrap().device, original.device);
        rewrap(&account, "old password", "new password").await.unwrap();
        let recovered = load(&account, "new password").await.unwrap();
        assert_eq!(recovered.device, original.device);
        assert_eq!(recovered.encryption, original.encryption);
        assert_eq!(recovered.signing, original.signing);
        assert!(load(&account, "old password").await.is_err());
    }
}
