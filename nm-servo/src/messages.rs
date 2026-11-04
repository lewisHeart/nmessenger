use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    files,
    models::MessageRow,
    state::AppState,
    validation,
};
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use nm_protocol::*;
use serde::Deserialize;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Deserialize)]
struct HistoryQuery {
    before: Option<Uuid>,
    q: Option<String>,
    limit: Option<i64>,
}

async fn history(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    query: web::Query<HistoryQuery>,
) -> Result<web::Json<Vec<Message>>> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(Error::bad("Limite deve ser 1–100"));
    }
    if let Some(q) = &query.q {
        validation::text(q, "Busca", 0, 200)?;
    }
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::member(&mut tx, session.user_id, *id).await?;
    let before: Option<(DateTime<Utc>, Uuid)> = if let Some(cursor) = query.before {
        Some(sqlx::query_as("SELECT m.created_at,m.id FROM messages m
            JOIN conversation_members mine ON mine.conversation_id=m.conversation_id AND mine.user_id=$3
            WHERE m.id=$1 AND m.conversation_id=$2 AND m.created_at>mine.hidden_at
            AND (NOT EXISTS(SELECT 1 FROM bots WHERE user_id=$3) OR m.created_at>=mine.joined_at)")
            .bind(cursor).bind(*id).bind(session.user_id).fetch_optional(&mut *tx).await?.ok_or_else(Error::missing)?)
    } else {
        None
    };
    let rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT m.* FROM messages m
        JOIN conversation_members mine ON mine.conversation_id=m.conversation_id AND mine.user_id=$2
        WHERE m.conversation_id=$1 AND m.created_at>mine.hidden_at
        AND (NOT EXISTS(SELECT 1 FROM bots WHERE user_id=$2) OR m.created_at>=mine.joined_at)
        AND ($3::timestamptz IS NULL OR (m.created_at,m.id)<($3,$4))
        AND ($5::text IS NULL OR strpos(lower(m.body),lower($5))>0)
        ORDER BY m.created_at DESC,m.id DESC LIMIT $6",
    )
    .bind(*id)
    .bind(session.user_id)
    .bind(before.map(|b| b.0))
    .bind(before.map(|b| b.1))
    .bind(&query.q)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(web::Json(
        rows.into_iter()
            .rev()
            .map(MessageRow::model)
            .collect::<Result<_>>()?,
    ))
}

fn validate_message(body: &SendMessageRequest) -> Result<()> {
    if matches!(body.kind, MessageKind::System | MessageKind::Activity) {
        return Err(Error::bad("Tipo de mensagem reservado ao servidor"));
    }
    if body.client_id.is_nil() {
        return Err(Error::bad("client_id inválido"));
    }
    if body.body.starts_with(ENCRYPTED_PREFIX) {
        validate_envelope(&body.body, MAX_ENCRYPTED_BODY)?;
    } else {
        validation::text(&body.body, "Mensagem", 0, MAX_MESSAGE_CHARS)?;
    }
    validation::style(&body.style)?;
    if body.kind == MessageKind::Text && body.body.trim().is_empty() {
        return Err(Error::bad("Mensagem vazia"));
    }
    if matches!(
        body.kind,
        MessageKind::File | MessageKind::Photo | MessageKind::Voice
    ) && body.attachment_id.is_none()
    {
        return Err(Error::bad("Anexo obrigatório"));
    }
    if body.kind == MessageKind::Ink && body.attachment_id.is_none() && body.body.trim().is_empty()
    {
        return Err(Error::bad("Desenho vazio"));
    }
    Ok(())
}

pub(crate) fn validate_envelope(body: &str, limit: usize) -> Result<EncryptedMessage> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    if body.len() > limit {
        return Err(Error::too_large());
    }
    let envelope: EncryptedMessage = serde_json::from_str(
        body.strip_prefix(ENCRYPTED_PREFIX)
            .ok_or_else(|| Error::bad("Envelope criptográfico ausente"))?,
    )
    .map_err(|_| Error::bad("Envelope criptográfico inválido"))?;
    let valid =
        |text: &str, size: usize| STANDARD.decode(text).is_ok_and(|bytes| bytes.len() == size);
    let unique: std::collections::HashSet<_> = envelope
        .recipients
        .iter()
        .map(|key| key.device_id)
        .collect();
    if envelope.sender_device.is_nil()
        || !valid(&envelope.nonce, 24)
        || !valid(&envelope.signature, 64)
        || envelope.recipients.is_empty()
        || envelope.recipients.len() > 256
        || unique.len() != envelope.recipients.len()
        || envelope.recipients.iter().any(|key| {
            key.device_id.is_nil() || !valid(&key.nonce, 24) || !valid(&key.ciphertext, 48)
        })
        || !STANDARD
            .decode(&envelope.ciphertext)
            .is_ok_and(|bytes| bytes.len() >= 16)
    {
        return Err(Error::bad("Envelope criptográfico inválido"));
    }
    Ok(envelope)
}

async fn send(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<SendMessageRequest>,
) -> Result<web::Json<Message>> {
    validate_message(&body)?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::lock_conversation(&mut tx, *id).await?;
    let members = authz::member(&mut tx, session.user_id, *id).await?;
    let peers: Vec<_> = members
        .iter()
        .copied()
        .filter(|u| *u != session.user_id)
        .collect();
    authz::peers(&mut tx, session.user_id, &peers).await?;
    let enrolled: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys WHERE user_id=$1) OR EXISTS(SELECT 1 FROM bots WHERE user_id=ANY($2))")
            .bind(session.user_id)
            .bind(&members)
            .fetch_one(&mut *tx)
            .await?;
    if enrolled && !body.body.starts_with(ENCRYPTED_PREFIX) {
        return Err(Error::bad(
            "Este remetente exige criptografia ponta a ponta",
        ));
    }
    if body.body.starts_with(ENCRYPTED_PREFIX) {
        let envelope = validate_envelope(&body.body, MAX_ENCRYPTED_BODY)?;
        let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys k JOIN encryption_key_sessions d ON d.device_id=k.device_id WHERE k.device_id=$1 AND k.user_id=$2 AND d.session_id=$3)").bind(envelope.sender_device).bind(session.user_id).bind(session.id).fetch_one(&mut *tx).await?;
        if !owned {
            return Err(Error::forbidden());
        }
        let keys = crate::encryption::keys_on(&mut tx, *id).await?;
        if members
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
                "A lista de dispositivos mudou; atualize a conversa antes de enviar",
            ));
        }
    }
    // Serializes retries even when a client_id is reused across conversations.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("{}:{}", session.user_id, body.client_id))
        .execute(&mut *tx)
        .await?;
    if let Some(existing) = sqlx::query_as::<_, MessageRow>(
        "SELECT * FROM messages WHERE sender_id=$1 AND client_id=$2",
    )
    .bind(session.user_id)
    .bind(body.client_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        if existing.conversation_id != *id {
            return Err(Error::conflict("client_id pertence a outra conversa"));
        }
        tx.commit().await?;
        return Ok(web::Json(existing.model()?));
    }
    state
        .rate(&format!("send:{}", session.user_id), 60, 10)
        .await?;
    if let Some(file) = body.attachment_id {
        let attachment = files::attachment(&state, &mut tx, session.user_id, file, *id).await?;
        if attachment.conversation_id != Some(*id) || attachment.owner_id != session.user_id {
            return Err(Error::forbidden());
        }
        match body.kind {
            MessageKind::Photo | MessageKind::Ink
                if !attachment.mime.starts_with("image/")
                    && attachment.mime != "application/vnd.nmessenger.encrypted" =>
            {
                return Err(Error::bad("Anexo deve ser imagem"));
            }
            MessageKind::Voice
                if !attachment.mime.starts_with("audio/")
                    && attachment.mime != "application/vnd.nmessenger.encrypted" =>
            {
                return Err(Error::bad("Anexo deve ser áudio"));
            }
            _ => {}
        }
    }
    if body.kind == MessageKind::Nudge {
        let disabled: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=ANY($1)
            AND NOT COALESCE((settings->>'nudges')::boolean,true))",
        )
        .bind(&peers)
        .fetch_one(&mut *tx)
        .await?;
        if disabled {
            return Err(Error::forbidden());
        }
        state
            .cooldown(
                &format!("nudge:{}:{}", session.user_id, *id),
                NUDGE_COOLDOWN_SECS,
            )
            .await?;
    }
    let row: MessageRow = sqlx::query_as(
        "INSERT INTO messages(id,conversation_id,sender_id,client_id,kind,body,style,attachment_id)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(*id)
    .bind(session.user_id)
    .bind(body.client_id)
    .bind(validation::wire(body.kind)?)
    .bind(&body.body)
    .bind(Json(&body.style))
    .bind(body.attachment_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE conversations SET updated_at=$2 WHERE id=$1")
        .bind(*id)
        .bind(row.created_at)
        .execute(&mut *tx)
        .await?;
    let message = row.model()?;
    crate::outbox::enqueue(&mut tx, &members, &message).await?;
    crate::push::enqueue(&mut tx, &peers, message.id).await?;
    tx.commit().await?;
    Ok(web::Json(message))
}

async fn read(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<ReadRequest>,
) -> Result<HttpResponse> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::lock_conversation(&mut tx, *id).await?;
    let members = authz::member(&mut tx, session.user_id, *id).await?;
    let marker: Option<(DateTime<Utc>, Uuid)> = sqlx::query_as(
        "SELECT m.created_at,m.id FROM messages m
        JOIN conversation_members mine ON mine.conversation_id=m.conversation_id AND mine.user_id=$2
        WHERE m.conversation_id=$1 AND m.created_at>mine.hidden_at AND ($3::uuid IS NULL OR m.id=$3)
        ORDER BY m.created_at DESC,m.id DESC LIMIT 1",
    )
    .bind(*id)
    .bind(session.user_id)
    .bind(body.message_id)
    .fetch_optional(&mut *tx)
    .await?;
    if body.message_id.is_some() && marker.is_none() {
        return Err(Error::missing());
    }
    if let Some((time, message)) = marker {
        sqlx::query("UPDATE conversation_members SET read_at=$3,read_id=$4 WHERE conversation_id=$1 AND user_id=$2
            AND (read_at,read_id)<($3,$4)")
            .bind(*id).bind(session.user_id).bind(time).bind(message).execute(&mut *tx).await?;
    }
    let actual: Option<Uuid> = sqlx::query_scalar(
        "SELECT NULLIF(read_id,'00000000-0000-0000-0000-000000000000'::uuid)
        FROM conversation_members WHERE conversation_id=$1 AND user_id=$2",
    )
    .bind(*id)
    .bind(session.user_id)
    .fetch_one(&mut *tx)
    .await?;
    let recipients = if crate::privacy::shares(&mut tx, session.user_id, "read_receipts").await? {
        members
    } else {
        vec![session.user_id]
    };
    tx.commit().await?;
    state
        .publish(
            recipients,
            ServerEvent::Read {
                conversation_id: *id,
                user_id: session.user_id,
                message_id: actual,
            },
        )
        .await;
    Ok(HttpResponse::NoContent().finish())
}

async fn edit(
    state: web::Data<AppState>, session: Session, path: web::Path<(Uuid, Uuid)>,
    body: web::Json<SendMessageRequest>,
) -> Result<web::Json<Message>> {
    validate_message(&body)?;
    let (conversation, message_id) = path.into_inner();
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::lock_conversation(&mut tx, conversation).await?;
    let members = authz::member(&mut tx, session.user_id, conversation).await?;
    let peers: Vec<_> = members.iter().copied().filter(|id| *id != session.user_id).collect();
    authz::peers(&mut tx, session.user_id, &peers).await?;
    let existing: MessageRow = sqlx::query_as("SELECT * FROM messages WHERE id=$1 AND conversation_id=$2 FOR UPDATE")
        .bind(message_id).bind(conversation).fetch_optional(&mut *tx).await?.ok_or_else(Error::missing)?;
    if existing.sender_id != session.user_id { return Err(Error::forbidden()); }
    let within_window: bool = sqlx::query_scalar("SELECT clock_timestamp() <= created_at + ($2::bigint * interval '1 second') FROM messages WHERE id=$1")
        .bind(message_id).bind(MESSAGE_EDIT_WINDOW_SECS).fetch_one(&mut *tx).await?;
    if !within_window { return Err(Error::conflict("O prazo de 15 minutos para editar esta mensagem terminou")); }
    if existing.kind != "text" || existing.attachment_id.is_some() || body.kind != MessageKind::Text
        || body.attachment_id.is_some() || body.client_id != existing.client_id {
        return Err(Error::bad("Somente mensagens de texto sem anexos podem ser editadas"));
    }
    let enrolled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys WHERE user_id=$1) OR EXISTS(SELECT 1 FROM bots WHERE user_id=ANY($2))")
        .bind(session.user_id).bind(&members).fetch_one(&mut *tx).await?;
    if enrolled && !body.body.starts_with(ENCRYPTED_PREFIX) {
        return Err(Error::bad("Este remetente exige criptografia ponta a ponta"));
    }
    if body.body.starts_with(ENCRYPTED_PREFIX) {
        let envelope = validate_envelope(&body.body, MAX_ENCRYPTED_BODY)?;
        let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys k JOIN encryption_key_sessions d ON d.device_id=k.device_id WHERE k.device_id=$1 AND k.user_id=$2 AND d.session_id=$3)")
            .bind(envelope.sender_device).bind(session.user_id).bind(session.id).fetch_one(&mut *tx).await?;
        if !owned { return Err(Error::forbidden()); }
        let keys = crate::encryption::keys_on(&mut tx, conversation).await?;
        if members.iter().any(|user| !keys.iter().any(|key| key.user_id == *user && key.active)) {
            return Err(Error::bad("Participante sem dispositivo criptográfico ativo"));
        }
        let expected: std::collections::HashSet<_> = keys.iter().filter(|key| key.active).map(|key| key.device_id).collect();
        let supplied: std::collections::HashSet<_> = envelope.recipients.iter().map(|key| key.device_id).collect();
        if expected != supplied { return Err(Error::conflict("A lista de dispositivos mudou; atualize a conversa antes de enviar")); }
    }
    state.rate(&format!("message-edit:{}", session.user_id), 60, 60).await?;
    let row: MessageRow = sqlx::query_as("UPDATE messages SET body=$3,style=$4 WHERE id=$1 AND conversation_id=$2 RETURNING *")
        .bind(message_id).bind(conversation).bind(&body.body).bind(Json(&body.style)).fetch_one(&mut *tx).await?;
    let message = row.model()?;
    crate::outbox::enqueue_update(&mut tx, &members, &message).await?;
    tx.commit().await?;
    Ok(web::Json(message))
}

async fn delete(
    state: web::Data<AppState>, session: Session, path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse> {
    let (conversation, message_id) = path.into_inner();
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::lock_conversation(&mut tx, conversation).await?;
    let members = authz::member(&mut tx, session.user_id, conversation).await?;
    let existing: Option<(Uuid, String)> = sqlx::query_as("SELECT sender_id,kind FROM messages WHERE id=$1 AND conversation_id=$2 FOR UPDATE")
        .bind(message_id).bind(conversation).fetch_optional(&mut *tx).await?;
    let (sender, kind) = existing.ok_or_else(Error::missing)?;
    if sender != session.user_id { return Err(Error::forbidden()); }
    if kind != "text" { return Err(Error::bad("Somente mensagens de texto podem ser excluídas aqui")); }
    sqlx::query("DELETE FROM messages WHERE id=$1 AND conversation_id=$2").bind(message_id).bind(conversation).execute(&mut *tx).await?;
    crate::outbox::enqueue_deletion(&mut tx, &members, conversation, message_id).await?;
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn clear(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::lock_conversation(&mut tx, *id).await?;
    authz::member(&mut tx, session.user_id, *id).await?;
    sqlx::query(
        "UPDATE conversation_members SET hidden_at=clock_timestamp(),
        read_at=clock_timestamp(),read_id='00000000-0000-0000-0000-000000000000'
        WHERE conversation_id=$1 AND user_id=$2",
    )
    .bind(*id)
    .bind(session.user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    state
        .publish(vec![session.user_id], ServerEvent::ConversationsChanged)
        .await;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/conversations/{id}/messages")
            .route(web::get().to(history))
            .route(web::post().to(send)),
    )
    .service(web::resource("/conversations/{id}/messages/{message_id}")
        .route(web::patch().to(edit)).route(web::delete().to(delete)))
    .route("/conversations/{id}/read", web::post().to(read))
    .route("/conversations/{id}/history", web::delete().to(clear));
}
