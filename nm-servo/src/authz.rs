use crate::error::{Error, Result};
use sqlx::PgConnection;
use uuid::Uuid;

// Access-changing transactions take the exclusive side. Mutations which send data
// take the shared side, so revocation/blocking cannot race with a committed send.
const PRIVACY_LOCK: i64 = 0x4e4d_5052_4956;
pub async fn shared(db: &mut PgConnection) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared($1)")
        .bind(PRIVACY_LOCK)
        .execute(db)
        .await?;
    Ok(())
}
pub async fn exclusive(db: &mut PgConnection) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PRIVACY_LOCK)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn blocked(db: &mut PgConnection, a: Uuid, b: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_blocks WHERE
        ((owner_id=$1 AND user_id=$2) OR (owner_id=$2 AND user_id=$1)))",
    )
    .bind(a)
    .bind(b)
    .fetch_one(db)
    .await?)
}

/// A block invalidates communication through groups as well as direct channels.
pub async fn member(db: &mut PgConnection, user: Uuid, conversation: Uuid) -> Result<Vec<Uuid>> {
    let members: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM conversation_members WHERE conversation_id=$1 ORDER BY user_id",
    )
    .bind(conversation)
    .fetch_all(&mut *db)
    .await?;
    if !members.contains(&user) {
        return Err(Error::forbidden());
    }
    let blocked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_blocks c
        JOIN conversation_members a ON a.user_id=c.owner_id AND a.conversation_id=$1
        JOIN conversation_members b ON b.user_id=c.user_id AND b.conversation_id=$1
        )",
    )
    .bind(conversation)
    .fetch_one(db)
    .await?;
    if blocked {
        return Err(Error::forbidden());
    }
    Ok(members)
}

/// Bilateral consent is required whenever either party disables strangers.
pub async fn peers(db: &mut PgConnection, user: Uuid, peers: &[Uuid]) -> Result<()> {
    if peers.is_empty() {
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE id=ANY($1)")
        .bind(peers)
        .fetch_one(&mut *db)
        .await?;
    if count != peers.len() as i64 {
        return Err(Error::missing());
    }
    let denied: bool = sqlx::query_scalar("SELECT EXISTS(
        SELECT 1 FROM users a JOIN users b ON b.id=ANY($2) WHERE a.id=$1 AND a.id<>b.id
        AND (EXISTS(SELECT 1 FROM user_blocks c WHERE
            ((c.owner_id=a.id AND c.user_id=b.id) OR (c.owner_id=b.id AND c.user_id=a.id)))
        OR ((NOT COALESCE((a.settings->>'allow_strangers')::boolean,false)
             OR NOT COALESCE((b.settings->>'allow_strangers')::boolean,false))
            AND (SELECT count(*) FROM contacts c WHERE c.state='accepted' AND
                ((c.owner_id=a.id AND c.user_id=b.id) OR (c.owner_id=b.id AND c.user_id=a.id)))<>2)))")
        .bind(user).bind(peers).fetch_one(db).await?;
    if denied {
        return Err(Error::forbidden());
    }
    Ok(())
}

pub async fn lock_conversation(
    db: &mut PgConnection,
    conversation: Uuid,
) -> Result<(String, Uuid)> {
    sqlx::query_as("SELECT kind,owner_id FROM conversations WHERE id=$1 FOR UPDATE")
        .bind(conversation)
        .fetch_optional(db)
        .await?
        .ok_or_else(Error::missing)
}
