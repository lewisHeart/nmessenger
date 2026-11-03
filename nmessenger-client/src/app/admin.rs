use nm_protocol::*;
use uuid::Uuid;

#[derive(Clone)]
pub enum Mutation {
    UserStatus { user: Uuid, email: String, disabled: bool },
    UserRole { user: Uuid, email: String, administrator: bool },
    RevokeUser { user: Uuid, email: String },
    ResetPassword { user: Uuid, email: String },
    ModerateUser { user: Uuid, email: String, banned: Option<bool> },
    RevokeSession { session: Uuid, email: String },
    DeleteBadge { id: Uuid, name: String },
}

#[derive(Default)]
pub struct Admin {
    pub open: bool,
    pub tab: usize,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: String,
    pub search: String,
    pub offset: i64,
    pub overview: Option<AdminOverview>,
    pub overview_refreshed: Option<std::time::Instant>,
    pub users: Option<AdminPage<AdminUser>>,
    pub sessions: Option<AdminPage<AdminSession>>,
    pub audit: Option<AdminPage<AdminAuditEntry>>,
    pub badges: Option<Vec<AdminBadge>>,
    pub editing_badge: Option<AdminBadge>,
    pub creating_badge: bool,
    pub badge_slug: String,
    pub badge_name: String,
    pub badge_description: String,
    pub badge_icon: String,
    pub badge_points: i32,
    pub managing_user_badges: Option<AdminUser>,
    pub pending: Option<Mutation>,
    pub password: String,
    pub new_password: String,
}

impl Drop for Admin {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.password.zeroize();
        self.new_password.zeroize();
    }
}
