use crate::{error::Result, validation::parse};
use chrono::{DateTime, Utc};
use nm_protocol::*;
use serde_json::Value;
use sqlx::{FromRow, types::Json};
use uuid::Uuid;

#[derive(FromRow, Clone)]
pub struct ProfileRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub personal_message: String,
    pub presence: String,
    pub avatar_id: Option<Uuid>,
    pub scene: String,
    pub now_playing: Option<String>,
    #[sqlx(default)]
    pub bio: String,
    pub created_at: DateTime<Utc>,
}
impl ProfileRow {
    #[allow(dead_code)]
    pub fn model(self) -> Result<Profile> {
        self.model_with_badges(Vec::new())
    }

    pub fn model_with_badges(self, badges: Vec<Badge>) -> Result<Profile> {
        Ok(Profile {
            id: self.id,
            email: self.email,
            display_name: self.display_name,
            personal_message: self.personal_message,
            presence: parse(&self.presence)?,
            avatar_id: self.avatar_id,
            scene: self.scene,
            now_playing: self.now_playing,
            created_at: self.created_at,
            bio: self.bio,
            badges,
        })
    }
}

#[derive(FromRow, Clone)]
pub struct MessageRow {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub client_id: Uuid,
    pub kind: String,
    pub body: String,
    pub style: Json<MessageStyle>,
    pub attachment_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
impl MessageRow {
    pub fn model(self) -> Result<Message> {
        Ok(Message {
            id: self.id,
            conversation_id: self.conversation_id,
            sender_id: self.sender_id,
            client_id: self.client_id,
            kind: parse(&self.kind)?,
            body: self.body,
            style: self.style.0,
            attachment_id: self.attachment_id,
            created_at: self.created_at,
        })
    }
}

#[derive(FromRow, Clone)]
pub struct FileRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
}
impl FileRow {
    pub fn model(self) -> FileInfo {
        FileInfo {
            id: self.id,
            owner_id: self.owner_id,
            conversation_id: self.conversation_id,
            filename: self.filename,
            mime: self.mime,
            size: self.size,
            sha256: self.sha256,
            created_at: self.created_at,
        }
    }
}

#[derive(FromRow)]
pub struct AssetRow {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub shortcut: Option<String>,
    pub file_id: Uuid,
}
impl AssetRow {
    pub fn model(self) -> Result<UserAsset> {
        Ok(UserAsset {
            id: self.id,
            kind: parse(&self.kind)?,
            name: self.name,
            shortcut: self.shortcut,
            file_id: self.file_id,
        })
    }
}

#[derive(FromRow, Clone)]
pub struct ActivityRow {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub initiator_id: Uuid,
    pub kind: String,
    pub status: String,
    pub payload: Json<Value>,
    pub created_at: DateTime<Utc>,
}
impl ActivityRow {
    pub fn model(self) -> Result<Activity> {
        Ok(Activity {
            id: self.id,
            conversation_id: self.conversation_id,
            initiator_id: self.initiator_id,
            kind: parse(&self.kind)?,
            status: parse(&self.status)?,
            payload: self.payload.0,
            created_at: self.created_at,
        })
    }
}
