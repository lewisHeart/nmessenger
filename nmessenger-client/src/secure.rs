//! OS-managed credentials and owner-only, atomic local storage.
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub async fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Arquivo sem diretório")?;
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
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = async {
        let mut file = options
            .open(&temporary)
            .await
            .map_err(|error| error.to_string())?;
        file.write_all(bytes)
            .await
            .map_err(|error| error.to_string())?;
        file.sync_all().await.map_err(|error| error.to_string())?;
        drop(file);
        tokio::fs::rename(&temporary, path)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result
}

pub fn account(server: &str, email: &str) -> String {
    use sha2::{Digest, Sha256};
    let value = serde_json::to_vec(&(
        server.trim_end_matches('/'),
        email.trim().to_ascii_lowercase(),
    ))
    .expect("string tuple");
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(all(
    not(test),
    any(target_os = "linux", target_os = "windows", target_os = "macos")
))]
pub async fn credential(
    account: String,
    password: Option<String>,
    remove: bool,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        use zeroize::Zeroizing;
        let password = password.map(Zeroizing::new);
        let entry = keyring::Entry::new("NMessenger.login", &account).map_err(|error| error.to_string())?;
        let result = if remove { entry.delete_credential().map(|_| None) }
            else if let Some(password) = password { entry.set_password(password.as_str()).map(|_| None) }
            else { entry.get_password().map(Some) };
        match result {
            Ok(value) => Ok(value),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("Não foi possível acessar o cofre de credenciais do sistema. A senha não será salva em arquivo.".into()),
        }
    }).await.map_err(|error| error.to_string())?
}

#[cfg(all(not(test), target_os = "android"))]
pub async fn credential(
    account: String,
    password: Option<String>,
    remove: bool,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let password = password.map(zeroize::Zeroizing::new);
        crate::android::credential(
            &account,
            password.as_ref().map(|password| password.as_str()),
            remove,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(all(
    not(test),
    not(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos",
        target_os = "android"
    ))
))]
pub async fn credential(_: String, _: Option<String>, _: bool) -> Result<Option<String>, String> {
    Err("O armazenamento seguro de login não está disponível nesta plataforma.".into())
}

#[cfg(test)]
pub async fn credential(
    account: String,
    password: Option<String>,
    remove: bool,
) -> Result<Option<String>, String> {
    use std::{
        collections::HashMap,
        sync::{LazyLock, Mutex},
    };
    static STORE: LazyLock<Mutex<HashMap<String, String>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut store = STORE.lock().unwrap();
    if remove {
        store.remove(&account);
        Ok(None)
    } else if let Some(password) = password {
        store.insert(account, password);
        Ok(None)
    } else {
        Ok(store.get(&account).cloned())
    }
}
