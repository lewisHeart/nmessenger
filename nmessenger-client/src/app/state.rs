use crate::{media::Ink, storage::Preferences};
use nm_protocol::*;
use std::{collections::HashMap, time::Instant};
use uuid::Uuid;

#[derive(Default)]
pub struct Login {
    pub prefs: Preferences,
    pub password: String,
    pub identity_password: String,
    pub identity_recovery: bool,
    pub name: String,
    pub register: bool,
    pub busy: bool,
    pub connection_open: bool,
    pub connection_draft: Option<Preferences>,
}

#[derive(Default)]
pub struct History {
    pub show_search: bool,
    pub show_media_gallery: bool,
    pub media_tab: usize,
    pub scroll_to_message: Option<Uuid>,
    pub revision: u64,
    pub messages: Vec<Message>,
    pub query: String,
    pub query_input: String,
    pub loading: bool,
    pub loaded: bool,
    pub more: bool,
    pub error: Option<String>,
}

pub struct Composer {
    pub text: String,
    pub style: MessageStyle,
    pub ink: Ink,
    pub drawing: bool,
    pub format_open: bool,
    pub ink_color: [u8; 3],
    pub ink_width: f32,
    pub last_typing: Option<Instant>,
    pub last_edit: Option<Instant>,
    pub nudge_sent: Option<Instant>,
    pub focus_requested: bool,
}

impl Default for Composer {
    fn default() -> Self {
        Self {
            text: String::new(),
            style: MessageStyle::default(),
            ink: Ink {
                version: 1,
                width: 600.0,
                height: 180.0,
                strokes: Vec::new(),
            },
            drawing: false,
            format_open: false,
            ink_color: [25, 80, 150],
            ink_width: 3.0,
            last_typing: None,
            last_edit: None,
            nudge_sent: None,
            focus_requested: false,
        }
    }
}

pub struct PendingMessage {
    pub conversation: Uuid,
    pub request: SendMessageRequest,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ContactFilter {
    #[default]
    All,
    Online,
    Requests,
    Blocked,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ContactDisplaySize {
    #[default]
    Small,
    Medium,
    Large,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupContactFilter {
    #[default]
    All,
    Online,
    Selected,
}

#[derive(Default)]
pub struct ContactForms {
    pub selected: Option<Uuid>,
    pub add_open: bool,
    pub email: String,
    pub nickname: String,
    pub view_profile: Option<Profile>,
    pub edit_profile_open: bool,
    pub edit_display_name: String,
    pub edit_personal_message: String,
    pub edit_bio: String,
    pub category_open: bool,
    pub category_id: Option<Uuid>,
    pub category_name: String,
    pub group_open: bool,
    pub group_title: String,
    pub group_members: Vec<Uuid>,
    pub group_search: String,
    pub group_filter: GroupContactFilter,
}

#[derive(Default)]
pub struct ConversationForms {
    pub conversation: Option<Uuid>,
    pub viewport: Option<eframe::egui::ViewportId>,
    pub members_open: bool,
    pub title: String,
    pub invite: Option<Uuid>,
    pub clear_confirm: bool,
}

#[derive(Default)]
pub struct Options {
    pub encryption_keys: Option<(Uuid, Vec<nm_protocol::EncryptionKey>)>,
    pub encryption_search: String,
    pub encryption_selected: Option<Uuid>,
    pub history_backup_password: String,
    pub history_backup_busy: bool,
    pub history_backup_status: Option<Result<bool, String>>,
    pub verification_codes: HashMap<Uuid, String>,
    pub roster_code: String,
    pub open: bool,
    pub avatar_picker: bool,
    pub avatar_choice: Option<AvatarChoice>,
    pub avatar_saving: bool,
    pub avatar_error: Option<String>,
    pub game_browser: Option<Uuid>,
    pub local: Preferences,
    pub devices: crate::media::devices::DeviceCatalog,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub tab: usize,
    pub sound_status: String,
    pub settings: UserSettings,
    pub display_name: String,
    pub personal_message: String,
    pub music: String,
    pub scene: String,
    pub asset_name: String,
    pub shortcut: String,
    pub emoticon_busy: bool,
    pub emoticon_status: Option<Result<bool, String>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileField {
    Name,
    PersonalMessage,
    Music,
}

#[derive(Clone)]
pub enum AvatarChoice {
    Preset {
        name: String,
        preview: String,
        original: String,
    },
    Existing(UserAsset),
    Local {
        path: std::path::PathBuf,
        name: String,
    },
}

#[derive(Default, Clone)]
pub struct Game {
    pub board: [Option<Uuid>; 9],
}

impl Game {
    pub fn winner(&self) -> Option<Uuid> {
        for [a, b, c] in [
            [0, 1, 2],
            [3, 4, 5],
            [6, 7, 8],
            [0, 3, 6],
            [1, 4, 7],
            [2, 5, 8],
            [0, 4, 8],
            [2, 4, 6],
        ] {
            if self.board[a].is_some()
                && self.board[a] == self.board[b]
                && self.board[b] == self.board[c]
            {
                return self.board[a];
            }
        }
        None
    }

    pub fn turn(&self, initiator: Uuid, other: Uuid) -> Uuid {
        if self.board.iter().filter(|c| c.is_some()).count() % 2 == 0 {
            initiator
        } else {
            other
        }
    }

    pub fn apply(&mut self, from: Uuid, cell: usize, initiator: Uuid, other: Uuid) -> bool {
        if cell >= 9
            || self.board[cell].is_some()
            || self.winner().is_some()
            || self.turn(initiator, other) != from
        {
            return false;
        }
        self.board[cell] = Some(from);
        true
    }
}

pub type Typing = HashMap<(Uuid, Uuid), Instant>;

/// A transient desktop notification, shown near the system tray.
#[cfg(any(not(target_os = "android"), test))]
#[derive(Clone)]
pub struct Toast {
    pub id: Uuid,
    pub profile: Profile,
    pub text: String,
    pub conversation: Option<Uuid>,
    pub at: Instant,
}

/// Toasts appear only when the user is not already reading that conversation.
pub fn should_notify(
    active: Option<Uuid>,
    conversation: Uuid,
    root_focused: bool,
    conversation_focused: bool,
) -> bool {
    !(active == Some(conversation) && (root_focused || conversation_focused))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toasts_are_skipped_only_while_the_focused_conversation_is_being_read() {
        let id = Uuid::from_u128(7);
        let other = Uuid::from_u128(8);
        assert!(!should_notify(Some(id), id, true, false));
        assert!(!should_notify(Some(id), id, false, true));
        assert!(should_notify(Some(id), id, false, false));
        assert!(should_notify(Some(other), id, true, true));
        assert!(should_notify(None, id, true, true));
    }

    #[test]
    fn signals_cannot_overwrite_cells_play_out_of_turn_or_continue_after_a_win() {
        let x = Uuid::from_u128(1);
        let o = Uuid::from_u128(2);
        let mut game = Game::default();
        assert!(!game.apply(o, 0, x, o));
        assert!(!game.apply(x, 9, x, o));
        for (user, cell) in [(x, 0), (o, 3), (x, 1), (o, 4), (x, 2)] {
            assert!(game.apply(user, cell, x, o));
        }
        assert_eq!(game.winner(), Some(x));
        assert!(!game.apply(o, 5, x, o));
        assert!(!game.apply(o, 0, x, o));
    }

    #[test]
    fn display_sizes_enforce_favorites_and_offline_rules() {
        let constrain = |section_key: &str, size: ContactDisplaySize| -> ContactDisplaySize {
            match section_key {
                "favorites" => {
                    if size == ContactDisplaySize::Small {
                        ContactDisplaySize::Medium
                    } else {
                        size
                    }
                }
                "available" | "offline" => {
                    if size == ContactDisplaySize::Large {
                        ContactDisplaySize::Medium
                    } else {
                        size
                    }
                }
                _ => size,
            }
        };

        assert_eq!(
            constrain("favorites", ContactDisplaySize::Small),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("favorites", ContactDisplaySize::Medium),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("favorites", ContactDisplaySize::Large),
            ContactDisplaySize::Large
        );

        assert_eq!(
            constrain("available", ContactDisplaySize::Large),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("available", ContactDisplaySize::Medium),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("available", ContactDisplaySize::Small),
            ContactDisplaySize::Small
        );

        assert_eq!(
            constrain("offline", ContactDisplaySize::Large),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("offline", ContactDisplaySize::Medium),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("offline", ContactDisplaySize::Small),
            ContactDisplaySize::Small
        );

        assert_eq!(
            constrain("groups", ContactDisplaySize::Small),
            ContactDisplaySize::Small
        );
        assert_eq!(
            constrain("groups", ContactDisplaySize::Medium),
            ContactDisplaySize::Medium
        );
        assert_eq!(
            constrain("groups", ContactDisplaySize::Large),
            ContactDisplaySize::Large
        );
    }
}
