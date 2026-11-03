//! Bot accounts use the same encrypted message protocol as other participants.
use crate::{auth::{self, Session}, authz, error::{Error, Result}, state::AppState, validation};
use actix_web::{web, HttpResponse};
use nm_protocol::{ServerEvent, UserSettings};
use rand::{rngs::SysRng, TryRng};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Deserialize)]
struct CreateBot { username: String, name: String }
#[derive(Deserialize)]
struct UpdateBot { name: Option<String> }
#[derive(Serialize)]
struct Bot { id: Uuid, username: String, name: String, address: String }
#[derive(Serialize)]
struct CreatedBot { bot: Bot, token: String }

fn validate_username(value: &str) -> Result<String> {
    let value = value.trim().trim_start_matches('@').to_ascii_lowercase();
    if !(3..=32).contains(&value.len()) || !value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') || value == "bot" {
        return Err(Error::bad("Identificador deve ter 3–32 letras, números ou sublinhados"));
    }
    Ok(value)
}

async fn create(state: web::Data<AppState>, session: Session, body: web::Json<CreateBot>) -> Result<web::Json<CreatedBot>> {
    let username = validate_username(&body.username)?;
    validation::text(body.name.trim(), "Nome", 1, 100)?;
    state.rate(&format!("bot-create:{}", session.user_id), 10, 3600).await?;
    let mut bytes = [0u8; 32];
    SysRng.try_fill_bytes(&mut bytes).map_err(|_| Error::internal())?;
    let token = hex::encode(bytes);
    let password_hash = auth::hash_password(token.clone()).await?;
    let id = Uuid::new_v4();
    let address = format!("{username}@bots.nmessenger");
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    auth::valid_on(&state, &mut *tx, &session).await?;
    let owner_is_bot: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE user_id=$1)").bind(session.user_id).fetch_one(&mut *tx).await?;
    if owner_is_bot { return Err(Error::forbidden()); }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM bots WHERE owner_id=$1").bind(session.user_id).fetch_one(&mut *tx).await?;
    if count >= 20 { return Err(Error::limit()); }
    let inserted = sqlx::query("INSERT INTO users(id,email,password_hash,display_name,settings) VALUES($1,$2,$3,$4,$5) ON CONFLICT(email) DO NOTHING")
        .bind(id).bind(&address).bind(password_hash).bind(body.name.trim()).bind(Json(UserSettings::default())).execute(&mut *tx).await?;
    if inserted.rows_affected() == 0 { return Err(Error::conflict("Identificador já está em uso")); }
    sqlx::query("INSERT INTO bots(user_id,owner_id,username) VALUES($1,$2,$3)").bind(id).bind(session.user_id).bind(&username).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_hash,device_name,expires_at) VALUES($1,$2,$3,'API do bot',clock_timestamp()+interval '10 years')")
        .bind(Uuid::new_v4()).bind(id).bind(auth::digest(&token)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(web::Json(CreatedBot { bot: Bot { id, username, name: body.name.trim().into(), address }, token }))
}

async fn list(state: web::Data<AppState>, session: Session) -> Result<web::Json<Vec<Bot>>> {
    let rows: Vec<(Uuid,String,String,String)> = sqlx::query_as("SELECT b.user_id,b.username,u.display_name,u.email FROM bots b JOIN users u ON u.id=b.user_id WHERE b.owner_id=$1 AND u.deleted_at IS NULL ORDER BY b.created_at,b.user_id")
        .bind(session.user_id).fetch_all(&state.db).await?;
    Ok(web::Json(rows.into_iter().map(|(id,username,name,address)| Bot { id, username, name, address }).collect()))
}

async fn update(state: web::Data<AppState>, session: Session, id: web::Path<Uuid>, body: web::Json<UpdateBot>) -> Result<HttpResponse> {
    let name = body.name.as_deref().ok_or_else(|| Error::bad("Nenhuma alteração informada"))?.trim();
    validation::text(name, "Nome", 1, 100)?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    auth::valid_on(&state, &mut *tx, &session).await?;
    let result = sqlx::query("UPDATE users SET display_name=$3 WHERE id=$1 AND deleted_at IS NULL AND EXISTS(SELECT 1 FROM bots WHERE user_id=$1 AND owner_id=$2)")
        .bind(*id).bind(session.user_id).bind(name).execute(&mut *tx).await?;
    if result.rows_affected() == 0 { return Err(Error::missing()); }
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Serialize)]
struct BotToken { token: String }

async fn rotate_token(state: web::Data<AppState>, session: Session, id: web::Path<Uuid>) -> Result<web::Json<BotToken>> {
    state.rate(&format!("bot-token:{}", session.user_id), 10, 3600).await?;
    let mut bytes = [0u8; 32];
    SysRng.try_fill_bytes(&mut bytes).map_err(|_| Error::internal())?;
    let token = hex::encode(bytes);
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    auth::valid_on(&state, &mut *tx, &session).await?;
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots b JOIN users u ON u.id=b.user_id WHERE b.user_id=$1 AND b.owner_id=$2 AND u.deleted_at IS NULL AND u.disabled_at IS NULL)")
        .bind(*id).bind(session.user_id).fetch_one(&mut *tx).await?;
    if !owned { return Err(Error::missing()); }
    let revoked: Vec<Uuid> = sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL RETURNING id")
        .bind(*id).fetch_all(&mut *tx).await?;
    sqlx::query("INSERT INTO auth_sessions(id,user_id,token_hash,device_name,expires_at) VALUES($1,$2,$3,'API do bot',clock_timestamp()+interval '10 years')")
        .bind(Uuid::new_v4()).bind(*id).bind(auth::digest(&token)).execute(&mut *tx).await?;
    tx.commit().await?;
    for session_id in revoked { state.publish(vec![*id], ServerEvent::SessionRevoked { session_id }).await; }
    Ok(web::Json(BotToken { token }))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/bots", web::post().to(create))
        .route("/bots", web::get().to(list))
        .route("/bots/{id}", web::patch().to(update))
        .route("/bots/{id}/token", web::post().to(rotate_token));
}
