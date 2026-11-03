use crate::error::Result;
use sqlx::PgConnection;
use uuid::Uuid;

/// Read preferences at send/delivery time, including events already queued.
pub async fn shares(db: &mut PgConnection, user: Uuid, setting: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT COALESCE((settings->>$2)::boolean,true) FROM users WHERE id=$1")
            .bind(user)
            .bind(setting)
            .fetch_one(db)
            .await?,
    )
}
