//! GitHub release notifications. Only authenticated, stable releases from the
//! configured repository are accepted; update artifacts are not executed here.
use crate::{auth::Session, error::{Error, Result}, state::AppState};
use actix_web::{web, HttpRequest, HttpResponse};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Payload {
    action: String,
    repository: Repository,
    release: Release,
}

#[derive(Deserialize)]
struct Repository { full_name: String }

#[derive(Deserialize, serde::Serialize)]
struct Release {
    tag_name: String,
    html_url: String,
    published_at: chrono::DateTime<chrono::Utc>,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize, serde::Serialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

fn verify(secret: &[u8], body: &[u8], header: &str) -> bool {
    let Some(signature) = header.strip_prefix("sha256=").and_then(|s| hex::decode(s).ok()) else { return false; };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else { return false; };
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

async fn webhook(state: web::Data<AppState>, request: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
    if body.len() > 1024 * 1024 { return Err(Error::too_large()); }
    let secret = std::env::var("GITHUB_WEBHOOK_SECRET").ok().filter(|s| s.len() >= 32).ok_or_else(Error::unavailable)?;
    let signature = request.headers().get("X-Hub-Signature-256").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !verify(secret.as_bytes(), &body, signature) { return Err(Error::forbidden()); }
    if request.headers().get("X-GitHub-Event").and_then(|v| v.to_str().ok()) != Some("release") {
        return Ok(HttpResponse::NoContent().finish());
    }
    let payload: Payload = serde_json::from_slice(&body).map_err(|_| Error::bad("Evento de release inválido"))?;
    if payload.repository.full_name != "lewisHeart/nmessenger" { return Err(Error::forbidden()); }
    if payload.action != "published" || payload.release.draft || payload.release.prerelease {
        return Ok(HttpResponse::NoContent().finish());
    }
    let release = payload.release;
    let expected = format!("https://github.com/lewisHeart/nmessenger/releases/tag/{}", release.tag_name);
    let safe_name = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !safe_name(&release.tag_name) || release.tag_name.len() > 128 || release.html_url != expected || release.assets.len() > 100 {
        return Err(Error::bad("Release inválida"));
    }
    for asset in &release.assets {
        if !safe_name(&asset.name) || asset.name.len() > 200 || asset.size == 0
            || asset.browser_download_url != format!("https://github.com/lewisHeart/nmessenger/releases/download/{}/{}", release.tag_name, asset.name)
        { return Err(Error::bad("Artefato inválido")); }
        if let Some(digest) = &asset.digest {
            if digest.len() != 71 || !digest.starts_with("sha256:") || hex::decode(&digest[7..]).is_err() {
                return Err(Error::bad("Hash inválido"));
            }
        }
    }
    let hash = hex::encode(Sha256::digest(&body));
    let mut tx = state.db.begin().await?;
    let inserted = sqlx::query("INSERT INTO github_release_deliveries(payload_hash) VALUES($1) ON CONFLICT DO NOTHING")
        .bind(hash).execute(&mut *tx).await?.rows_affected();
    let changed = if inserted == 1 {
        sqlx::query("INSERT INTO github_releases(singleton,published_at,payload) VALUES(true,$1,$2) ON CONFLICT(singleton) DO UPDATE SET published_at=EXCLUDED.published_at,payload=EXCLUDED.payload WHERE github_releases.published_at<EXCLUDED.published_at")
            .bind(release.published_at).bind(sqlx::types::Json(&release)).execute(&mut *tx).await?.rows_affected() == 1
    } else { false };
    tx.commit().await?;
    if changed { state.publish(Vec::new(), nm_protocol::ServerEvent::ReleaseChanged).await; }
    Ok(HttpResponse::NoContent().finish())
}

async fn latest(state: web::Data<AppState>, _session: Session) -> Result<HttpResponse> {
    let payload: Option<serde_json::Value> = sqlx::query_scalar("SELECT payload FROM github_releases WHERE singleton")
        .fetch_optional(&state.db).await?;
    Ok(match payload { Some(payload) => HttpResponse::Ok().json(payload), None => HttpResponse::NoContent().finish() })
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/webhooks/github")
        .app_data(web::PayloadConfig::new(1024 * 1024))
        .route(web::post().to(webhook)))
        .route("/releases/latest", web::get().to(latest));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_github_hmac_and_rejects_modified_payload() {
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(b"payload");
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        assert!(verify(b"secret", b"payload", &signature));
        assert!(!verify(b"secret", b"tampered", &signature));
        assert!(!verify(b"wrong", b"payload", &signature));
        assert!(!verify(b"secret", b"payload", "sha256=00"));
    }
}
