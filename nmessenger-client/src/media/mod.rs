pub mod audio;
pub mod calls;
pub mod devices;
pub mod images;
pub mod notifications;
pub mod now_playing;
pub mod playback;
pub mod remote;
pub mod scene;
pub mod video;

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct MediaConfig {
    pub input: String,
    pub output: String,
    pub camera: String,
    pub muted: bool,
    pub camera_enabled: bool,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self { input: String::new(), output: String::new(), camera: String::new(), muted: false, camera_enabled: true }
    }
}

pub fn next_sequence() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let previous = SEQUENCE.fetch_max(clock, Ordering::Relaxed);
    SEQUENCE.fetch_add(1, Ordering::Relaxed).max(previous)
}

fn test_media() -> bool {
    cfg!(debug_assertions) && std::env::var("NM_TEST_MEDIA").as_deref() == Ok("1")
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Ink {
    pub version: u8,
    pub width: f32,
    pub height: f32,
    pub strokes: Vec<Stroke>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub color: [u8; 3],
    pub width: f32,
    pub points: Vec<[f32; 2]>,
}
