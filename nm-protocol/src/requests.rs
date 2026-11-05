use crate::{
    ActivityKind, ActivityStatus, AssetKind, ContactDetails, MessageKind, MessageStyle, Presence,
    Profile,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub session_id: Uuid,
    pub user: Profile,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub personal_message: Option<String>,
    pub presence: Option<Presence>,
    pub avatar_id: Option<Uuid>,
    pub scene: Option<String>,
    pub now_playing: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddContactRequest {
    pub email: String,
    pub nickname: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UpdateContactRequest {
    pub nickname: Option<String>,
    pub favorite: Option<bool>,
    pub blocked: Option<bool>,
    pub accepted: Option<bool>,
    pub category_ids: Option<Vec<Uuid>>,
    pub details: Option<ContactDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportContactsRequest {
    pub emails: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRequest {
    pub name: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    pub member_ids: Vec<Uuid>,
    pub title: Option<String>,
    #[serde(default = "default_true")]
    pub members_can_invite: bool,
}

impl Default for CreateConversationRequest {
    fn default() -> Self {
        Self {
            member_ids: Vec::new(),
            title: None,
            members_can_invite: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConversationRequest {
    pub title: String,
    #[serde(default)]
    pub members_can_invite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub client_id: Uuid,
    pub kind: MessageKind,
    pub body: String,
    #[serde(default)]
    pub style: MessageStyle,
    pub attachment_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadRequest {
    pub message_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssetRequest {
    pub kind: AssetKind,
    pub name: String,
    pub shortcut: Option<String>,
    pub file_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRequest {
    pub kind: ActivityKind,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateActivityRequest {
    pub status: ActivityStatus,
    pub payload: Option<Value>,
}
