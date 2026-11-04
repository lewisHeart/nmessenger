//! Per-session push registration for a configured UnifiedPush distributor.
use crate::{auth::Session, error::{Error, Result}, state::AppState};
use actix_web::{web, HttpResponse};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

#[derive(Deserialize)]
struct Registration { endpoint: String, public_key: String, auth_secret: String }

fn validate_endpoint(endpoint: &str, configured_origin: &str) -> bool {
    let Ok(endpoint) = reqwest::Url::parse(endpoint) else { return false; };
    let Ok(origin) = reqwest::Url::parse(configured_origin) else { return false; };
    endpoint.scheme() == "https" && origin.scheme() == "https"
        && endpoint.origin() == origin.origin() && endpoint.username().is_empty()
        && endpoint.password().is_none() && endpoint.fragment().is_none()
        && endpoint.query().is_none() && endpoint.path().len() > 16 && endpoint.as_str().len() <= 1000
}

async fn register(state: web::Data<AppState>, session: Session, body: web::Json<Registration>) -> Result<HttpResponse> {
    let origin = std::env::var("NTFY_ORIGIN").map_err(|_| Error::unavailable())?;
    if !validate_endpoint(&body.endpoint, &origin) { return Err(Error::bad("Endpoint push inválido")); }
    let key = URL_SAFE_NO_PAD.decode(&body.public_key).map_err(|_| Error::bad("Chave push inválida"))?;
    let secret = URL_SAFE_NO_PAD.decode(&body.auth_secret).map_err(|_| Error::bad("Segredo push inválido"))?;
    if key.len() != 65 || key[0] != 4 || secret.len() != 16 { return Err(Error::bad("Chaves push inválidas")); }
    sqlx::query("INSERT INTO push_devices(session_id,user_id,endpoint,public_key,auth_secret) VALUES($1,$2,$3,$4,$5) ON CONFLICT(session_id) DO UPDATE SET endpoint=EXCLUDED.endpoint,public_key=EXCLUDED.public_key,auth_secret=EXCLUDED.auth_secret,updated_at=now() WHERE push_devices.user_id=EXCLUDED.user_id")
        .bind(session.id).bind(session.user_id).bind(&body.endpoint).bind(&body.public_key).bind(&body.auth_secret)
        .execute(&state.db).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn unregister(state: web::Data<AppState>, session: Session) -> Result<HttpResponse> {
    sqlx::query("DELETE FROM push_devices WHERE session_id=$1 AND user_id=$2")
        .bind(session.id).bind(session.user_id).execute(&state.db).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/push/device", web::put().to(register)).route("/push/device", web::delete().to(unregister));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_must_match_secure_configured_origin() {
        let origin = "https://push.example.test";
        assert!(validate_endpoint("https://push.example.test/abcdef0123456789abcdef", origin));
        assert!(!validate_endpoint("http://push.example.test/abcdef0123456789abcdef", origin));
        assert!(!validate_endpoint("https://evil.test/abcdef0123456789abcdef", origin));
        assert!(!validate_endpoint("https://push.example.test/abcdef0123456789abcdef?token=x", origin));
    }
}
