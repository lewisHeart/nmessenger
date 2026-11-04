use nm_protocol::{Announcement, AnnouncementFeed, SaveAnnouncementRequest};
use std::time::Instant;
use uuid::Uuid;

#[derive(Default)]
pub struct Announcements {
    pub feed: AnnouncementFeed,
    pub open: bool,
    pub loading: bool,
    pub busy: bool,
    pub last_refresh: Option<Instant>,
    pub selected: Option<Uuid>,
    pub draft: SaveAnnouncementRequest,
    pub link: String,
    pub error: Option<String>,
    pub notice: String,
    pub rotation: Option<Instant>,
    pub current_index: usize,
    pub viewing: Option<Announcement>,
}

impl Announcements {
    pub fn edit(&mut self, item: Option<&Announcement>) {
        self.selected = item.map(|item| item.id);
        self.draft = item
            .map(|item| SaveAnnouncementRequest {
                title: item.title.clone(),
                body: item.body.clone(),
                link_url: item.link_url.clone(),
                image_file_id: item.image_file_id,
                full_banner: item.full_banner,
                enabled: item.enabled,
            })
            .unwrap_or_default();
        self.link = self.draft.link_url.clone().unwrap_or_default();
        self.error = None;
        self.notice.clear();
    }
}
