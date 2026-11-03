use crate::{
    error::{Error, Result},
    models::ProfileRow,
    state::AppState,
    validation,
};
use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload, web};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Utc};
use futures_util::future::LocalBoxFuture;
use nm_protocol::{
    AuthResponse, LoginRequest, RegisterRequest, ServerEvent, SessionInfo, UserSettings,
};
use rand::{TryRng, rngs::SysRng};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
}

impl FromRequest for Session {
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self>>;
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let state = req.app_data::<web::Data<AppState>>().cloned();
        let token = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_owned);
        Box::pin(async move {
            let state = state.ok_or_else(Error::internal)?;
            let session =
                authenticate(&state, token.as_deref().ok_or_else(Error::unauthorized)?).await?;
            state
                .rate(&format!("http:{}", session.user_id), 600, 60)
                .await?;
            Ok(session)
        })
    }
}

pub fn digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn authenticate(state: &AppState, token: &str) -> Result<Session> {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::unauthorized());
    }
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id,user_id FROM auth_sessions
        WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp()",
    )
    .bind(digest(token))
    .fetch_optional(&state.db)
    .await?;
    let (id, user_id) = row.ok_or_else(Error::unauthorized)?;
    let session = Session { id, user_id };
    valid(state, &session).await?;
    sqlx::query(
        "UPDATE auth_sessions SET last_seen_at=clock_timestamp()
        WHERE id=$1 AND last_seen_at<clock_timestamp()-interval '60 seconds'",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(session)
}

pub async fn valid(state: &AppState, session: &Session) -> Result<()> {
    valid_on(state, &state.db, session).await
}

pub(crate) async fn valid_on<'e>(
    state: &AppState,
    db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    session: &Session,
) -> Result<()> {
    let revoked: bool = redis::cmd("EXISTS")
        .arg(format!("nm:revoked:{}", session.id))
        .query_async(&mut state.redis.clone())
        .await?;
    if revoked {
        return Err(Error::unauthorized());
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.id=s.user_id
        WHERE s.id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND u.disabled_at IS NULL
        AND NOT EXISTS(SELECT 1 FROM bots b JOIN users owner ON owner.id=b.owner_id
            WHERE b.user_id=u.id AND (owner.disabled_at IS NOT NULL OR owner.deleted_at IS NOT NULL)))",
    )
    .bind(session.id)
    .bind(session.user_id)
    .fetch_one(db)
    .await?;
    if !exists {
        return Err(Error::unauthorized());
    }
    Ok(())
}

fn argon() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(19_456, 2, 1, None).expect("constant Argon2 parameters"),
    )
}

static PASSWORD_WORK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub(crate) async fn hash_password(password: String) -> Result<String> {
    let permit = PASSWORD_WORK.try_acquire().map_err(|_| Error::limit())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        argon()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|_| Error::internal())
    })
    .await
    .map_err(|_| Error::internal())?
}

async fn verify_password(password: String, hash: Option<String>) -> Result<bool> {
    let permit = PASSWORD_WORK.try_acquire().map_err(|_| Error::limit())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if let Some(hash) = hash {
            let hash = PasswordHash::new(&hash).map_err(|_| Error::internal())?;
            Ok(argon().verify_password(password.as_bytes(), &hash).is_ok())
        } else {
            // Equal expensive work for nonexistent identities, off the async executor.
            let _ = argon().hash_password(password.as_bytes());
            Ok(false)
        }
    })
    .await
    .map_err(|_| Error::internal())?
}

async fn create_session(
    db: &mut PgConnection,
    user: Uuid,
    device: &str,
    ttl: i64,
) -> Result<(String, Uuid)> {
    let permitted: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND disabled_at IS NULL FOR UPDATE").bind(user).fetch_optional(&mut *db).await?;
    if permitted.is_none() { return Err(Error::unauthorized()); }
    let mut bytes = [0u8; 32];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| Error::internal())?;
    let token = hex::encode(bytes);
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO auth_sessions(id,user_id,token_hash,device_name,expires_at)
         VALUES($1,$2,$3,$4,clock_timestamp()+$5::bigint*interval '1 second')",
    )
    .bind(id)
    .bind(user)
    .bind(digest(&token))
    .bind(device)
    .bind(ttl)
    .execute(db)
    .await?;
    Ok((token, id))
}

async fn auth_ip_rate(state: &AppState, request: &HttpRequest) -> Result<()> {
    let ip = request
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    state.rate(&format!("auth-ip:{ip}"), 30, 60).await
}

pub(crate) async fn confirm_password(state: &AppState, session: &Session, password: &str) -> Result<()> {
    state.rate(&format!("admin-confirm:{}", session.user_id), 8, 60).await?;
    if validation::password(password).is_err() { return Err(Error::unauthorized()); }
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1 AND disabled_at IS NULL").bind(session.user_id).fetch_optional(&state.db).await?;
    if !verify_password(password.to_owned(), hash).await? { return Err(Error::unauthorized()); }
    Ok(())
}

async fn register(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RegisterRequest>,
) -> Result<web::Json<AuthResponse>> {
    auth_ip_rate(&state, &req).await?;
    let email = validation::email(&body.email)?;
    state.rate(&format!("auth-email:{}", digest(&email)), 12, 60).await?;
    if email.ends_with("@bots.nmessenger") {
        return Err(Error::bad("Domínio reservado para bots"));
    }
    validation::password(&body.password)?;
    validation::text(body.display_name.trim(), "Nome", 1, 100)?;
    validation::text(&body.device_name, "Dispositivo", 1, 100)?;
    let password_hash = hash_password(body.password.clone()).await?;
    let mut tx = state.db.begin().await?;
    crate::authz::shared(&mut tx).await?;
    let banned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM banned_emails WHERE email=$1)")
        .bind(&email).fetch_one(&mut *tx).await?;
    if banned { return Err(Error::forbidden()); }
    let user: ProfileRow = sqlx::query_as(
        "INSERT INTO users(id,email,password_hash,display_name,settings)
        VALUES($1,$2,$3,$4,$5) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(password_hash)
    .bind(body.display_name.trim())
    .bind(Json(UserSettings::default()))
    .fetch_one(&mut *tx)
    .await?;
    let first: Option<bool> = sqlx::query_scalar(
        "UPDATE first_account_bootstrap SET claimed=true WHERE singleton AND NOT claimed RETURNING claimed",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if first.is_some() {
        sqlx::query("INSERT INTO app_admins(user_id) VALUES($1)")
            .bind(user.id)
            .execute(&mut *tx)
            .await?;
    }
    let (token, session_id) =
        create_session(&mut tx, user.id, &body.device_name, state.session_ttl_secs).await?;
    let my_badges = crate::badges::for_user_conn(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(web::Json(AuthResponse {
        token,
        session_id,
        user: user.model_with_badges(my_badges)?,
    }))
}

async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LoginRequest>,
) -> Result<web::Json<AuthResponse>> {
    auth_ip_rate(&state, &req).await?;
    let email = validation::email(&body.email)?;
    state.rate(&format!("auth-email:{}", digest(&email)), 12, 60).await?;
    validation::password(&body.password)?;
    validation::text(&body.device_name, "Dispositivo", 1, 100)?;
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT u.id,u.password_hash FROM users u WHERE u.email=$1 AND NOT EXISTS(SELECT 1 FROM bots WHERE user_id=u.id)")
            .bind(&email)
            .fetch_optional(&state.db)
            .await?;
    if !verify_password(body.password.clone(), row.as_ref().map(|r| r.1.clone())).await? {
        return Err(Error::unauthorized());
    }
    let (user_id, verified_hash) = row.ok_or_else(Error::unauthorized)?;
    let mut tx = state.db.begin().await?;
    let current_hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1 FOR UPDATE")
        .bind(user_id).fetch_optional(&mut *tx).await?;
    if current_hash.as_deref() != Some(verified_hash.as_str()) { return Err(Error::unauthorized()); }
    let (token, session_id) =
        create_session(&mut tx, user_id, &body.device_name, state.session_ttl_secs).await?;
    let user: ProfileRow = sqlx::query_as("SELECT * FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    let my_badges = crate::badges::for_user_conn(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(web::Json(AuthResponse {
        token,
        session_id,
        user: user.model_with_badges(my_badges)?,
    }))
}

pub async fn list(state: &AppState, session: &Session) -> Result<Vec<SessionInfo>> {
    let rows: Vec<(Uuid, String, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id,device_name,created_at,last_seen_at FROM auth_sessions WHERE user_id=$1
        AND revoked_at IS NULL AND expires_at>clock_timestamp() ORDER BY created_at DESC",
    )
    .bind(session.user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, device_name, created_at, last_seen_at)| SessionInfo {
            id,
            device_name,
            created_at,
            last_seen_at,
            current: id == session.id,
        })
        .collect())
}

async fn sessions(
    state: web::Data<AppState>,
    session: Session,
) -> Result<web::Json<Vec<SessionInfo>>> {
    Ok(web::Json(list(&state, &session).await?))
}

async fn revoke(state: &AppState, session: &Session, id: Uuid) -> Result<()> {
    let expires: Option<DateTime<Utc>> = sqlx::query_scalar("UPDATE auth_sessions
        SET revoked_at=COALESCE(revoked_at,clock_timestamp()) WHERE id=$1 AND user_id=$2 RETURNING expires_at")
        .bind(id).bind(session.user_id).fetch_optional(&state.db).await?;
    let expires = expires.ok_or_else(Error::missing)?;
    let ttl = (expires - Utc::now()).num_seconds().max(1);
    let _: () = redis::cmd("SET")
        .arg(format!("nm:revoked:{id}"))
        .arg("1")
        .arg("EX")
        .arg(ttl)
        .query_async(&mut state.redis.clone())
        .await?;
    state
        .publish(
            vec![session.user_id],
            ServerEvent::SessionRevoked { session_id: id },
        )
        .await;
    Ok(())
}

async fn logout(state: web::Data<AppState>, session: Session) -> Result<HttpResponse> {
    revoke(&state, &session, session.id).await?;
    Ok(HttpResponse::NoContent().finish())
}
async fn delete_session(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    revoke(&state, &session, *id).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/auth/register", web::post().to(register))
        .route("/auth/login", web::post().to(login))
        .route("/auth/logout", web::post().to(logout))
        .route("/sessions", web::get().to(sessions))
        .route("/sessions/{id}", web::delete().to(delete_session));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn argon2id_and_token_digest() {
        let hash = hash_password("password with spaces".into()).await.unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(
            verify_password("password with spaces".into(), Some(hash.clone()))
                .await
                .unwrap()
        );
        assert!(
            !verify_password("wrong password".into(), Some(hash))
                .await
                .unwrap()
        );
        assert_eq!(
            digest("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
