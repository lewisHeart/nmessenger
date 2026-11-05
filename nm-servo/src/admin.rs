use crate::{auth::{self, Session}, authz, error::{Error, Result}, state::AppState, validation};
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use nm_protocol::*;
use serde::Deserialize;
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

pub async fn allowed(db: &mut PgConnection, user: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM app_admins a JOIN users u ON u.id=a.user_id WHERE a.user_id=$1 AND u.disabled_at IS NULL)").bind(user).fetch_one(db).await?)
}

pub async fn require(db: &mut PgConnection, session: &Session) -> Result<()> {
    let authorized: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM app_admins a JOIN users u ON u.id=a.user_id JOIN auth_sessions s ON s.user_id=u.id WHERE a.user_id=$1 AND s.id=$2 AND u.disabled_at IS NULL AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp())").bind(session.user_id).bind(session.id).fetch_one(db).await?;
    if !authorized { return Err(Error::forbidden()); }
    Ok(())
}

pub async fn audit(db: &mut PgConnection, actor: Uuid, action: &str, user: Option<Uuid>, session: Option<Uuid>, details: serde_json::Value) -> Result<()> {
    sqlx::query("INSERT INTO admin_audit(actor_id,actor_email,action,target_user,target_session,details) SELECT id,email,$2,$3,$4,$5 FROM users WHERE id=$1")
        .bind(actor).bind(action).bind(user).bind(session).bind(Json(details)).execute(db).await?;
    Ok(())
}

#[derive(Deserialize)]
struct PageQuery { q: Option<String>, offset: Option<i64>, limit: Option<i64> }
impl PageQuery {
    fn values(&self) -> Result<(String,i64,i64)> {
        let q = self.q.clone().unwrap_or_default();
        validation::text(&q, "Busca", 0, 200)?;
        let offset = self.offset.unwrap_or(0);
        let limit = self.limit.unwrap_or(25);
        if !(0..=1_000_000).contains(&offset) || !(1..=100).contains(&limit) { return Err(Error::bad("Paginação inválida")); }
        Ok((q,offset,limit))
    }
}

async fn overview(state: web::Data<AppState>, session: Session) -> Result<web::Json<AdminOverview>> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?; require(&mut tx, &session).await?;
    let row: (i64,i64,i64,i64,i64,i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM users WHERE deleted_at IS NULL),(SELECT count(*) FROM users WHERE disabled_at IS NOT NULL AND deleted_at IS NULL),(SELECT count(*) FROM auth_sessions WHERE revoked_at IS NULL AND expires_at>clock_timestamp()),(SELECT count(*) FROM conversations),(SELECT count(*) FROM messages),(SELECT count(*) FROM files),(SELECT COALESCE(sum(size),0)::bigint FROM files),(SELECT count(*) FROM announcements WHERE enabled)").fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(web::Json(AdminOverview { users:row.0,suspended_users:row.1,active_sessions:row.2,conversations:row.3,messages:row.4,files:row.5,stored_bytes:row.6,published_announcements:row.7,user_upload_quota:state.user_upload_quota,session_ttl_secs:state.session_ttl_secs }))
}

async fn users(state: web::Data<AppState>, session: Session, query: web::Query<PageQuery>) -> Result<web::Json<AdminPage<AdminUser>>> {
    let (q,offset,limit) = query.values()?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?; require(&mut tx, &session).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE deleted_at IS NULL AND strpos(lower(email||' '||display_name),lower($1))>0").bind(&q).fetch_one(&mut *tx).await?;
    let rows: Vec<(Uuid,String,String,DateTime<Utc>,Option<DateTime<Utc>>,bool,i64,i64,bool)> = sqlx::query_as("SELECT u.id,u.email,u.display_name,u.created_at,u.disabled_at,EXISTS(SELECT 1 FROM app_admins WHERE user_id=u.id),(SELECT count(*) FROM auth_sessions WHERE user_id=u.id AND revoked_at IS NULL AND expires_at>clock_timestamp()),(SELECT COALESCE(sum(size),0)::bigint FROM files WHERE owner_id=u.id),EXISTS(SELECT 1 FROM banned_emails WHERE email=u.email) FROM users u WHERE u.deleted_at IS NULL AND strpos(lower(u.email||' '||u.display_name),lower($1))>0 ORDER BY u.created_at DESC,u.id DESC LIMIT $2 OFFSET $3").bind(&q).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let mut badge_map = crate::badges::for_users_conn(&mut tx, &ids).await?;
    tx.commit().await?;
    let items = rows.into_iter().map(|(id,email,display_name,created_at,disabled_at,administrator,active_sessions,stored_bytes,banned)| {
        let badges = badge_map.remove(&id).unwrap_or_default();
        AdminUser {id,email,display_name,created_at,disabled_at,administrator,active_sessions,stored_bytes,badges,banned}
    }).collect();
    Ok(web::Json(AdminPage {items,total,offset}))
}

async fn sessions(state: web::Data<AppState>, session: Session, query: web::Query<PageQuery>) -> Result<web::Json<AdminPage<AdminSession>>> {
    let (q,offset,limit) = query.values()?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?; require(&mut tx, &session).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND strpos(lower(u.email||' '||s.device_name),lower($1))>0").bind(&q).fetch_one(&mut *tx).await?;
    let rows: Vec<(Uuid,Uuid,String,String,DateTime<Utc>,DateTime<Utc>,DateTime<Utc>)> = sqlx::query_as("SELECT s.id,s.user_id,u.email,s.device_name,s.created_at,s.last_seen_at,s.expires_at FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND strpos(lower(u.email||' '||s.device_name),lower($1))>0 ORDER BY s.last_seen_at DESC,s.id DESC LIMIT $2 OFFSET $3").bind(&q).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let items = rows.into_iter().map(|(id,user_id,email,device_name,created_at,last_seen_at,expires_at)| AdminSession {id,user_id,email,device_name,created_at,last_seen_at,expires_at,current:id==session.id}).collect();
    Ok(web::Json(AdminPage {items,total,offset}))
}

async fn publish_revocations(state: &AppState, user: Uuid, sessions: Vec<Uuid>) {
    for session_id in sessions { state.publish(vec![user], ServerEvent::SessionRevoked {session_id}).await; }
}

#[derive(Deserialize)]
struct ModerateUser { confirmation_password: String, banned: Option<bool> }

async fn moderate_user(state: web::Data<AppState>, session: Session, target: web::Path<Uuid>, body: web::Json<ModerateUser>) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}", session.user_id), 30, 60).await?;
    if *target == session.user_id { return Err(Error::bad("Use outro administrador para alterar sua própria conta")); }
    { let mut db = state.db.acquire().await?; require(&mut db, &session).await?; }
    auth::confirm_password(&state, &session, &body.confirmation_password).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    require(&mut tx, &session).await?;
    auth::valid_on(&state, &mut *tx, &session).await?;
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM users WHERE id=$1 AND deleted_at IS NULL FOR UPDATE").bind(*target).fetch_optional(&mut *tx).await?;
    let email = email.ok_or_else(Error::missing)?;
    let revoked;
    let action;
    if let Some(banned) = body.banned {
        if banned {
            sqlx::query("INSERT INTO banned_emails(email) VALUES($1) ON CONFLICT DO NOTHING").bind(&email).execute(&mut *tx).await?;
            sqlx::query("UPDATE users SET disabled_at=COALESCE(disabled_at,clock_timestamp()) WHERE id=$1").bind(*target).execute(&mut *tx).await?;
        } else {
            sqlx::query("DELETE FROM banned_emails WHERE email=$1").bind(&email).execute(&mut *tx).await?;
            sqlx::query("UPDATE users SET disabled_at=NULL WHERE id=$1").bind(*target).execute(&mut *tx).await?;
        }
        action = if banned { "user.ban" } else { "user.unban" };
        revoked = if banned { sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL RETURNING id").bind(*target).fetch_all(&mut *tx).await? } else { Vec::new() };
    } else {
        let banned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM banned_emails WHERE email=$1)").bind(&email).fetch_one(&mut *tx).await?;
        if banned { return Err(Error::bad("Remova o banimento antes de excluir a conta")); }
        revoked = sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL RETURNING id").bind(*target).fetch_all(&mut *tx).await?;
        sqlx::query("UPDATE users SET email=$2, display_name='Usuário excluído', personal_message='', bio='', presence='offline', avatar_id=NULL, scene='', now_playing=NULL, disabled_at=clock_timestamp(), deleted_at=clock_timestamp() WHERE id=$1").bind(*target).bind(format!("deleted-{}@invalid.local", *target)).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM app_admins WHERE user_id=$1").bind(*target).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM contacts WHERE owner_id=$1 OR user_id=$1").bind(*target).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM user_badges WHERE user_id=$1").bind(*target).execute(&mut *tx).await?;
        action = "user.delete";
    }
    audit(&mut tx, session.user_id, action, Some(*target), None, serde_json::json!({"email":email})).await?;
    tx.commit().await?;
    publish_revocations(&state, *target, revoked).await;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
struct ResetPassword { password: String, confirmation_password: String }

async fn reset_password(state: web::Data<AppState>, session: Session, target: web::Path<Uuid>, body: web::Json<ResetPassword>) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}", session.user_id), 30, 60).await?;
    if *target == session.user_id { return Err(Error::bad("Use as configurações para alterar sua própria senha")); }
    { let mut db = state.db.acquire().await?; require(&mut db, &session).await?; }
    auth::confirm_password(&state, &session, &body.confirmation_password).await?;
    validation::password(&body.password)?;
    let hash = auth::hash_password(body.password.clone()).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    require(&mut tx, &session).await?;
    auth::valid_on(&state, &mut *tx, &session).await?;
    let changed = sqlx::query("UPDATE users SET password_hash=$2 WHERE id=$1 AND deleted_at IS NULL").bind(*target).bind(hash).execute(&mut *tx).await?;
    if changed.rows_affected() == 0 { return Err(Error::missing()); }
    sqlx::query("INSERT INTO opaque_recovery_credentials(user_id,record) SELECT user_id,record FROM opaque_credentials WHERE user_id=$1 ON CONFLICT(user_id) DO NOTHING")
        .bind(*target).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM opaque_credentials WHERE user_id=$1").bind(*target).execute(&mut *tx).await?;
    let revoked: Vec<Uuid> = sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL RETURNING id").bind(*target).fetch_all(&mut *tx).await?;
    audit(&mut tx, session.user_id, "user.reset_password", Some(*target), None, serde_json::json!({})).await?;
    tx.commit().await?;
    publish_revocations(&state, *target, revoked).await;
    Ok(HttpResponse::NoContent().finish())
}

async fn update_user(state: web::Data<AppState>, session: Session, target: web::Path<Uuid>, body: web::Json<AdminUpdateUser>) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}",session.user_id),30,60).await?;
    if body.disabled.is_none() && body.administrator.is_none() { return Err(Error::bad("Nenhuma alteração informada")); }
    if *target == session.user_id { return Err(Error::bad("Use outro administrador para alterar seu próprio acesso")); }
    // Check authorization before performing password work or revealing target state.
    { let mut db = state.db.acquire().await?; require(&mut db, &session).await?; }
    if body.administrator.is_some() { auth::confirm_password(&state, &session, body.confirmation_password.as_deref().unwrap_or("")).await?; }
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?; require(&mut tx, &session).await?;
    auth::valid_on(&state, &mut *tx, &session).await?;
    let old: Option<Option<DateTime<Utc>>> = sqlx::query_scalar("SELECT disabled_at FROM users WHERE id=$1 AND deleted_at IS NULL FOR UPDATE").bind(*target).fetch_optional(&mut *tx).await?;
    old.ok_or_else(Error::missing)?;
    if body.disabled == Some(false) {
        let banned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u JOIN banned_emails b ON b.email=u.email WHERE u.id=$1)").bind(*target).fetch_one(&mut *tx).await?;
        if banned { return Err(Error::bad("Remova o banimento antes de reativar a conta")); }
    }
    let mut revoked = Vec::new();
    if let Some(disabled) = body.disabled {
        sqlx::query("UPDATE users SET disabled_at=CASE WHEN $2 THEN COALESCE(disabled_at,clock_timestamp()) ELSE NULL END WHERE id=$1").bind(*target).bind(disabled).execute(&mut *tx).await?;
        if disabled { revoked = sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL RETURNING id").bind(*target).fetch_all(&mut *tx).await?; }
    }
    if let Some(administrator) = body.administrator {
        if administrator { sqlx::query("INSERT INTO app_admins(user_id) VALUES($1) ON CONFLICT DO NOTHING").bind(*target).execute(&mut *tx).await?; }
        else { sqlx::query("DELETE FROM app_admins WHERE user_id=$1").bind(*target).execute(&mut *tx).await?; }
    }
    audit(&mut tx, session.user_id, "user.access", Some(*target), None, serde_json::json!({"disabled":body.disabled,"administrator":body.administrator})).await?;
    tx.commit().await?;
    publish_revocations(&state, *target, revoked).await;
    state.publish(vec![*target], ServerEvent::AnnouncementsChanged).await;
    Ok(HttpResponse::NoContent().finish())
}

async fn revoke_user_sessions(state: web::Data<AppState>, session: Session, user: web::Path<Uuid>) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}",session.user_id),30,60).await?;
    if *user == session.user_id { return Err(Error::bad("Use a aba Sessões para encerrar suas outras sessões")); }
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?; require(&mut tx, &session).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1)").bind(*user).fetch_one(&mut *tx).await?;
    if !exists { return Err(Error::missing()); }
    let ids: Vec<Uuid> = sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL RETURNING id").bind(*user).fetch_all(&mut *tx).await?;
    audit(&mut tx,session.user_id,"user.revoke_sessions",Some(*user),None,serde_json::json!({"sessions":ids.len()})).await?;
    tx.commit().await?;
    publish_revocations(&state,*user,ids).await;
    Ok(HttpResponse::NoContent().finish())
}

async fn revoke_session(state: web::Data<AppState>, session: Session, id: web::Path<Uuid>) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}",session.user_id),30,60).await?;
    if *id == session.id { return Err(Error::bad("Não encerre a sessão que está administrando o aplicativo")); }
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?; require(&mut tx,&session).await?;
    let user: Option<Uuid> = sqlx::query_scalar("UPDATE auth_sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp()) WHERE id=$1 RETURNING user_id").bind(*id).fetch_optional(&mut *tx).await?;
    let user = user.ok_or_else(Error::missing)?;
    audit(&mut tx,session.user_id,"session.revoke",Some(user),Some(*id),serde_json::json!({})).await?;
    tx.commit().await?;
    publish_revocations(&state,user,vec![*id]).await;
    Ok(HttpResponse::NoContent().finish())
}

async fn audit_log(state: web::Data<AppState>, session: Session, query: web::Query<PageQuery>) -> Result<web::Json<AdminPage<AdminAuditEntry>>> {
    let (_,offset,limit) = query.values()?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?; require(&mut tx,&session).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit").fetch_one(&mut *tx).await?;
    let rows: Vec<(i64,Option<Uuid>,String,String,Option<Uuid>,Option<Uuid>,Json<serde_json::Value>,DateTime<Utc>)> = sqlx::query_as("SELECT id,actor_id,actor_email,action,target_user,target_session,details,created_at FROM admin_audit ORDER BY id DESC LIMIT $1 OFFSET $2").bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let items = rows.into_iter().map(|(id,actor_id,actor_email,action,target_user,target_session,details,created_at)| AdminAuditEntry {id,actor_id,actor_email,action,target_user,target_session,details:details.0,created_at}).collect();
    Ok(web::Json(AdminPage {items,total,offset}))
}

async fn list_badges(
    state: web::Data<AppState>,
    session: Session,
) -> Result<web::Json<Vec<AdminBadge>>> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    require(&mut tx, &session).await?;
    let rows: Vec<(Uuid, String, String, String, String, i32, i64, DateTime<Utc>)> = sqlx::query_as(
        "SELECT b.id, b.slug, b.name, b.description, b.icon, b.points,
        (SELECT count(*) FROM user_badges WHERE badge_id = b.id), b.created_at
        FROM badges b
        ORDER BY b.points DESC, b.created_at ASC",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let items = rows
        .into_iter()
        .map(|(id, slug, name, description, icon, points, users_count, created_at)| {
            AdminBadge {
                id,
                slug,
                name,
                description,
                icon,
                points,
                users_count,
                created_at,
            }
        })
        .collect();
    Ok(web::Json(items))
}

async fn create_badge(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<CreateBadgeRequest>,
) -> Result<web::Json<AdminBadge>> {
    state.rate(&format!("admin-write:{}", session.user_id), 30, 60).await?;
    let slug = body.slug.trim().to_lowercase();
    validation::text(&slug, "Identificador (slug)", 2, 64)?;
    validation::text(body.name.trim(), "Nome do selo", 2, 100)?;
    validation::text(&body.description, "Descrição", 0, 500)?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    require(&mut tx, &session).await?;
    let row: (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO badges (slug, name, description, icon, points)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, created_at",
    )
    .bind(&slug)
    .bind(body.name.trim())
    .bind(&body.description)
    .bind(body.icon.trim())
    .bind(body.points)
    .fetch_one(&mut *tx)
    .await?;
    audit(
        &mut tx,
        session.user_id,
        "badge.create",
        None,
        None,
        serde_json::json!({"slug": slug, "name": body.name, "points": body.points}),
    )
    .await?;
    tx.commit().await?;
    Ok(web::Json(AdminBadge {
        id: row.0,
        slug,
        name: body.name.trim().to_owned(),
        description: body.description.clone(),
        icon: body.icon.trim().to_owned(),
        points: body.points,
        users_count: 0,
        created_at: row.1,
    }))
}

async fn update_badge(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<UpdateBadgeRequest>,
) -> Result<web::Json<AdminBadge>> {
    state.rate(&format!("admin-write:{}", session.user_id), 30, 60).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    require(&mut tx, &session).await?;
    let row: (Uuid, String, String, String, String, i32, i64, DateTime<Utc>) = sqlx::query_as(
        "UPDATE badges SET
        name = COALESCE($2, name),
        description = COALESCE($3, description),
        icon = COALESCE($4, icon),
        points = COALESCE($5, points)
        WHERE id = $1
        RETURNING id, slug, name, description, icon, points,
        (SELECT count(*) FROM user_badges WHERE badge_id = $1), created_at",
    )
    .bind(*id)
    .bind(body.name.as_ref().map(|s| s.trim()))
    .bind(&body.description)
    .bind(body.icon.as_ref().map(|s| s.trim()))
    .bind(body.points)
    .fetch_one(&mut *tx)
    .await?;
    audit(
        &mut tx,
        session.user_id,
        "badge.update",
        None,
        None,
        serde_json::json!({"id": *id, "name": body.name, "points": body.points}),
    )
    .await?;
    tx.commit().await?;
    Ok(web::Json(AdminBadge {
        id: row.0,
        slug: row.1,
        name: row.2,
        description: row.3,
        icon: row.4,
        points: row.5,
        users_count: row.6,
        created_at: row.7,
    }))
}

async fn delete_badge(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}", session.user_id), 30, 60).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    require(&mut tx, &session).await?;
    let slug: Option<String> = sqlx::query_scalar("DELETE FROM badges WHERE id=$1 RETURNING slug")
        .bind(*id)
        .fetch_optional(&mut *tx)
        .await?;
    let slug = slug.ok_or_else(Error::missing)?;
    audit(
        &mut tx,
        session.user_id,
        "badge.delete",
        None,
        None,
        serde_json::json!({"id": *id, "slug": slug}),
    )
    .await?;
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn mutate_user_badge(
    state: web::Data<AppState>,
    session: Session,
    target: web::Path<Uuid>,
    body: web::Json<UserBadgeMutation>,
) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}", session.user_id), 30, 60).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    require(&mut tx, &session).await?;
    let user_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1)")
        .bind(*target)
        .fetch_one(&mut *tx)
        .await?;
    if !user_exists {
        return Err(Error::missing());
    }
    let badge_name: Option<String> = sqlx::query_scalar("SELECT name FROM badges WHERE id=$1")
        .bind(body.badge_id)
        .fetch_optional(&mut *tx)
        .await?;
    let badge_name = badge_name.ok_or_else(Error::missing)?;

    if body.grant {
        sqlx::query(
            "INSERT INTO user_badges (user_id, badge_id, awarded_by)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id, badge_id) DO NOTHING",
        )
        .bind(*target)
        .bind(body.badge_id)
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
        audit(
            &mut tx,
            session.user_id,
            "badge.grant",
            Some(*target),
            None,
            serde_json::json!({"badge_id": body.badge_id, "badge_name": badge_name}),
        )
        .await?;
    } else {
        sqlx::query("DELETE FROM user_badges WHERE user_id=$1 AND badge_id=$2")
            .bind(*target)
            .bind(body.badge_id)
            .execute(&mut *tx)
            .await?;
        audit(
            &mut tx,
            session.user_id,
            "badge.revoke",
            Some(*target),
            None,
            serde_json::json!({"badge_id": body.badge_id, "badge_name": badge_name}),
        )
        .await?;
    }

    let user_row: crate::models::ProfileRow = sqlx::query_as("SELECT * FROM users WHERE id=$1")
        .bind(*target)
        .fetch_one(&mut *tx)
        .await?;
    let audience = crate::presence::audience(&mut tx, *target).await?;
    let badges = crate::badges::for_user_conn(&mut tx, *target).await?;
    tx.commit().await?;

    let profile = user_row.model_with_badges(badges)?;
    state
        .publish(
            vec![*target],
            ServerEvent::ProfileUpdated {
                profile: profile.clone(),
            },
        )
        .await;
    let mut public = profile.clone();
    public.presence = if crate::presence::online(&state, *target).await? {
        profile.presence.visible()
    } else {
        nm_protocol::Presence::Offline
    };
    state
        .publish(
            audience.into_iter().filter(|id| *id != *target).collect(),
            ServerEvent::ProfileUpdated { profile: public },
        )
        .await;

    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/admin/overview", web::get().to(overview))
        .route("/admin/users", web::get().to(users))
        .route("/admin/users/{id}",web::patch().to(update_user))
        .route("/admin/users/{id}/password",web::post().to(reset_password))
        .route("/admin/users/{id}/moderate",web::post().to(moderate_user))
        .route("/admin/users/{id}/revoke-sessions",web::post().to(revoke_user_sessions))
        .route("/admin/users/{id}/badges", web::post().to(mutate_user_badge))
        .route("/admin/badges", web::get().to(list_badges))
        .route("/admin/badges", web::post().to(create_badge))
        .route("/admin/badges/{id}", web::put().to(update_badge))
        .route("/admin/badges/{id}", web::delete().to(delete_badge))
        .route("/admin/sessions",web::get().to(sessions))
        .route("/admin/sessions/{id}",web::delete().to(revoke_session))
        .route("/admin/audit",web::get().to(audit_log));
}
