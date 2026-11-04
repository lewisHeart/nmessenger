//! Per-session push registration for a configured UnifiedPush distributor.
use crate::{auth::Session, error::{Error, Result}, state::AppState};
use actix_web::{web, HttpResponse};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn enqueue(db: &mut PgConnection, recipients: &[Uuid], message: Uuid) -> Result<()> {
    sqlx::query("INSERT INTO push_queue(message_id,session_id) SELECT $1,p.session_id FROM push_devices p JOIN auth_sessions s ON s.id=p.session_id JOIN users u ON u.id=p.user_id WHERE p.user_id=ANY($2) AND s.revoked_at IS NULL AND s.expires_at>now() AND COALESCE((u.settings->>'message_alerts')::boolean,true) ON CONFLICT DO NOTHING")
        .bind(message).bind(recipients).execute(db).await?;
    Ok(())
}

pub async fn run(state: AppState) {
    let Ok(client) = reqwest::Client::builder().timeout(std::time::Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none()).build() else { return; };
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        interval.tick().await;
        if let Err(error) = dispatch(&state, &client).await {
            tracing::warn!(code = error.code, "push queue delivery failed; will retry");
        }
    }
}

async fn dispatch(state: &AppState, client: &reqwest::Client) -> Result<()> {
    let mut tx = state.db.begin().await?;
    let rows: Vec<(Uuid, Uuid, String, String, String, Uuid)> = sqlx::query_as(
        "SELECT q.message_id,q.session_id,p.endpoint,p.public_key,p.auth_secret,m.conversation_id
         FROM push_queue q JOIN push_devices p ON p.session_id=q.session_id
         JOIN auth_sessions s ON s.id=p.session_id JOIN messages m ON m.id=q.message_id
         JOIN conversation_members cm ON cm.user_id=p.user_id AND cm.conversation_id=m.conversation_id
         WHERE q.next_attempt_at<=now() AND s.revoked_at IS NULL AND s.expires_at>now()
         ORDER BY q.created_at LIMIT 10 FOR UPDATE OF q SKIP LOCKED")
        .fetch_all(&mut *tx).await?;
    for (message, session, endpoint, key, secret, conversation) in rows {
        let trusted = std::env::var("NTFY_ORIGIN").ok().is_some_and(|origin| validate_endpoint(&endpoint, &origin));
        if !trusted {
            sqlx::query("DELETE FROM push_queue WHERE message_id=$1 AND session_id=$2").bind(message).bind(session).execute(&mut *tx).await?;
            continue;
        }
        let info = web_push::SubscriptionInfo::new(&endpoint, &key, &secret);
        let mut builder = web_push::WebPushMessageBuilder::new(&info);
        builder.set_ttl(3600);
        let payload = serde_json::json!({"type":"message","conversation_id":conversation,"message_id":message}).to_string();
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload.as_bytes());
        let delivered = if let Ok(message) = builder.build() {
            let request = web_push::request_builder::build_request::<Vec<u8>>(message);
            let (parts, body) = request.into_parts();
            let mut request = client.post(parts.uri.to_string()).body(body);
            for (name, value) in &parts.headers {
                if let (Ok(name), Ok(value)) = (reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()), reqwest::header::HeaderValue::from_bytes(value.as_bytes())) {
                    request = request.header(name, value);
                }
            }
            if let Ok(token) = std::env::var("NTFY_TOKEN") { if !token.is_empty() { request = request.bearer_auth(token); } }
            request.send().await.is_ok_and(|response| response.status().is_success())
        } else { false };
        if delivered {
            sqlx::query("DELETE FROM push_queue WHERE message_id=$1 AND session_id=$2").bind(message).bind(session).execute(&mut *tx).await?;
        } else {
            sqlx::query("UPDATE push_queue SET attempts=attempts+1,next_attempt_at=now()+make_interval(secs=>LEAST(300,power(2,LEAST(attempts,8))::int)) WHERE message_id=$1 AND session_id=$2")
                .bind(message).bind(session).execute(&mut *tx).await?;
        }
    }
    sqlx::query("DELETE FROM push_queue q WHERE q.created_at<now()-interval '24 hours' OR NOT EXISTS(SELECT 1 FROM push_devices p JOIN auth_sessions s ON s.id=p.session_id JOIN messages m ON m.id=q.message_id JOIN conversation_members cm ON cm.user_id=p.user_id AND cm.conversation_id=m.conversation_id WHERE p.session_id=q.session_id AND s.revoked_at IS NULL AND s.expires_at>now())")
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

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
    #[test]
    fn web_push_payload_does_not_expose_notification_metadata() {
        let info = web_push::SubscriptionInfo::new(
            "https://push.example.test/abcdefghijk123456789",
            "BGa4N1PI79lboMR_YrwCiCsgp35DRvedt7opHcf0yM3iOBTSoQYqQLwWxAfRKE6tsDnReWmhsImkhDF_DBdkNSU",
            "EvcWjEgzr4rbvhfi3yds0A",
        );
        let mut builder = web_push::WebPushMessageBuilder::new(&info);
        let secret = b"conversation_id";
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, secret);
        let request = web_push::request_builder::build_request::<Vec<u8>>(builder.build().unwrap());
        assert!(!request.body().windows(secret.len()).any(|window| window == secret));
    }
}
