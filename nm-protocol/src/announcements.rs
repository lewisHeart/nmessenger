use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub link_url: Option<String>,
    pub image_file_id: Option<Uuid>,
    pub full_banner: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnouncementFeed {
    pub can_manage: bool,
    pub items: Vec<Announcement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveAnnouncementRequest {
    pub title: String,
    pub body: String,
    pub link_url: Option<String>,
    pub image_file_id: Option<Uuid>,
    pub full_banner: bool,
    pub enabled: bool,
}

impl Default for SaveAnnouncementRequest {
    fn default() -> Self {
        Self {
            title: String::new(),
            body: String::new(),
            link_url: None,
            image_file_id: None,
            full_banner: false,
            enabled: true,
        }
    }
}
