//! Download release artifacts only after independently checking GitHub metadata.
use nm_protocol::ReleaseInfo;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Duration};
use tokio::io::AsyncWriteExt;

const MAX_SIZE: u64 = 2 * 1024 * 1024 * 1024;

fn matches_digest(hash: Sha256, expected: &str) -> bool {
    let actual = hash.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    actual.eq_ignore_ascii_case(expected)
}

fn artifact_name(tag: &str, os: &str, arch: &str) -> Option<String> {
    let suffix = match (os, arch) {
        ("android", "aarch64") => "android-arm64-v8a.apk",
        ("android", "x86_64") => "android-x86_64.apk",
        ("linux", "x86_64") => "x86_64.AppImage",
        ("linux", "aarch64") => "aarch64.AppImage",
        ("windows", "x86_64") => "windows-x86_64-setup.exe",
        ("windows", "aarch64") => "windows-arm64-setup.exe",
        ("macos", "x86_64") => "macos-x86_64.dmg",
        ("macos", "aarch64") => "macos-arm64.dmg",
        _ => return None,
    };
    Some(format!("nmessenger-{tag}-{suffix}"))
}

fn validate_tag(tag: &str) -> Result<(), String> {
    let version = semver::Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|_| "Versão inválida")?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(|_| "Versão instalada inválida")?;
    if tag.len() > 128 || !tag.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')) || !version.pre.is_empty() || version <= current {
        return Err("Versão de atualização inválida ou antiga".into());
    }
    Ok(())
}

pub async fn download(tag: &str) -> Result<Option<PathBuf>, String> {
    validate_tag(tag)?;
    let name = artifact_name(tag, std::env::consts::OS, std::env::consts::ARCH).ok_or("Esta plataforma não possui um pacote de atualização")?;
    let client = reqwest::Client::builder().user_agent("NMessenger updater")
        .connect_timeout(Duration::from_secs(15)).timeout(Duration::from_secs(900))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() < 5 && url.scheme() == "https" && matches!(url.host_str(), Some("github.com" | "release-assets.githubusercontent.com" | "objects.githubusercontent.com")) {
                attempt.follow()
            } else { attempt.stop() }
        })).build().map_err(|e| e.to_string())?;
    let url = format!("https://api.github.com/repos/lewisHeart/nmessenger/releases/tags/{tag}");
    let response = client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let release: ReleaseInfo = crate::transport::api::bounded_json(response).await?;
    if release.tag_name != tag || release.draft || release.prerelease { return Err("Release GitHub incompatível".into()); }
    let mut matching = release.assets.iter().filter(|asset| asset.name == name);
    let asset = matching.next().ok_or("Pacote desta plataforma ainda não está disponível")?;
    if matching.next().is_some() || asset.size == 0 || asset.size > MAX_SIZE { return Err("Tamanho ou pacote de atualização inválido".into()); }
    let expected_url = format!("https://github.com/lewisHeart/nmessenger/releases/download/{tag}/{name}");
    if asset.browser_download_url != expected_url { return Err("Origem do pacote inválida".into()); }
    let digest = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:"))
        .filter(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("GitHub não informou o SHA-256 do pacote")?;
    let Some(path) = crate::platform::save_file(name, None).await? else { return Ok(None); };
    let temp = path.with_extension(format!("download-{}", uuid::Uuid::new_v4()));
    let result = async {
        let mut response = client.get(expected_url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
        if response.content_length().is_some_and(|n| n != asset.size) { return Err("Tamanho do pacote não corresponde à release".into()); }
        let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true).open(&temp).await.map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            size = size.checked_add(chunk.len() as u64).ok_or("Pacote grande demais")?;
            if size > asset.size { return Err("Pacote excede o tamanho informado".into()); }
            hash.update(&chunk);
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        drop(file);
        if size != asset.size || !matches_digest(hash, digest) { return Err("Falha na verificação SHA-256 do pacote".into()); }
        tokio::fs::rename(&temp, &path).await.map_err(|e| e.to_string())?;
        crate::platform::finish_save(&path).await?;
        Ok(Some(path))
    }.await;
    if result.is_err() { let _ = tokio::fs::remove_file(temp).await; }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_and_old_versions() {
        assert!(validate_tag("v9999.0.0").is_ok());
        assert!(validate_tag("v0.0.1").is_err());
        assert!(validate_tag("v9999.0.0-rc1").is_err());
        assert!(validate_tag("../../evil").is_err());
    }
    #[test]
    fn names_match_release_packaging() {
        assert_eq!(artifact_name("v1.2.3", "linux", "aarch64").unwrap(), "nmessenger-v1.2.3-aarch64.AppImage");
        assert_eq!(artifact_name("v1.2.3", "windows", "aarch64").unwrap(), "nmessenger-v1.2.3-windows-arm64-setup.exe");
        assert_eq!(artifact_name("v1.2.3", "android", "aarch64").unwrap(), "nmessenger-v1.2.3-android-arm64-v8a.apk");
    }
    #[test]
    fn digest_check_rejects_corrupted_downloads() {
        let mut hash = Sha256::new();
        hash.update(b"abc");
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(matches_digest(hash.clone(), expected));
        hash.update(b"tampered");
        assert!(!matches_digest(hash, expected));
    }
}
