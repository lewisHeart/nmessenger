use crate::{
    activities, assets,
    auth::{self, Session},
    authz, contacts, conversations,
    error::{Error, Result},
    models::ProfileRow,
    presence,
    state::AppState,
    validation,
};
use actix_web::web;
use nm_protocol::{Bootstrap, Profile, ServerEvent, UpdateProfileRequest, UserSettings};
use sqlx::types::Json;
use uuid::Uuid;

pub async fn settings(state: &AppState, user: uuid::Uuid) -> Result<UserSettings> {
    let Json(value) =
        sqlx::query_scalar::<_, Json<UserSettings>>("SELECT settings FROM users WHERE id=$1")
            .bind(user)
            .fetch_one(&state.db)
            .await?;
    Ok(value)
}

async fn bootstrap(state: web::Data<AppState>, session: Session) -> Result<web::Json<Bootstrap>> {
    let me: ProfileRow = sqlx::query_as("SELECT * FROM users WHERE id=$1")
        .bind(session.user_id)
        .fetch_one(&state.db)
        .await?;
    let my_badges = crate::badges::for_user(&state.db, session.user_id).await?;
    Ok(web::Json(Bootstrap {
        me: me.model_with_badges(my_badges)?,
        contacts: contacts::list(&state, session.user_id).await?,
        categories: contacts::categories(&state, session.user_id).await?,
        conversations: conversations::list(&state, session.user_id).await?,
        settings: settings(&state, session.user_id).await?,
        sessions: auth::list(&state, &session).await?,
        assets: assets::list(&state, session.user_id).await?,
        activities: activities::list(&state, session.user_id).await?,
    }))
}

async fn update(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<UpdateProfileRequest>,
) -> Result<web::Json<Profile>> {
    if let Some(v) = &body.display_name {
        validation::text(v.trim(), "Nome", 1, 100)?;
    }
    if let Some(v) = &body.personal_message {
        validation::text(v, "Mensagem pessoal", 0, 512)?;
    }
    if let Some(v) = &body.scene {
        validation::text(v, "Cenário", 0, 256)?;
    }
    if let Some(v) = &body.now_playing {
        validation::text(v, "Música", 0, 256)?;
    }
    if let Some(v) = &body.bio {
        validation::text(v, "Sobre mim", 0, 10000)?;
    }
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    if let Some(id) = body.avatar_id {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM user_assets
            WHERE id=$1 AND owner_id=$2 AND kind='avatar')",
        )
        .bind(id)
        .bind(session.user_id)
        .fetch_one(&mut *tx)
        .await?;
        if !owned {
            return Err(Error::forbidden());
        }
    }
    let presence = body.presence.map(validation::wire).transpose()?;
    let row: ProfileRow = sqlx::query_as(
        "UPDATE users SET
        display_name=COALESCE($2,display_name),personal_message=COALESCE($3,personal_message),
        presence=COALESCE($4,presence),avatar_id=COALESCE($5,avatar_id),
        scene=COALESCE($6,scene),now_playing=COALESCE($7,now_playing),
        bio=COALESCE($8,bio) WHERE id=$1 RETURNING *",
    )
    .bind(session.user_id)
    .bind(body.display_name.as_ref().map(|s| s.trim()))
    .bind(&body.personal_message)
    .bind(presence)
    .bind(body.avatar_id)
    .bind(&body.scene)
    .bind(&body.now_playing)
    .bind(&body.bio)
    .fetch_one(&mut *tx)
    .await?;
    let audience = presence::audience(&mut tx, session.user_id).await?;
    let my_badges = crate::badges::for_user_conn(&mut tx, session.user_id).await?;
    tx.commit().await?;
    let profile = row.model_with_badges(my_badges)?;
    state
        .publish(
            vec![session.user_id],
            ServerEvent::ProfileUpdated {
                profile: profile.clone(),
            },
        )
        .await;
    let mut public = profile.clone();
    public.presence = if presence::online(&state, session.user_id).await? {
        profile.presence.visible()
    } else {
        nm_protocol::Presence::Offline
    };
    state
        .publish(
            audience
                .into_iter()
                .filter(|id| *id != session.user_id)
                .collect(),
            ServerEvent::ProfileUpdated { profile: public },
        )
        .await;
    presence::announce(&state, session.user_id).await?;
    Ok(web::Json(profile))
}

async fn get_settings(
    state: web::Data<AppState>,
    session: Session,
) -> Result<web::Json<UserSettings>> {
    Ok(web::Json(settings(&state, session.user_id).await?))
}
async fn put_settings(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<UserSettings>,
) -> Result<web::Json<UserSettings>> {
    validation::settings(&body)?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    sqlx::query("UPDATE users SET settings=$2 WHERE id=$1")
        .bind(session.user_id)
        .bind(Json(&*body))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    state
        .publish(vec![session.user_id], ServerEvent::ConversationsChanged)
        .await;
    Ok(body)
}

async fn get_profile(
    state: web::Data<AppState>,
    session: Session,
    target: web::Path<Uuid>,
) -> Result<web::Json<Profile>> {
    let row: Option<ProfileRow> = sqlx::query_as("SELECT * FROM users WHERE id=$1")
        .bind(*target)
        .fetch_optional(&state.db)
        .await?;
    let row = row.ok_or_else(Error::missing)?;
    let profiles = presence::profiles(&state, session.user_id, vec![row]).await?;
    let profile = profiles.into_iter().next().ok_or_else(Error::missing)?;
    Ok(web::Json(profile))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/bootstrap", web::get().to(bootstrap))
        .route("/me", web::patch().to(update))
        .route("/profiles/{id}", web::get().to(get_profile))
        .service(
            web::resource("/settings")
                .route(web::get().to(get_settings))
                .route(web::put().to(put_settings)),
        );
}
