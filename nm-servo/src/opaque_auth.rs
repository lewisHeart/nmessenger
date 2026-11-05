//! Password authentication and client-only recovery keys using OPAQUE.
use crate::{auth::{self, Session}, error::{Error, Result}, models::ProfileRow, state::AppState, validation};
use actix_web::{web, HttpRequest, HttpResponse};
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::{AuthResponse, secure_auth::{CipherSuite, opaque_ke}};
use opaque_ke::{CredentialFinalization, CredentialRequest, RegistrationRequest, RegistrationUpload,
    ServerLogin, ServerLoginStartParameters, ServerRegistration, ServerSetup};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
struct RegistrationStart { request: String }
#[derive(Deserialize)]
struct RegistrationFinish { record: String }
#[derive(Serialize)]
struct RegistrationResponse { response: String }
#[derive(Deserialize)]
struct LoginStart { email: String, device_name: String, request: String }
#[derive(Serialize)]
struct LoginResponse { challenge: Uuid, response: String }
#[derive(Deserialize)]
struct LoginFinish { challenge: Uuid, finalization: String }
#[derive(Serialize, Deserialize)]
struct Pending { user: Option<Uuid>, state: String, device: String }

fn decode(input: &str) -> Result<Vec<u8>> {
    if input.len() > 4096 { return Err(Error::bad("Dados de autenticação inválidos")); }
    STANDARD.decode(input).map_err(|_| Error::bad("Dados de autenticação inválidos"))
}

async fn setup(state: &AppState) -> Result<ServerSetup<CipherSuite>> {
    let generated = ServerSetup::<CipherSuite>::new(&mut opaque_ke::rand::rngs::OsRng).serialize().to_vec();
    let stored: Vec<u8> = sqlx::query_scalar("INSERT INTO opaque_server_setup(singleton,setup) VALUES(true,$1) ON CONFLICT(singleton) DO UPDATE SET setup=opaque_server_setup.setup RETURNING setup")
        .bind(generated).fetch_one(&state.db).await?;
    ServerSetup::deserialize(&stored).map_err(|_| Error::internal())
}

async fn registration_start(state: web::Data<AppState>, session: Session, body: web::Json<RegistrationStart>) -> Result<web::Json<RegistrationResponse>> {
    state.rate(&format!("opaque-register:{}", session.user_id), 10, 60).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM opaque_credentials WHERE user_id=$1)").bind(session.user_id).fetch_one(&state.db).await?;
    if exists { return Err(Error::conflict("Autenticação segura já cadastrada")); }
    let request = RegistrationRequest::<CipherSuite>::deserialize(&decode(&body.request)?).map_err(|_| Error::bad("Dados de autenticação inválidos"))?;
    let response = ServerRegistration::<CipherSuite>::start(&setup(&state).await?, request, session.user_id.as_bytes()).map_err(|_| Error::bad("Dados de autenticação inválidos"))?;
    Ok(web::Json(RegistrationResponse { response: STANDARD.encode(response.message.serialize()) }))
}

async fn registration_finish(state: web::Data<AppState>, session: Session, body: web::Json<RegistrationFinish>) -> Result<HttpResponse> {
    state.rate(&format!("opaque-register-finish:{}", session.user_id), 10, 60).await?;
    let upload = RegistrationUpload::<CipherSuite>::deserialize(&decode(&body.record)?).map_err(|_| Error::bad("Dados de autenticação inválidos"))?;
    let record = ServerRegistration::<CipherSuite>::finish(upload).serialize();
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE").bind(session.user_id).fetch_one(&mut *tx).await?;
    let inserted = sqlx::query("INSERT INTO opaque_credentials(user_id,record) VALUES($1,$2) ON CONFLICT DO NOTHING")
        .bind(session.user_id).bind(record.as_slice()).execute(&mut *tx).await?.rows_affected();
    if inserted == 0 { return Err(Error::conflict("Autenticação segura já cadastrada")); }
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn login_start(state: web::Data<AppState>, request: HttpRequest, body: web::Json<LoginStart>) -> Result<web::Json<LoginResponse>> {
    auth::auth_ip_rate(&state, &request).await?;
    let email = validation::email(&body.email)?;
    state.rate(&format!("auth-email:{}", auth::digest(&email)), 12, 60).await?;
    validation::text(&body.device_name, "Dispositivo", 1, 100)?;
    let input = CredentialRequest::<CipherSuite>::deserialize(&decode(&body.request)?).map_err(|_| Error::bad("Dados de autenticação inválidos"))?;
    let row: Option<(Uuid, Option<Vec<u8>>)> = sqlx::query_as("SELECT u.id,c.record FROM users u LEFT JOIN opaque_credentials c ON c.user_id=u.id WHERE u.email=$1 AND u.disabled_at IS NULL AND NOT EXISTS(SELECT 1 FROM bots WHERE user_id=u.id)")
        .bind(email).fetch_optional(&state.db).await?;
    if row.as_ref().is_some_and(|row| row.1.is_none()) {
        return Err(Error::new(actix_web::http::StatusCode::CONFLICT, "legacy_auth", "Conta ainda não migrou para autenticação segura"));
    }
    let user = row.as_ref().map(|row| row.0);
    let file = row.as_ref().and_then(|row| row.1.as_ref()).map(|record| ServerRegistration::<CipherSuite>::deserialize(record)).transpose().map_err(|_| Error::internal())?;
    let identifier = user.unwrap_or(Uuid::nil());
    let login = ServerLogin::start(&mut opaque_ke::rand::rngs::OsRng, &setup(&state).await?, file, input, identifier.as_bytes(), ServerLoginStartParameters::default()).map_err(|_| Error::unauthorized())?;
    let challenge = Uuid::new_v4();
    let pending = Pending { user, state: STANDARD.encode(login.state.serialize()), device: body.device_name.clone() };
    let value = serde_json::to_string(&pending).map_err(|_| Error::internal())?;
    let _: () = redis::cmd("SET").arg(format!("nm:opaque:{challenge}")).arg(value).arg("EX").arg(90)
        .query_async(&mut state.redis.clone()).await?;
    Ok(web::Json(LoginResponse { challenge, response: STANDARD.encode(login.message.serialize()) }))
}

async fn login_finish(state: web::Data<AppState>, body: web::Json<LoginFinish>) -> Result<web::Json<AuthResponse>> {
    let pending: Option<String> = redis::cmd("GETDEL").arg(format!("nm:opaque:{}", body.challenge)).query_async(&mut state.redis.clone()).await?;
    let pending: Pending = serde_json::from_str(&pending.ok_or_else(Error::unauthorized)?).map_err(|_| Error::unauthorized())?;
    let login = ServerLogin::<CipherSuite>::deserialize(&decode(&pending.state)?).map_err(|_| Error::unauthorized())?;
    let finalization = CredentialFinalization::<CipherSuite>::deserialize(&decode(&body.finalization)?).map_err(|_| Error::unauthorized())?;
    login.finish(finalization).map_err(|_| Error::unauthorized())?;
    let user_id = pending.user.ok_or_else(Error::unauthorized)?;
    let mut tx = state.db.begin().await?;
    let (token, session_id) = auth::create_session(&mut tx, user_id, &pending.device, state.session_ttl_secs).await?;
    let user: ProfileRow = sqlx::query_as("SELECT * FROM users WHERE id=$1").bind(user_id).fetch_one(&mut *tx).await?;
    let badges = crate::badges::for_user_conn(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(web::Json(AuthResponse { token, session_id, user: user.model_with_badges(badges)? }))
}

async fn recovery_start(state: web::Data<AppState>, session: Session, body: web::Json<RegistrationStart>) -> Result<web::Json<RegistrationResponse>> {
    state.rate(&format!("opaque-recovery:{}", session.user_id), 10, 60).await?;
    let record: Vec<u8> = sqlx::query_scalar("SELECT record FROM opaque_recovery_credentials WHERE user_id=$1")
        .bind(session.user_id).fetch_optional(&state.db).await?.ok_or_else(Error::missing)?;
    let file = ServerRegistration::<CipherSuite>::deserialize(&record).map_err(|_| Error::internal())?;
    let request = CredentialRequest::<CipherSuite>::deserialize(&decode(&body.request)?).map_err(|_| Error::bad("Dados de autenticação inválidos"))?;
    let login = ServerLogin::start(&mut opaque_ke::rand::rngs::OsRng, &setup(&state).await?, Some(file), request, session.user_id.as_bytes(), ServerLoginStartParameters::default())
        .map_err(|_| Error::unauthorized())?;
    Ok(web::Json(RegistrationResponse { response: STANDARD.encode(login.message.serialize()) }))
}

async fn recovery_complete(state: web::Data<AppState>, session: Session) -> Result<HttpResponse> {
    // Only allow cleanup once a new secure credential and a backup exist.
    sqlx::query("DELETE FROM opaque_recovery_credentials WHERE user_id=$1 AND EXISTS(SELECT 1 FROM opaque_credentials WHERE user_id=$1) AND EXISTS(SELECT 1 FROM encrypted_history_backups WHERE user_id=$1)")
        .bind(session.user_id).execute(&state.db).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/auth/opaque/register/start", web::post().to(registration_start))
        .route("/auth/opaque/register/finish", web::post().to(registration_finish))
        .route("/auth/opaque/login/start", web::post().to(login_start))
        .route("/auth/opaque/login/finish", web::post().to(login_finish))
        .route("/auth/opaque/recovery/start", web::post().to(recovery_start))
        .route("/auth/opaque/recovery/complete", web::post().to(recovery_complete));
}
