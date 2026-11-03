use crate::error::Result;
use chrono::{DateTime, Utc};
use nm_protocol::Badge;
use sqlx::{PgConnection, PgPool};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct BatchUserBadgeRow {
    user_id: Uuid,
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    icon: String,
    points: i32,
    awarded_at: Option<DateTime<Utc>>,
}

async fn sync_age_badges_users(db: &mut PgConnection, user_ids: &[Uuid]) -> Result<()> {
    if user_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO user_badges (user_id, badge_id)
        SELECT u.id, b.id
        FROM users u
        JOIN badges b ON (
            (b.slug = '1-ano-de-conta' AND u.created_at <= clock_timestamp() - INTERVAL '1 year')
            OR (b.slug = '2-anos-de-conta' AND u.created_at <= clock_timestamp() - INTERVAL '2 years')
            OR (b.slug = '3-anos-de-conta' AND u.created_at <= clock_timestamp() - INTERVAL '3 years')
            OR (b.slug = '4-anos-de-conta' AND u.created_at <= clock_timestamp() - INTERVAL '4 years')
            OR (b.slug = '5-anos-de-conta' AND u.created_at <= clock_timestamp() - INTERVAL '5 years')
        )
        WHERE u.id = ANY($1)
        ON CONFLICT DO NOTHING",
    )
    .bind(user_ids)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn for_user(db: &PgPool, user_id: Uuid) -> Result<Vec<Badge>> {
    Ok(for_users(db, &[user_id]).await?.remove(&user_id).unwrap_or_default())
}

pub async fn for_user_conn(db: &mut PgConnection, user_id: Uuid) -> Result<Vec<Badge>> {
    Ok(for_users_conn(db, &[user_id]).await?.remove(&user_id).unwrap_or_default())
}

pub async fn for_users(db: &PgPool, user_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<Badge>>> {
    let mut connection = db.acquire().await?;
    for_users_conn(&mut connection, user_ids).await
}

pub async fn for_users_conn(db: &mut PgConnection, user_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<Badge>>> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }
    sync_age_badges_users(&mut *db, user_ids).await?;

    let rows: Vec<BatchUserBadgeRow> = sqlx::query_as(
        "SELECT ub.user_id, b.id, b.slug, b.name, b.description, b.icon, b.points, ub.awarded_at
        FROM badges b
        JOIN user_badges ub ON ub.badge_id = b.id
        WHERE ub.user_id = ANY($1)
        ORDER BY b.points DESC, ub.awarded_at ASC",
    )
    .bind(user_ids)
    .fetch_all(db)
    .await?;

    let mut map: HashMap<Uuid, Vec<Badge>> = HashMap::new();
    for r in rows {
        map.entry(r.user_id).or_default().push(Badge {
            id: r.id,
            slug: r.slug,
            name: r.name,
            description: r.description,
            icon: r.icon,
            points: r.points,
            awarded_at: r.awarded_at,
        });
    }
    Ok(map)
}
