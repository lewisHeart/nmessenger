use crate::{
    error::Result,
    state::{AppState, EVENT_CHANNEL, Envelope},
};
use nm_protocol::{Message, ServerEvent};
use sqlx::{PgConnection, types::Json};
use std::time::Duration;
use uuid::Uuid;

/// The event is committed in the same transaction as its message.
pub async fn enqueue(db: &mut PgConnection, recipients: &[Uuid], message: &Message) -> Result<()> {
    sqlx::query("INSERT INTO message_outbox(message_id,recipients,event) VALUES($1,$2,$3)")
        .bind(message.id)
        .bind(recipients)
        .bind(Json(ServerEvent::Message {
            message: message.clone(),
        }))
        .execute(db)
        .await?;
    Ok(())
}

pub async fn enqueue_update(db: &mut PgConnection, recipients: &[Uuid], message: &Message) -> Result<()> {
    sqlx::query("INSERT INTO message_outbox(message_id,recipients,event) VALUES($1,$2,$3)
        ON CONFLICT(message_id) DO UPDATE SET recipients=EXCLUDED.recipients,event=EXCLUDED.event,
        delivered_at=NULL,next_attempt_at=clock_timestamp(),attempts=0")
        .bind(message.id).bind(recipients)
        .bind(Json(ServerEvent::MessageUpdated { message: message.clone() }))
        .execute(db).await?;
    Ok(())
}

pub async fn enqueue_deletion(db: &mut PgConnection, recipients: &[Uuid], conversation_id: Uuid, message_id: Uuid) -> Result<()> {
    sqlx::query("INSERT INTO message_deletion_outbox(id,recipients,event) VALUES($1,$2,$3)")
        .bind(Uuid::new_v4()).bind(recipients)
        .bind(Json(ServerEvent::MessageDeleted { conversation_id, message_id }))
        .execute(db).await?;
    Ok(())
}

pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if dispatch(&state).await.is_err() {
            tracing::warn!("message outbox delivery will retry");
        }
    }
}

async fn dispatch(state: &AppState) -> Result<()> {
    let mut tx = state.db.begin().await?;
    let rows: Vec<(Uuid, Vec<Uuid>, Json<ServerEvent>)> = sqlx::query_as(
        "SELECT message_id,recipients,event FROM message_outbox WHERE delivered_at IS NULL AND next_attempt_at<=clock_timestamp()
        ORDER BY created_at,message_id LIMIT 20 FOR UPDATE SKIP LOCKED")
        .fetch_all(&mut *tx).await?;
    for (id, recipients, Json(event)) in rows {
        let envelope = serde_json::to_string(&Envelope { recipients, event })
            .map_err(|_| crate::error::Error::internal())?;
        let sent: redis::RedisResult<i64> = redis::cmd("PUBLISH")
            .arg(EVENT_CHANNEL)
            .arg(envelope)
            .query_async(&mut state.redis.clone())
            .await;
        if sent.is_ok() {
            sqlx::query("UPDATE message_outbox SET delivered_at=clock_timestamp(),attempts=attempts+1 WHERE message_id=$1")
                .bind(id).execute(&mut *tx).await?;
        } else {
            sqlx::query("UPDATE message_outbox SET next_attempt_at=clock_timestamp()+make_interval(secs=>LEAST(30,power(2,LEAST(attempts,5))::int)),attempts=attempts+1 WHERE message_id=$1")
                .bind(id).execute(&mut *tx).await?;
        }
    }
    let deletions: Vec<(Uuid, Vec<Uuid>, Json<ServerEvent>)> = sqlx::query_as(
        "SELECT id,recipients,event FROM message_deletion_outbox WHERE delivered_at IS NULL AND next_attempt_at<=clock_timestamp()
         ORDER BY created_at,id LIMIT 20 FOR UPDATE SKIP LOCKED")
        .fetch_all(&mut *tx).await?;
    for (id, recipients, Json(event)) in deletions {
        let envelope = serde_json::to_string(&Envelope { recipients, event }).map_err(|_| crate::error::Error::internal())?;
        let sent: redis::RedisResult<i64> = redis::cmd("PUBLISH").arg(EVENT_CHANNEL).arg(envelope).query_async(&mut state.redis.clone()).await;
        if sent.is_ok() {
            sqlx::query("DELETE FROM message_deletion_outbox WHERE id=$1")
                .bind(id).execute(&mut *tx).await?;
        } else {
            sqlx::query("UPDATE message_deletion_outbox SET next_attempt_at=clock_timestamp()+make_interval(secs=>LEAST(30,power(2,LEAST(attempts,5))::int)),attempts=attempts+1 WHERE id=$1")
                .bind(id).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
