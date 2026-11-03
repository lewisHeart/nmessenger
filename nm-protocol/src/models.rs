use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    #[default]
    Available,
    Busy,
    Away,
    Invisible,
    Offline,
}

impl Presence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Available => "Disponível",
            Self::Busy => "Ocupado",
            Self::Away => "Ausente",
            Self::Invisible => "Invisível",
            Self::Offline => "Offline",
        }
    }

    pub fn visible(self) -> Self {
        if self == Self::Invisible {
            Self::Offline
        } else {
            self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Badge {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub points: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awarded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub personal_message: String,
    pub presence: Presence,
    pub avatar_id: Option<Uuid>,
    pub scene: String,
    pub now_playing: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub badges: Vec<Badge>,
}

impl Profile {
    pub fn badge_points(&self) -> i32 {
        self.badges.iter().map(|b| b.points).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactState {
    PendingIncoming,
    PendingOutgoing,
    Accepted,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContactDetails {
    pub full_name: String,
    pub mobile: String,
    pub home_email: String,
    pub work_email: String,
    pub birthday: String,
    pub website: String,
    pub company: String,
    pub job_title: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub user: Profile,
    pub nickname: Option<String>,
    pub category_ids: Vec<Uuid>,
    pub favorite: bool,
    pub blocked: bool,
    pub state: ContactState,
    pub details: ContactDetails,
}

impl Contact {
    pub fn name(&self) -> &str {
        self.nickname
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.user.display_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: Uuid,
    pub name: String,
    pub position: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub title: Option<String>,
    pub kind: ConversationKind,
    pub members: Vec<Profile>,
    pub updated_at: DateTime<Utc>,
    pub unread: i64,
    pub last_message: Option<Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Text,
    Nudge,
    Wink,
    Ink,
    Voice,
    File,
    Photo,
    System,
    Activity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MessageStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font_family: String,
    pub font_size: f32,
    pub color: [u8; 3],
    pub rtl: bool,
}

impl Default for MessageStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            font_family: "sans-serif".into(),
            font_size: 12.0,
            color: [30, 40, 55],
            rtl: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub client_id: Uuid,
    pub kind: MessageKind,
    pub body: String,
    pub style: MessageStyle,
    pub attachment_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub conversation_id: Option<Uuid>,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Avatar,
    Emoticon,
    Background,
    Sound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAsset {
    pub id: Uuid,
    pub kind: AssetKind,
    pub name: String,
    pub shortcut: Option<String>,
    pub file_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    AudioCall,
    VideoCall,
    Game,
    RemoteAssistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Offered,
    Accepted,
    Declined,
    Ended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub initiator_id: Uuid,
    pub kind: ActivityKind,
    pub status: ActivityStatus,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub device_name: String,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub allow_strangers: bool,
    pub read_receipts: bool,
    pub typing_indicators: bool,
    pub notification_previews: bool,
    pub music_status: bool,
    pub show_timestamps: bool,
    pub group_messages: bool,
    pub emoticons: bool,
    pub nudges: bool,
    pub sounds: bool,
    pub online_alerts: bool,
    pub message_alerts: bool,
    pub keep_history: bool,
    pub auto_away_minutes: u32,
    pub font: MessageStyle,
    pub theme: String,
    pub receive_directory: String,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            allow_strangers: false,
            read_receipts: true,
            typing_indicators: true,
            notification_previews: false,
            music_status: true,
            show_timestamps: true,
            group_messages: true,
            emoticons: true,
            nudges: true,
            sounds: true,
            online_alerts: true,
            message_alerts: true,
            keep_history: true,
            auto_away_minutes: 10,
            font: MessageStyle::default(),
            theme: "azul".into(),
            receive_directory: "downloads".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bootstrap {
    pub me: Profile,
    pub contacts: Vec<Contact>,
    pub categories: Vec<Category>,
    pub conversations: Vec<Conversation>,
    pub settings: UserSettings,
    pub sessions: Vec<SessionInfo>,
    pub assets: Vec<UserAsset>,
    pub activities: Vec<Activity>,
}
