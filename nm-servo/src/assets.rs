use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    models::AssetRow,
    presence,
    state::AppState,
    validation,
};
use actix_web::{HttpResponse, web};
use nm_protocol::*;
use uuid::Uuid;

pub async fn list(state: &AppState, user: Uuid) -> Result<Vec<UserAsset>> {
    sqlx::query_as::<_, AssetRow>(
        "SELECT * FROM user_assets WHERE owner_id=$1 ORDER BY kind,name,id",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(AssetRow::model)
    .collect()
}

async fn create(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<CreateAssetRequest>,
) -> Result<web::Json<UserAsset>> {
    validation::text(body.name.trim(), "Nome do asset", 1, 100)?;
    if let Some(v) = &body.shortcut {
        validation::text(v, "Atalho", 1, 32)?;
    }
    state
        .rate(&format!("asset:{}", session.user_id), 30, 60)
        .await?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let file: Option<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT owner_id,conversation_id,mime
        FROM files WHERE id=$1 FOR UPDATE",
    )
    .bind(body.file_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (owner, conversation, mime) = file.ok_or_else(Error::missing)?;
    if owner != session.user_id || conversation.is_some() {
        return Err(Error::forbidden());
    }
    let correct = match body.kind {
        AssetKind::Sound => mime.starts_with("audio/"),
        _ => mime.starts_with("image/"),
    };
    if !correct {
        return Err(Error::bad("Tipo do arquivo incompatível com asset"));
    }
    let row: AssetRow = sqlx::query_as(
        "INSERT INTO user_assets(id,owner_id,kind,name,shortcut,file_id)
        VALUES($1,$2,$3,$4,$5,$6) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(session.user_id)
    .bind(validation::wire(body.kind)?)
    .bind(body.name.trim())
    .bind(&body.shortcut)
    .bind(body.file_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    state
        .publish(vec![session.user_id], ServerEvent::ContactsChanged)
        .await;
    Ok(web::Json(row.model()?))
}

async fn delete(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let result = sqlx::query("DELETE FROM user_assets WHERE id=$1 AND owner_id=$2")
        .bind(*id)
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        return Err(Error::missing());
    }
    let row: crate::models::ProfileRow = sqlx::query_as("SELECT * FROM users WHERE id=$1")
        .bind(session.user_id)
        .fetch_one(&mut *tx)
        .await?;
    let my_badges = crate::badges::for_user_conn(&mut tx, session.user_id).await?;
    let recipients = presence::audience(&mut tx, session.user_id).await?;
    tx.commit().await?;
    state
        .publish(recipients, ServerEvent::ContactsChanged)
        .await;
    // Refresh through bootstrap rather than forwarding an unfiltered private presence.
    state
        .publish(
            vec![session.user_id],
            ServerEvent::ProfileUpdated {
                profile: row.model_with_badges(my_badges)?,
            },
        )
        .await;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/assets", web::post().to(create))
        .route("/assets/{id}", web::delete().to(delete));
}
