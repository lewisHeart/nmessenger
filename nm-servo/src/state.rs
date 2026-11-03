use crate::error::{Error, Result};
use nm_protocol::ServerEvent;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, time::Duration};
use tokio::sync::broadcast;
use uuid::Uuid;

pub const EVENT_CHANNEL: &str = "nm:events:v1";
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

// Accept the known historical checksum for migration 1 from before the migration consolidation.
// Other modified migrations still fail SQLx validation.
async fn upgrade_legacy_migration_checksum(db: &sqlx::PgPool) -> Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(db)
        .await?;
    if exists {
        let old = hex::decode("a1c922ec218d0e5acae77a6c65f4698e801e22e25c0d59255bb4084b6b2af05017670560f5aa6c8d6901d5bf32906250")
            .map_err(|_| Error::internal())?;
        let current = MIGRATOR
            .iter()
            .find(|migration| migration.version == 1)
            .ok_or_else(Error::internal)?;

        let has_legacy_split: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version > 1)")
            .fetch_one(db)
            .await
            .unwrap_or(false);

        if has_legacy_split {
            let _ = sqlx::query("DELETE FROM _sqlx_migrations WHERE version > 1").execute(db).await;
            let _ = sqlx::query("UPDATE _sqlx_migrations SET checksum=$1 WHERE version=1")
                .bind(current.checksum.as_ref()).execute(db).await;
        } else {
            sqlx::query("UPDATE _sqlx_migrations SET checksum=$1 WHERE version=1 AND success AND checksum=$2")
                .bind(current.checksum.as_ref()).bind(old).execute(db).await?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub bind_addr: String,
    pub upload_dir: PathBuf,
    pub session_ttl_secs: i64,
    pub user_upload_quota: i64,
}
impl Config {
    pub fn from_env() -> std::result::Result<Self, String> {
        let required = |name| std::env::var(name).map_err(|_| format!("{name} é obrigatório"));
        let ttl = std::env::var("SESSION_TTL_SECS")
            .unwrap_or_else(|_| "2592000".into())
            .parse::<i64>()
            .map_err(|_| "SESSION_TTL_SECS inválido")?;
        if !(60..=7_776_000).contains(&ttl) {
            return Err("SESSION_TTL_SECS deve ser 60–7776000".into());
        }
        Ok(Self {
            user_upload_quota: std::env::var("USER_UPLOAD_QUOTA_BYTES")
                .unwrap_or_else(|_| "536870912".into())
                .parse::<i64>()
                .ok()
                .filter(|quota| (26_214_400..=1_099_511_627_776).contains(quota))
                .ok_or("USER_UPLOAD_QUOTA_BYTES deve ser entre 25 MiB e 1 TiB")?,
            database_url: required("DATABASE_URL")?,
            redis_url: required("REDIS_URL")?,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| {
                if let Ok(port) = std::env::var("PORT") {
                    format!("0.0.0.0:{port}")
                } else {
                    "0.0.0.0:8080".into()
                }
            }),
            upload_dir: std::env::var("UPLOAD_DIR")
                .unwrap_or_else(|_| "data/uploads".into())
                .into(),
            session_ttl_secs: ttl,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub recipients: Vec<Uuid>,
    pub event: ServerEvent,
}

pub struct HealthStatus {
    pub checked_at: std::time::Instant,
    pub postgres: bool,
    pub redis: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub health: std::sync::Arc<tokio::sync::Mutex<Option<HealthStatus>>>,
    pub db: sqlx::PgPool,
    pub redis: ConnectionManager,
    pub redis_client: redis::Client,
    pub events: broadcast::Sender<Envelope>,
    pub upload_dir: PathBuf,
    pub session_ttl_secs: i64,
    pub user_upload_quota: i64,
}

impl AppState {
    /// Migrations run for every server/test instance; SQLx takes a PostgreSQL advisory lock.
    pub async fn connect(config: &Config) -> Result<Self> {
        let db = PgPoolOptions::new()
            .max_connections(20)
            .after_connect(|connection, _| {
                Box::pin(async move {
                    sqlx::query("SET statement_timeout='15s'")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SET lock_timeout='10s'")
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .acquire_timeout(Duration::from_secs(10))
            .connect(&config.database_url)
            .await?;
        upgrade_legacy_migration_checksum(&db).await?;
        MIGRATOR.run(&db).await.map_err(|error| {
            tracing::error!(%error,"database migration failed");
            Error::internal()
        })?;
        let redis_client = redis::Client::open(config.redis_url.as_str())?;
        let redis = ConnectionManager::new(redis_client.clone()).await?;
        let _: String = redis::cmd("PING").query_async(&mut redis.clone()).await?;
        tokio::fs::create_dir_all(&config.upload_dir).await?;
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            db,
            health: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            redis,
            redis_client,
            events,
            upload_dir: config.upload_dir.clone(),
            session_ttl_secs: config.session_ttl_secs,
            user_upload_quota: config.user_upload_quota,
        })
    }

    pub async fn publish(&self, recipients: Vec<Uuid>, event: ServerEvent) {
        if recipients.is_empty() && !matches!(event, ServerEvent::ReleaseChanged) {
            return;
        }
        let envelope = Envelope { recipients, event };
        let Ok(json) = serde_json::to_string(&envelope) else {
            return;
        };
        let result: redis::RedisResult<i64> = redis::cmd("PUBLISH")
            .arg(EVENT_CHANNEL)
            .arg(json)
            .query_async(&mut self.redis.clone())
            .await;
        if result.is_err() {
            tracing::warn!("event publication failed; requesting persisted-state resync");
            let _ = self.events.send(Envelope {
                recipients: envelope.recipients,
                event: ServerEvent::ResyncRequired,
            });
        }
    }

    /// Fixed windows are atomic across processes, including the key expiry.
    pub async fn rate(&self, key: &str, maximum: u64, seconds: u64) -> Result<()> {
        let count: u64 = redis::Script::new(
            "local n=redis.call('INCR',KEYS[1]); if n==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]) end; return n"
        ).key(format!("nm:rate:{key}")).arg(seconds).invoke_async(&mut self.redis.clone()).await?;
        if count > maximum {
            return Err(Error::limit());
        }
        Ok(())
    }

    pub async fn cooldown(&self, key: &str, seconds: u64) -> Result<()> {
        let result: Option<String> = redis::cmd("SET")
            .arg(format!("nm:cooldown:{key}"))
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(seconds)
            .query_async(&mut self.redis.clone())
            .await?;
        if result.is_none() {
            return Err(Error::limit());
        }
        Ok(())
    }
}
