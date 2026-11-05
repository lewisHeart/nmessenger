use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    models::{MessageRow, ProfileRow},
    presence,
    state::AppState,
    validation,
};
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use nm_protocol::*;
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(FromRow)]
struct ConversationRow {
    id: Uuid,
    owner_id: Uuid,
    title: Option<String>,
    kind: String,
    members_can_invite: bool,
    updated_at: DateTime<Utc>,
}
#[derive(FromRow)]
struct MemberRow {
    conversation_id: Uuid,
    #[sqlx(flatten)]
    profile: ProfileRow,
}
#[derive(FromRow)]
struct InviteRow {
    id: Uuid,
    conversation_id: Uuid,
    conversation_title: Option<String>,
    created_at: DateTime<Utc>,
    #[sqlx(flatten)]
    inviter: ProfileRow,
    member_count: i64,
}

/// Batch loading avoids one profile, unread and last-message query per conversation.
async fn load(state: &AppState, user: Uuid, ids: &[Uuid]) -> Result<Vec<Conversation>> {
    let rows: Vec<ConversationRow> = sqlx::query_as(
        "SELECT c.* FROM conversations c
        JOIN conversation_members mine ON mine.conversation_id=c.id AND mine.user_id=$1
        WHERE c.id=ANY($2) AND NOT EXISTS(SELECT 1 FROM user_blocks b
            JOIN conversation_members x ON x.user_id=b.owner_id AND x.conversation_id=c.id
            JOIN conversation_members y ON y.user_id=b.user_id AND y.conversation_id=c.id)
        ORDER BY c.updated_at DESC,c.id DESC",
    )
    .bind(user)
    .bind(ids)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<_> = rows.iter().map(|c| c.id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let member_rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT cm.conversation_id,u.*
        FROM conversation_members cm JOIN users u ON u.id=cm.user_id
        WHERE cm.conversation_id=ANY($1) ORDER BY u.display_name,u.id",
    )
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    let profiles = presence::profiles(
        state,
        user,
        member_rows.iter().map(|m| m.profile.clone()).collect(),
    )
    .await?;
    let mut members: HashMap<Uuid, Vec<Profile>> = HashMap::new();
    for (row, profile) in member_rows.into_iter().zip(profiles) {
        members
            .entry(row.conversation_id)
            .or_default()
            .push(profile);
    }
    let last_rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT DISTINCT ON(m.conversation_id) m.*
        FROM messages m JOIN conversation_members mine ON mine.conversation_id=m.conversation_id
        AND mine.user_id=$1 WHERE m.conversation_id=ANY($2) AND m.created_at>mine.hidden_at
        AND (NOT EXISTS(SELECT 1 FROM bots WHERE user_id=$1) OR m.created_at>=mine.joined_at)
        ORDER BY m.conversation_id,m.created_at DESC,m.id DESC",
    )
    .bind(user)
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    let mut last = HashMap::new();
    for row in last_rows {
        last.insert(row.conversation_id, row.model()?);
    }
    let counts: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT m.conversation_id,count(*) FROM messages m
        JOIN conversation_members mine ON mine.conversation_id=m.conversation_id AND mine.user_id=$1
        WHERE m.conversation_id=ANY($2) AND m.sender_id<>$1 AND m.created_at>mine.hidden_at
        AND (NOT EXISTS(SELECT 1 FROM bots WHERE user_id=$1) OR m.created_at>=mine.joined_at)
        AND (m.created_at,m.id)>(mine.read_at,mine.read_id) GROUP BY m.conversation_id",
    )
    .bind(user)
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    let counts: HashMap<_, _> = counts.into_iter().collect();
    rows.into_iter()
        .map(|row| {
            Ok(Conversation {
                id: row.id,
                owner_id: Some(row.owner_id),
                title: row.title,
                kind: validation::parse(&row.kind)?,
                members_can_invite: row.members_can_invite,
                updated_at: row.updated_at,
                members: members.remove(&row.id).unwrap_or_default(),
                unread: *counts.get(&row.id).unwrap_or(&0),
                last_message: last.remove(&row.id),
            })
        })
        .collect()
}

pub async fn list(state: &AppState, user: Uuid) -> Result<Vec<Conversation>> {
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT conversation_id FROM conversation_members WHERE user_id=$1")
            .bind(user)
            .fetch_all(&state.db)
            .await?;
    load(state, user, &ids).await
}

pub async fn one(state: &AppState, user: Uuid, id: Uuid) -> Result<Conversation> {
    load(state, user, &[id])
        .await?
        .pop()
        .ok_or_else(Error::forbidden)
}

async fn get(state: web::Data<AppState>, session: Session) -> Result<web::Json<Vec<Conversation>>> {
    Ok(web::Json(list(&state, session.user_id).await?))
}

async fn create(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<CreateConversationRequest>,
) -> Result<web::Json<Conversation>> {
    let mut members = body.member_ids.clone();
    members.push(session.user_id);
    members.sort_unstable();
    members.dedup();
    if !(2..=50).contains(&members.len()) {
        return Err(Error::bad("Conversa deve ter 2–50 participantes"));
    }
    let title = body
        .title
        .as_ref()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty());
    if let Some(v) = title {
        validation::text(v, "Título", 1, 150)?;
    }
    state
        .rate(&format!("conversation-create:{}", session.user_id), 30, 60)
        .await?;
    let direct = members.len() == 2 && title.is_none();
    let direct_key = direct.then(|| format!("{}:{}", members[0], members[1]));
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    for (index, user) in members.iter().enumerate() {
        authz::peers(&mut tx, *user, &members[index + 1..]).await?;
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO conversations(id,kind,title,owner_id,direct_key,members_can_invite)
        VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(direct_key) DO UPDATE SET direct_key=EXCLUDED.direct_key RETURNING id")
        .bind(Uuid::new_v4()).bind(if direct {"direct"} else {"group"}).bind(title)
        .bind(session.user_id).bind(direct_key).bind(body.members_can_invite).fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO conversation_members(conversation_id,user_id) SELECT $1,unnest($2::uuid[])
        ON CONFLICT(conversation_id,user_id) DO NOTHING",
    )
    .bind(id)
    .bind(&members)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    state
        .publish(members, ServerEvent::ConversationsChanged)
        .await;
    Ok(web::Json(one(&state, session.user_id, id).await?))
}

async fn update(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<UpdateConversationRequest>,
) -> Result<web::Json<Conversation>> {
    let title = body.title.trim();
    if !title.is_empty() {
        validation::text(title, "Título", 1, 150)?;
    }
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let (kind, owner, members_can_invite) = authz::lock_conversation(&mut tx, *id).await?;
    let members = authz::member(&mut tx, session.user_id, *id).await?;
    if kind != "group" || owner != session.user_id {
        return Err(Error::forbidden());
    }
    let new_can_invite = body.members_can_invite.unwrap_or(members_can_invite);
    if title.is_empty() {
        sqlx::query("UPDATE conversations SET members_can_invite=$2,updated_at=clock_timestamp() WHERE id=$1")
            .bind(*id)
            .bind(new_can_invite)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE conversations SET title=$2,members_can_invite=$3,updated_at=clock_timestamp() WHERE id=$1")
            .bind(*id)
            .bind(title)
            .bind(new_can_invite)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    state
        .publish(members, ServerEvent::ConversationsChanged)
        .await;
    Ok(web::Json(one(&state, session.user_id, *id).await?))
}

async fn add_member(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<MemberRequest>,
) -> Result<web::Json<Conversation>> {
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let (kind, owner, members_can_invite) = authz::lock_conversation(&mut tx, *id).await?;
    let mut members = authz::member(&mut tx, session.user_id, *id).await?;
    if kind != "group" {
        return Err(Error::forbidden());
    }
    if !members_can_invite && owner != session.user_id {
        return Err(Error::forbidden());
    }
    if !members.contains(&body.user_id) {
        if members.len() >= 50 {
            return Err(Error::bad("Máximo de 50 participantes"));
        }
        authz::group_invite(&mut tx, session.user_id, body.user_id, &members).await?;

        let target_settings: Option<sqlx::types::Json<UserSettings>> =
            sqlx::query_scalar("SELECT settings FROM users WHERE id=$1")
                .bind(body.user_id)
                .fetch_optional(&mut *tx)
                .await?;
        let require_invite = target_settings.map(|s| s.require_group_invite).unwrap_or(false);

        if require_invite {
            sqlx::query(
                "INSERT INTO conversation_invites(conversation_id,inviter_id,user_id)
                VALUES($1,$2,$3) ON CONFLICT(conversation_id,user_id) DO UPDATE SET inviter_id=EXCLUDED.inviter_id,created_at=clock_timestamp()",
            )
            .bind(*id)
            .bind(session.user_id)
            .bind(body.user_id)
            .execute(&mut *tx)
            .await?;
            state.publish(vec![body.user_id], ServerEvent::ConversationsChanged).await;
        } else {
            sqlx::query("INSERT INTO conversation_members(conversation_id,user_id) VALUES($1,$2)")
                .bind(*id)
                .bind(body.user_id)
                .execute(&mut *tx)
                .await?;
            members.push(body.user_id);
        }
    }
    sqlx::query("UPDATE conversations SET updated_at=clock_timestamp() WHERE id=$1")
        .bind(*id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .publish(members, ServerEvent::ConversationsChanged)
        .await;
    Ok(web::Json(one(&state, session.user_id, *id).await?))
}

async fn remove_member(
    state: web::Data<AppState>,
    session: Session,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse> {
    let (id, target) = *path;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let (kind, owner, _) = authz::lock_conversation(&mut tx, id).await?;
    let members: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM conversation_members WHERE conversation_id=$1 ORDER BY user_id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    if kind != "group"
        || !members.contains(&session.user_id)
        || (owner != session.user_id && target != session.user_id)
    {
        return Err(Error::forbidden());
    }
    if !members.contains(&target) {
        return Err(Error::missing());
    }
    sqlx::query("DELETE FROM conversation_members WHERE conversation_id=$1 AND user_id=$2")
        .bind(id)
        .bind(target)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE activities SET status='ended' WHERE conversation_id=$1 AND status IN('offered','accepted')")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query(
        "DELETE FROM activity_participants p USING activities a
        WHERE p.activity_id=a.id AND a.conversation_id=$1 AND p.user_id=$2",
    )
    .bind(id)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    if owner == target
        && let Some(next) = members.iter().find(|u| **u != target)
    {
        sqlx::query("UPDATE conversations SET owner_id=$2 WHERE id=$1")
            .bind(id)
            .bind(next)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE conversations SET updated_at=clock_timestamp() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .publish(members, ServerEvent::ConversationsChanged)
        .await;
    Ok(HttpResponse::NoContent().finish())
}

async fn transfer_owner(state: web::Data<AppState>, session: Session, id: web::Path<Uuid>, body: web::Json<MemberRequest>) -> Result<web::Json<Conversation>> {
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let (kind, owner, _) = authz::lock_conversation(&mut tx, *id).await?;
    let members = authz::member(&mut tx, session.user_id, *id).await?;
    if kind != "group" || owner != session.user_id { return Err(Error::forbidden()); }
    if !members.contains(&body.user_id) { return Err(Error::missing()); }
    sqlx::query("UPDATE conversations SET owner_id=$2,updated_at=clock_timestamp() WHERE id=$1")
        .bind(*id).bind(body.user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    state.publish(members, ServerEvent::ConversationsChanged).await;
    Ok(web::Json(one(&state, session.user_id, *id).await?))
}

pub async fn invites(state: &AppState, user: Uuid) -> Result<Vec<ConversationInvite>> {
    let rows: Vec<InviteRow> = sqlx::query_as(
        "SELECT ci.id, ci.conversation_id, c.title AS conversation_title, ci.created_at,
                u.*,
                (SELECT count(*) FROM conversation_members WHERE conversation_id=c.id) AS member_count
         FROM conversation_invites ci
         JOIN conversations c ON c.id=ci.conversation_id
         JOIN users u ON u.id=ci.inviter_id
         WHERE ci.user_id=$1
         ORDER BY ci.created_at DESC",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?;

    let profiles = presence::profiles(
        state,
        user,
        rows.iter().map(|r| r.inviter.clone()).collect(),
    )
    .await?;

    rows.into_iter()
        .zip(profiles)
        .map(|(row, inviter)| {
            Ok(ConversationInvite {
                id: row.id,
                conversation_id: row.conversation_id,
                conversation_title: row.conversation_title,
                inviter,
                member_count: row.member_count,
                created_at: row.created_at,
            })
        })
        .collect()
}

async fn get_invites(
    state: web::Data<AppState>,
    session: Session,
) -> Result<web::Json<Vec<ConversationInvite>>> {
    Ok(web::Json(invites(&state, session.user_id).await?))
}

async fn accept_invite(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<web::Json<Conversation>> {
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let (kind, _, _) = authz::lock_conversation(&mut tx, *id).await?;
    if kind != "group" {
        return Err(Error::forbidden());
    }
    let invite_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM conversation_invites WHERE conversation_id=$1 AND user_id=$2)",
    )
    .bind(*id)
    .bind(session.user_id)
    .fetch_one(&mut *tx)
    .await?;
    if !invite_exists {
        return Err(Error::bad("Convite não encontrado"));
    }
    sqlx::query("DELETE FROM conversation_invites WHERE conversation_id=$1 AND user_id=$2")
        .bind(*id)
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    let mut members: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM conversation_members WHERE conversation_id=$1",
    )
    .bind(*id)
    .fetch_all(&mut *tx)
    .await?;
    if members.len() >= 50 {
        return Err(Error::bad("Máximo de 50 participantes"));
    }
    if !members.contains(&session.user_id) {
        sqlx::query("INSERT INTO conversation_members(conversation_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(*id)
            .bind(session.user_id)
            .execute(&mut *tx)
            .await?;
        members.push(session.user_id);
    }
    sqlx::query("UPDATE conversations SET updated_at=clock_timestamp() WHERE id=$1")
        .bind(*id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .publish(members, ServerEvent::ConversationsChanged)
        .await;
    Ok(web::Json(one(&state, session.user_id, *id).await?))
}

async fn decline_invite(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    sqlx::query("DELETE FROM conversation_invites WHERE conversation_id=$1 AND user_id=$2")
        .bind(*id)
        .bind(session.user_id)
        .execute(&state.db)
        .await?;
    state
        .publish(vec![session.user_id], ServerEvent::ConversationsChanged)
        .await;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/conversations")
            .route(web::get().to(get))
            .route(web::post().to(create)),
    )
    .route("/conversations/invites", web::get().to(get_invites))
    .route("/conversations/{id}/invites/accept", web::post().to(accept_invite))
    .route("/conversations/{id}/invites/decline", web::post().to(decline_invite))
    .route("/conversations/{id}", web::patch().to(update))
    .route("/conversations/{id}/members", web::post().to(add_member))
    .route("/conversations/{id}/owner", web::put().to(transfer_owner))
    .route(
        "/conversations/{id}/members/{user_id}",
        web::delete().to(remove_member),
    );
}
