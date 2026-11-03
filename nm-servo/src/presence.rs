use crate::{error::Result, models::ProfileRow, state::AppState};
use nm_protocol::{Presence, Profile, ServerEvent};
use sqlx::PgConnection;
use uuid::Uuid;

const TTL: i64 = 75;
fn key(user: Uuid) -> String {
    format!("nm:presence:{user}")
}

pub async fn audience(db: &mut PgConnection, user: Uuid) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT c.owner_id FROM contacts c WHERE c.user_id=$1
        AND c.state='accepted' AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE
            (b.owner_id=$1 AND b.user_id=c.owner_id) OR (b.owner_id=c.owner_id AND b.user_id=$1))
        AND EXISTS(SELECT 1 FROM contacts r WHERE r.owner_id=$1 AND r.user_id=c.owner_id
            AND r.state='accepted' AND NOT r.blocked)
        UNION SELECT $1",
    )
    .bind(user)
    .fetch_all(db)
    .await?)
}

pub async fn refresh(state: &AppState, user: Uuid, connection: Uuid) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let accepted: i64 = redis::Script::new("redis.call('ZREMRANGEBYSCORE',KEYS[1],'-inf',ARGV[1]);
        if not redis.call('ZSCORE',KEYS[1],ARGV[3]) and redis.call('ZCARD',KEYS[1])>=8 then return 0 end;
        redis.call('ZADD',KEYS[1],ARGV[2],ARGV[3]); redis.call('EXPIRE',KEYS[1],80);
        redis.call('ZADD',KEYS[2],ARGV[2],ARGV[4]); return 1")
        .key(key(user)).key("nm:online_users").arg(now).arg(now + TTL).arg(connection.to_string()).arg(user.to_string())
        .invoke_async(&mut state.redis.clone()).await?;
    if accepted != 1 {
        return Err(crate::error::Error::limit());
    }
    Ok(())
}

pub async fn disconnect(state: &AppState, user: Uuid, connection: Uuid) -> Result<()> {
    let _: i64 = redis::cmd("ZREM")
        .arg(key(user))
        .arg(connection.to_string())
        .query_async(&mut state.redis.clone())
        .await?;
    announce(state, user).await
}

pub async fn online(state: &AppState, user: Uuid) -> Result<bool> {
    let count: i64 = redis::cmd("ZCOUNT")
        .arg(key(user))
        .arg(format!("({}", chrono::Utc::now().timestamp()))
        .arg("+inf")
        .query_async(&mut state.redis.clone())
        .await?;
    Ok(count > 0)
}

/// One Redis pipeline and one SQL query for a whole contact/member batch.
pub async fn profiles(
    state: &AppState,
    viewer: Uuid,
    rows: Vec<ProfileRow>,
) -> Result<Vec<Profile>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let hidden: Vec<Uuid> = sqlx::query_scalar(
        "SELECT u.id FROM users u WHERE u.id=ANY($2)
        AND u.id<>$1 AND (EXISTS(SELECT 1 FROM user_blocks c WHERE
            ((c.owner_id=$1 AND c.user_id=u.id) OR (c.owner_id=u.id AND c.user_id=$1)))
        OR (SELECT count(*) FROM contacts c WHERE c.state='accepted' AND
            ((c.owner_id=$1 AND c.user_id=u.id) OR (c.owner_id=u.id AND c.user_id=$1)))<>2)",
    )
    .bind(viewer)
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    let mut pipeline = redis::pipe();
    let now = format!("({}", chrono::Utc::now().timestamp());
    for id in &ids {
        pipeline.cmd("ZCOUNT").arg(key(*id)).arg(&now).arg("+inf");
    }
    let counts: Vec<i64> = pipeline.query_async(&mut state.redis.clone()).await?;
    let mut badge_map = crate::badges::for_users(&state.db, &ids).await?;
    rows.into_iter()
        .zip(counts)
        .map(|(row, count)| {
            let user_badges = badge_map.remove(&row.id).unwrap_or_default();
            let mut p = row.model_with_badges(user_badges)?;
            if p.id != viewer {
                p.presence = if hidden.contains(&p.id) || count == 0 {
                    Presence::Offline
                } else {
                    p.presence.visible()
                };
            }
            Ok(p)
        })
        .collect()
}

pub async fn announce(state: &AppState, user: Uuid) -> Result<()> {
    let raw: String = sqlx::query_scalar("SELECT presence FROM users WHERE id=$1")
        .bind(user)
        .fetch_one(&state.db)
        .await?;
    let desired: Presence = crate::validation::parse(&raw)?;
    let presence = if online(state, user).await? {
        desired.visible()
    } else {
        Presence::Offline
    };
    let recipients = audience(&mut *state.db.acquire().await?, user).await?;
    state
        .publish(
            recipients,
            ServerEvent::Presence {
                user_id: user,
                presence,
            },
        )
        .await;
    Ok(())
}

/// Crash/offline detection is shared across instances, with an atomic claim.
pub async fn sweep(state: &AppState) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let users: Vec<String> = redis::cmd("ZRANGEBYSCORE")
        .arg("nm:online_users")
        .arg("-inf")
        .arg(now)
        .arg("LIMIT")
        .arg(0)
        .arg(100)
        .query_async(&mut state.redis.clone())
        .await?;
    for user in users {
        let claimed: i64 = redis::Script::new("local n=redis.call('ZSCORE',KEYS[1],ARGV[1]);
            if n and tonumber(n)<=tonumber(ARGV[2]) then return redis.call('ZREM',KEYS[1],ARGV[1]) end; return 0")
            .key("nm:online_users").arg(&user).arg(now)
            .invoke_async(&mut state.redis.clone()).await?;
        if claimed == 1
            && let Ok(id) = Uuid::parse_str(&user)
        {
            let _ = announce(state, id).await;
        }
    }
    Ok(())
}
