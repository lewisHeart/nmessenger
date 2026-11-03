use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    state::AppState,
};
use actix_web::web;
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::{EncryptionKey, RegisterEncryptionKey};
use uuid::Uuid;

async fn register(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<RegisterEncryptionKey>,
) -> Result<web::Json<EncryptionKey>> {
    if body.device_id.is_nil() {
        return Err(Error::bad("Dispositivo inválido"));
    }
    for key in [&body.public_key, &body.signing_key] {
        let decoded = STANDARD
            .decode(key)
            .map_err(|_| Error::bad("Chave pública inválida"))?;
        if key.len() != 44 || decoded.len() != 32 || decoded.iter().all(|byte| *byte == 0) {
            return Err(Error::bad("Chave pública inválida"));
        }
    }
    state
        .rate(&format!("encryption-key:{}", session.user_id), 10, 60)
        .await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let associated: Option<Uuid> =
        sqlx::query_scalar("SELECT device_id FROM encryption_key_sessions WHERE session_id=$1")
            .bind(session.id)
            .fetch_optional(&mut *tx)
            .await?;
    if associated.is_some_and(|device| device != body.device_id) {
        return Err(Error::conflict(
            "Esta sessão já possui uma identidade criptográfica",
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM encryption_keys WHERE user_id=$1")
        .bind(session.user_id)
        .fetch_one(&mut *tx)
        .await?;
    let existing: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT user_id,public_key,signing_key FROM encryption_keys WHERE device_id=$1",
    )
    .bind(body.device_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((user, public, signing)) = existing {
        if user != session.user_id || public != body.public_key || signing != body.signing_key {
            return Err(Error::conflict(
                "A identidade criptográfica do dispositivo não pode ser substituída",
            ));
        }
    } else {
        if count >= 16 {
            return Err(Error::limit());
        }
        sqlx::query("INSERT INTO encryption_keys(device_id,user_id,public_key,signing_key) VALUES($1,$2,$3,$4)").bind(body.device_id).bind(session.user_id).bind(&body.public_key).bind(&body.signing_key).execute(&mut *tx).await?;
    }
    sqlx::query("INSERT INTO encryption_key_sessions(device_id,session_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(body.device_id).bind(session.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(web::Json(EncryptionKey {
        user_id: session.user_id,
        device_id: body.device_id,
        public_key: body.public_key.clone(),
        signing_key: body.signing_key.clone(),
        active: true,
    }))
}

pub async fn keys_on(
    db: &mut sqlx::PgConnection,
    conversation: Uuid,
) -> Result<Vec<EncryptionKey>> {
    let rows: Vec<(Uuid,Uuid,String,String,bool)> = sqlx::query_as("SELECT k.user_id,k.device_id,k.public_key,k.signing_key,EXISTS(SELECT 1 FROM encryption_key_sessions d JOIN auth_sessions s ON s.id=d.session_id WHERE d.device_id=k.device_id AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp()) AS active FROM encryption_keys k JOIN conversation_members m ON m.user_id=k.user_id WHERE m.conversation_id=$1 ORDER BY k.user_id,k.device_id").bind(conversation).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(
            |(user_id, device_id, public_key, signing_key, active)| EncryptionKey {
                user_id,
                device_id,
                public_key,
                signing_key,
                active,
            },
        )
        .collect())
}

async fn keys(
    state: web::Data<AppState>,
    session: Session,
    conversation: web::Path<Uuid>,
) -> Result<web::Json<Vec<EncryptionKey>>> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::member(&mut tx, session.user_id, *conversation).await?;
    let keys = keys_on(&mut tx, *conversation).await?;
    tx.commit().await?;
    Ok(web::Json(keys))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/encryption/keys", web::post().to(register))
        .route("/conversations/{id}/encryption/keys", web::get().to(keys))
        .route(
            "/activities/{id}/encryption/keys",
            web::get().to(activity_keys),
        );
}

async fn activity_keys(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<web::Json<Vec<EncryptionKey>>> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let (activity, participants) = crate::activities::active(&mut tx, session.user_id, *id).await?;
    let keys: Vec<_> = keys_on(&mut tx, activity.conversation_id)
        .await?
        .into_iter()
        .filter(|key| participants.contains(&key.user_id))
        .collect();
    if participants
        .iter()
        .any(|user| !keys.iter().any(|key| key.user_id == *user && key.active))
    {
        return Err(Error::bad(
            "Participante sem dispositivo criptográfico ativo",
        ));
    }
    tx.commit().await?;
    Ok(web::Json(keys))
}

pub async fn route(
    db: &mut sqlx::PgConnection,
    session: &Session,
    conversation: Uuid,
    participants: &[Uuid],
    envelope: &nm_protocol::EncryptedMessage,
) -> Result<()> {
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys k JOIN encryption_key_sessions d ON d.device_id=k.device_id WHERE k.device_id=$1 AND k.user_id=$2 AND d.session_id=$3)").bind(envelope.sender_device).bind(session.user_id).bind(session.id).fetch_one(&mut *db).await?;
    if !owned {
        return Err(Error::forbidden());
    }
    let keys: Vec<_> = keys_on(db, conversation)
        .await?
        .into_iter()
        .filter(|key| participants.contains(&key.user_id))
        .collect();
    if participants
        .iter()
        .any(|user| !keys.iter().any(|key| key.user_id == *user && key.active))
    {
        return Err(Error::bad(
            "Participante sem dispositivo criptográfico ativo",
        ));
    }
    let expected: std::collections::HashSet<_> = keys
        .iter()
        .filter(|key| key.active)
        .map(|key| key.device_id)
        .collect();
    let supplied: std::collections::HashSet<_> = envelope
        .recipients
        .iter()
        .map(|key| key.device_id)
        .collect();
    if expected != supplied {
        return Err(Error::conflict(
            "A lista de dispositivos mudou; atualize a conversa",
        ));
    }
    Ok(())
}
