use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminOverview {
    pub users: i64,
    pub suspended_users: i64,
    pub active_sessions: i64,
    pub conversations: i64,
    pub messages: i64,
    pub files: i64,
    pub stored_bytes: i64,
    pub published_announcements: i64,
    pub user_upload_quota: i64,
    pub session_ttl_secs: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub disabled_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub banned: bool,
    pub administrator: bool,
    pub active_sessions: i64,
    pub stored_bytes: i64,
    #[serde(default)]
    pub badges: Vec<crate::Badge>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AdminUpdateUser {
    pub disabled: Option<bool>,
    pub administrator: Option<bool>,
    pub confirmation_password: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub device_name: String,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminAuditEntry {
    pub id: i64,
    pub actor_id: Option<Uuid>,
    pub actor_email: String,
    pub action: String,
    pub target_user: Option<Uuid>,
    pub target_session: Option<Uuid>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminPage<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminBadge {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub points: i32,
    pub users_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateBadgeRequest {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub points: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateBadgeRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub points: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserBadgeMutation {
    pub badge_id: Uuid,
    pub grant: bool,
}
