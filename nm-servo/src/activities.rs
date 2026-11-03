use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    models::ActivityRow,
    state::AppState,
    validation,
};
use actix_web::web;
use nm_protocol::*;
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

pub async fn list(state: &AppState, user: Uuid) -> Result<Vec<Activity>> {
    let rows: Vec<ActivityRow>=sqlx::query_as("SELECT a.* FROM activities a
        JOIN activity_participants p ON p.activity_id=a.id AND p.user_id=$1
        JOIN conversation_members m ON m.conversation_id=a.conversation_id AND m.user_id=$1
        WHERE a.status IN('offered','accepted') AND p.status IN('offered','accepted')
        AND NOT EXISTS(SELECT 1 FROM user_blocks b
            JOIN conversation_members x ON x.user_id=b.owner_id AND x.conversation_id=a.conversation_id
            JOIN conversation_members y ON y.user_id=b.user_id AND y.conversation_id=a.conversation_id)
        ORDER BY a.created_at,a.id")
        .bind(user).fetch_all(&state.db).await?;
    rows.into_iter().map(ActivityRow::model).collect()
}

fn payload(value: &serde_json::Value) -> Result<()> {
    validation::json_size(value, 16 * 1024)?;
    if !value.is_object() && !value.is_null() {
        return Err(Error::bad("Payload deve ser objeto JSON"));
    }
    Ok(())
}

async fn audience(db: &mut PgConnection, activity: Uuid) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT p.user_id FROM activity_participants p JOIN activities a ON a.id=p.activity_id
        JOIN conversation_members m ON m.conversation_id=a.conversation_id AND m.user_id=p.user_id
        WHERE p.activity_id=$1",
    )
    .bind(activity)
    .fetch_all(db)
    .await?)
}

fn is_media_call(kind: ActivityKind) -> bool {
    matches!(kind, ActivityKind::AudioCall | ActivityKind::VideoCall | ActivityKind::RemoteAssistance)
}

/// Serialize call reservations across conversations and server instances.
async fn reserve_call_users(db: &mut PgConnection, users: &[Uuid], except: Option<Uuid>) -> Result<()> {
    let mut users = users.to_vec();
    users.sort_unstable();
    users.dedup();
    for user in &users {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("nm:call:{user}"))
            .execute(&mut *db).await?;
    }
    let busy: bool = sqlx::query_scalar("SELECT EXISTS(
        SELECT 1 FROM activities a
        JOIN activity_participants p ON p.activity_id=a.id
        JOIN conversation_members m ON m.conversation_id=a.conversation_id AND m.user_id=p.user_id
        WHERE a.kind IN ('audio_call','video_call','remote_assistance')
        AND a.status IN ('offered','accepted') AND p.status='accepted'
        AND p.user_id=ANY($1) AND ($2::uuid IS NULL OR a.id<>$2)
    )").bind(&users).bind(except).fetch_one(&mut *db).await?;
    if busy {
        return Err(Error::conflict("Um participante já está em outra chamada"));
    }
    Ok(())
}

async fn create(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<ActivityRequest>,
) -> Result<web::Json<Activity>> {
    payload(&body.payload)?;
    state
        .rate(&format!("activity:{}", session.user_id), 10, 60)
        .await?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    authz::lock_conversation(&mut tx, *id).await?;
    let members = authz::member(&mut tx, session.user_id, *id).await?;
    if members.len() < 2 {
        return Err(Error::bad("Atividade exige outro participante"));
    }
    authz::peers(
        &mut tx,
        session.user_id,
        &members
            .iter()
            .copied()
            .filter(|u| *u != session.user_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    if is_media_call(body.kind) {
        reserve_call_users(&mut tx, &members, None).await?;
    }
    let count: i64=sqlx::query_scalar("SELECT count(*) FROM activities WHERE conversation_id=$1 AND status IN('offered','accepted')")
        .bind(*id).fetch_one(&mut *tx).await?;
    if count >= 10 {
        return Err(Error::conflict("Muitas atividades abertas nesta conversa"));
    }
    let row: ActivityRow = sqlx::query_as(
        "INSERT INTO activities(id,conversation_id,initiator_id,kind,payload)
        VALUES($1,$2,$3,$4,$5) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(*id)
    .bind(session.user_id)
    .bind(validation::wire(body.kind)?)
    .bind(Json(&body.payload))
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO activity_participants(activity_id,user_id,status)
        SELECT $1,u,CASE WHEN u=$3 THEN 'accepted' ELSE 'offered' END FROM unnest($2::uuid[]) u",
    )
    .bind(row.id)
    .bind(&members)
    .bind(session.user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let activity = row.model()?;
    state
        .publish(
            members,
            ServerEvent::Activity {
                activity: activity.clone(),
            },
        )
        .await;
    Ok(web::Json(activity))
}

async fn update(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<UpdateActivityRequest>,
) -> Result<web::Json<Activity>> {
    if let Some(v) = &body.payload {
        payload(v)?;
    }
    state
        .rate(&format!("activity-update:{}", session.user_id), 60, 60)
        .await?;
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let row: ActivityRow = sqlx::query_as("SELECT * FROM activities WHERE id=$1 FOR UPDATE")
        .bind(*id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(Error::missing)?;
    let members = authz::member(&mut tx, session.user_id, row.conversation_id).await?;
    let participant: String = sqlx::query_scalar(
        "SELECT status FROM activity_participants WHERE activity_id=$1 AND user_id=$2",
    )
    .bind(*id)
    .bind(session.user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(Error::forbidden)?;
    let requested = validation::wire(body.status)?;
    if row.status == "ended" || row.status == "declined" {
        if requested != row.status {
            return Err(Error::conflict("Atividade já encerrada"));
        }
        tx.commit().await?;
        return Ok(web::Json(row.model()?));
    }
    let kind: ActivityKind = validation::parse(&row.kind)?;
    if is_media_call(kind) && body.status == ActivityStatus::Accepted && participant == "offered" {
        reserve_call_users(&mut tx, &members, Some(*id)).await?;
    }
    let next = match body.status {
        ActivityStatus::Offered => return Err(Error::bad("Não é possível reabrir convite")),
        ActivityStatus::Accepted | ActivityStatus::Declined => {
            if row.initiator_id == session.user_id {
                return Err(Error::forbidden());
            }
            if participant != requested && participant != "offered" {
                return Err(Error::conflict("Convite já respondido"));
            }
            sqlx::query(
                "UPDATE activity_participants SET status=$3 WHERE activity_id=$1 AND user_id=$2",
            )
            .bind(*id)
            .bind(session.user_id)
            .bind(&requested)
            .execute(&mut *tx)
            .await?;
            let (accepted,offered): (i64,i64)=sqlx::query_as("SELECT
                count(*) FILTER(WHERE status='accepted' AND user_id<>$2),
                count(*) FILTER(WHERE status='offered') FROM activity_participants WHERE activity_id=$1")
                .bind(*id).bind(row.initiator_id).fetch_one(&mut *tx).await?;
            if accepted > 0 {
                "accepted"
            } else if offered > 0 {
                "offered"
            } else {
                "declined"
            }
        }
        ActivityStatus::Ended => {
            if participant != "accepted" {
                return Err(Error::forbidden());
            }
            sqlx::query("UPDATE activity_participants SET status='ended' WHERE activity_id=$1")
                .bind(*id)
                .execute(&mut *tx)
                .await?;
            "ended"
        }
    };
    let row: ActivityRow = sqlx::query_as(
        "UPDATE activities SET status=$2,payload=COALESCE($3,payload) WHERE id=$1 RETURNING *",
    )
    .bind(*id)
    .bind(next)
    .bind(body.payload.as_ref().map(Json))
    .fetch_one(&mut *tx)
    .await?;
    let recipients = audience(&mut tx, *id).await?;
    tx.commit().await?;
    let activity = row.model()?;
    state
        .publish(
            recipients,
            ServerEvent::Activity {
                activity: activity.clone(),
            },
        )
        .await;
    Ok(web::Json(activity))
}

/// Both ends must have explicitly accepted, and still be conversation members.
pub async fn active(db: &mut PgConnection, user: Uuid, id: Uuid) -> Result<(Activity, Vec<Uuid>)> {
    let row: ActivityRow = sqlx::query_as(
        "SELECT a.* FROM activities a JOIN activity_participants p
        ON p.activity_id=a.id AND p.user_id=$2 AND p.status='accepted'
        WHERE a.id=$1 AND a.status='accepted'",
    )
    .bind(id)
    .bind(user)
    .fetch_optional(&mut *db)
    .await?
    .ok_or_else(Error::forbidden)?;
    let members = authz::member(db, user, row.conversation_id).await?;
    let accepted: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM activity_participants
        WHERE activity_id=$1 AND status='accepted' AND user_id=ANY($2)",
    )
    .bind(id)
    .bind(&members)
    .fetch_all(db)
    .await?;
    if accepted.len() < 2 {
        return Err(Error::forbidden());
    }
    Ok((row.model()?, accepted))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/conversations/{id}/activities", web::post().to(create))
        .route("/activities/{id}", web::patch().to(update));
}
