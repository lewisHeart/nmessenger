use crate::{Activity, Message, Presence, Profile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaStream {
    Audio,
    Camera,
    Screen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientEvent {
    Authenticate {
        token: String,
    },
    Typing {
        conversation_id: Uuid,
        is_typing: bool,
    },
    Signal {
        activity_id: Uuid,
        data: Value,
    },
    Media {
        activity_id: Uuid,
        stream: MediaStream,
        sequence: u64,
        data: String,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Authenticated {
        user_id: Uuid,
    },
    Message {
        message: Message,
    },
    MessageUpdated { message: Message },
    MessageDeleted { conversation_id: Uuid, message_id: Uuid },
    Presence {
        user_id: Uuid,
        presence: Presence,
    },
    ProfileUpdated {
        profile: Profile,
    },
    ContactsChanged,
    ConversationsChanged,
    AnnouncementsChanged,
    ReleaseChanged,
    Typing {
        conversation_id: Uuid,
        user_id: Uuid,
        is_typing: bool,
    },
    Read {
        conversation_id: Uuid,
        user_id: Uuid,
        message_id: Option<Uuid>,
    },
    Activity {
        activity: Activity,
    },
    Signal {
        activity_id: Uuid,
        from_user_id: Uuid,
        data: Value,
    },
    Media {
        activity_id: Uuid,
        from_user_id: Uuid,
        stream: MediaStream,
        sequence: u64,
        data: String,
    },
    SessionRevoked {
        session_id: Uuid,
    },
    Error {
        code: String,
        message: String,
    },
    ResyncRequired,
    Pong,
}
