use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Presentation metadata in the contract's opaque `Profile.scene` string.
/// avatar_id identifies an asset; these references identify downloadable files.
#[derive(Default, Serialize, Deserialize)]
pub struct Scene {
    pub avatar_file: Option<Uuid>,
    pub background_file: Option<Uuid>,
}

impl Scene {
    pub fn parse(value: &str) -> Self {
        serde_json::from_str(value).unwrap_or_else(|_| Self {
            background_file: value.strip_prefix("file:").and_then(|id| id.parse().ok()),
            ..Default::default()
        })
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
