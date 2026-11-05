//! Wire contract shared by the native desktop client and the HTTP/WebSocket API.
mod announcements;
mod admin;
mod encryption;
mod events;
mod models;
mod requests;
pub mod secure_auth;

pub use announcements::*;
pub use admin::*;
pub use encryption::*;
pub use events::*;
pub use models::*;
pub use requests::*;

pub const API_VERSION: &str = "/v1";
pub const MAX_MESSAGE_CHARS: usize = 16_000;
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;
pub const MAX_MEDIA_FRAME_BYTES: usize = 256 * 1024;
pub const NUDGE_COOLDOWN_SECS: u64 = 10;
pub const MESSAGE_EDIT_WINDOW_SECS: i64 = 15 * 60;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}
