use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    state::AppState,
    validation,
};
use actix_web::{HttpResponse, web};
use chrono::{DateTime, Utc};
use nm_protocol::{Announcement, AnnouncementFeed, SaveAnnouncementRequest, ServerEvent};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    title: String,
    body: String,
    link_url: Option<String>,
    image_file_id: Option<Uuid>,
    full_banner: bool,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
impl From<Row> for Announcement {
    fn from(row: Row) -> Self {
        Self {
            id: row.id,
            title: row.title,
            body: row.body,
            link_url: row.link_url,
            image_file_id: row.image_file_id,
            full_banner: row.full_banner,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

async fn can_manage(db: &mut PgConnection, user: Uuid) -> Result<bool> {
    crate::admin::allowed(db, user).await
}


async fn list(state: web::Data<AppState>, session: Session) -> Result<web::Json<AnnouncementFeed>> {
    let mut tx = state.db.begin().await?;
    let admin = can_manage(&mut tx, session.user_id).await?;
    let rows: Vec<Row> = sqlx::query_as("SELECT * FROM announcements WHERE enabled OR $1 ORDER BY updated_at DESC,id DESC LIMIT 100")
        .bind(admin).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(web::Json(AnnouncementFeed {
        can_manage: admin,
        items: rows.into_iter().map(Into::into).collect(),
    }))
}

fn validate(body: &SaveAnnouncementRequest) -> Result<()> {
    validation::text(body.title.trim(), "Título do anúncio", 1, 120)?;
    validation::text(&body.body, "Texto do anúncio", 0, 2000)?;
    if let Some(link) = &body.link_url {
        validation::text(link, "Link", 1, 2048)?;
        let uri: actix_web::http::Uri = link.parse().map_err(|_| Error::bad("Link inválido"))?;
        if !matches!(uri.scheme_str(), Some("http" | "https"))
            || uri
                .authority()
                .is_none_or(|host| host.as_str().contains('@'))
        {
            return Err(Error::bad(
                "O link deve ser um endereço HTTP(S), sem credenciais",
            ));
        }
    }
    Ok(())
}

async fn changed(state: &AppState) -> Result<()> {
    let recipients: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT user_id FROM auth_sessions WHERE revoked_at IS NULL AND expires_at>clock_timestamp()")
        .fetch_all(&state.db).await?;
    state
        .publish(recipients, ServerEvent::AnnouncementsChanged)
        .await;
    Ok(())
}

async fn save(
    state: &AppState,
    session: &Session,
    id: Option<Uuid>,
    body: SaveAnnouncementRequest,
) -> Result<Announcement> {
    state.rate(&format!("admin-write:{}",session.user_id),30,60).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    crate::admin::require(&mut tx, session).await?;
    validate(&body)?;
    if let Some(image) = body.image_file_id {
        let file: Option<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
            "SELECT owner_id,conversation_id,mime FROM files WHERE id=$1 FOR UPDATE",
        )
        .bind(image)
        .fetch_optional(&mut *tx)
        .await?;
        let (owner, conversation, mime) = file.ok_or_else(Error::missing)?;
        let retained: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM announcements WHERE id=$1 AND image_file_id=$2)",
        )
        .bind(id)
        .bind(image)
        .fetch_one(&mut *tx)
        .await?;
        if conversation.is_some() || (owner != session.user_id && !retained) {
            return Err(Error::forbidden());
        }
        if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/gif") {
            return Err(Error::bad("Use uma imagem PNG, JPEG ou GIF no anúncio"));
        }
    }
    let row: Row = if let Some(id) = id {
        sqlx::query_as("UPDATE announcements SET title=$2,body=$3,link_url=$4,image_file_id=$5,full_banner=$6,enabled=$7,updated_at=clock_timestamp() WHERE id=$1 RETURNING *")
            .bind(id).bind(body.title.trim()).bind(&body.body).bind(&body.link_url).bind(body.image_file_id).bind(body.full_banner).bind(body.enabled)
            .fetch_optional(&mut *tx).await?.ok_or_else(Error::missing)?
    } else {
        sqlx::query_as("INSERT INTO announcements(id,title,body,link_url,image_file_id,full_banner,enabled,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *")
            .bind(Uuid::new_v4()).bind(body.title.trim()).bind(&body.body).bind(&body.link_url).bind(body.image_file_id).bind(body.full_banner).bind(body.enabled).bind(session.user_id)
            .fetch_one(&mut *tx).await?
    };
    crate::admin::audit(&mut tx,session.user_id,if id.is_some() { "announcement.update" } else { "announcement.create" },None,None,serde_json::json!({"announcement_id":row.id,"enabled":row.enabled})).await?;
    tx.commit().await?;
    changed(state).await?;
    Ok(row.into())
}

async fn create(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<SaveAnnouncementRequest>,
) -> Result<web::Json<Announcement>> {
    Ok(web::Json(
        save(&state, &session, None, body.into_inner()).await?,
    ))
}
async fn update(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<SaveAnnouncementRequest>,
) -> Result<web::Json<Announcement>> {
    Ok(web::Json(
        save(&state, &session, Some(*id), body.into_inner()).await?,
    ))
}
async fn delete(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    state.rate(&format!("admin-write:{}",session.user_id),30,60).await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    crate::admin::require(&mut tx, &session).await?;
    let deleted = sqlx::query("DELETE FROM announcements WHERE id=$1")
        .bind(*id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(Error::missing());
    }
    crate::admin::audit(&mut tx,session.user_id,"announcement.delete",None,None,serde_json::json!({"announcement_id":*id})).await?;
    tx.commit().await?;
    changed(&state).await?;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/announcements")
            .route(web::get().to(list))
            .route(web::post().to(create)),
    )
    .service(
        web::resource("/announcements/{id}")
            .route(web::put().to(update))
            .route(web::delete().to(delete)),
    );
}
