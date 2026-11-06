mod announcements;
pub mod admin;
mod state;
mod updates;
pub use state::*;

use crate::{
    media::images::{ImageCache, ImageSource, LocalAsset},
    transport::{Action, Transport},
};
use eframe::egui::{self, TextureHandle};
use nm_protocol::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroize;

#[cfg(any(not(target_os = "android"), test))]
pub const TOAST_SECONDS: f32 = 9.0;

pub struct MessengerApp {
    pub shared: Option<std::sync::Arc<std::sync::Mutex<MessengerApp>>>,
    pub transport: Transport,
    pub generation: u64,
    pub login: Login,
    pub data: Option<Bootstrap>,
    pub connection: String,
    pub errors: VecDeque<String>,
    pub error_at: Instant,
    pub notice: String,
    pub available_release: Option<ReleaseInfo>,
    pub release_downloading: bool,
    pub search: String,
    pub contact_filter: ContactFilter,
    pub collapsed_sections: HashSet<String>,
    pub profile_edit: Option<ProfileField>,
    pub profile_draft: String,
    pub contact_forms: ContactForms,
    pub conversation_forms: ConversationForms,
    pub options: Options,
    pub announcements: announcements::Announcements,
    pub admin: admin::Admin,
    #[cfg(any(not(target_os = "android"), test))]
    pub toasts: Vec<Toast>,
    pub root_focused: bool,
    pub quitting: bool,
    pub conversation_focus: HashSet<Uuid>,
    #[cfg(target_os = "linux")]
    pub tray: Option<std::sync::mpsc::Receiver<crate::tray::Action>>,
    pub tray_presence: Presence,
    pub tabs: Vec<Uuid>,
    pub detached: HashSet<Uuid>,
    pub active: Option<Uuid>,
    pub histories: HashMap<Uuid, History>,
    pub composers: HashMap<Uuid, Composer>,
    pub pending: HashMap<Uuid, PendingMessage>,
    pub typing: Typing,
    pub reads: HashMap<(Uuid, Uuid), Option<Uuid>>,
    pub marked_read: HashMap<Uuid, Uuid>,
    pub read_retry: HashMap<Uuid, Instant>,
    pub images: ImageCache,
    pub local_assets: Vec<LocalAsset>,
    pub file_info: HashMap<Uuid, FileInfo>,
    pub image_viewer: Option<Uuid>,
    pub forward_attachment: Option<(Uuid, MessageKind)>,
    pub forward_search: String,
    pub editing_message: Option<Message>,
    pub edit_message_draft: String,
    pub edit_message_busy: bool,
    pub edit_message_error: Option<String>,
    pub nudges: HashMap<Uuid, Instant>,
    pub winks: HashMap<Uuid, (String, Instant)>,
    pub hover: HashMap<Uuid, Instant>,
    pub now_playing: Option<crate::media::now_playing::Watcher>,
    pub auto_music: Option<String>,
    pub published_music: Option<String>,
    pub music_pending: Option<(String, Instant)>,
    pub music_track_changed: bool,
    pub games: HashMap<Uuid, Game>,
    pub authorized_calls: HashSet<Uuid>,
    pub offered_calls: HashMap<Uuid, ActivityKind>,
    pub current_call: Option<Uuid>,
    pub mobile_call_view: Option<Uuid>,
    pub media_status: Option<crate::media::calls::CallStatus>,
    pub live_frames: HashMap<(Uuid, Uuid), TextureHandle>,
    pub camera_disabled: HashSet<(Uuid, Uuid)>,
    pub screen_frames: HashMap<(Uuid, Uuid), TextureHandle>,
    pub screen_disabled: HashSet<(Uuid, Uuid)>,
    pub remote_controls: HashSet<Uuid>,
    pub remote_text: HashMap<Uuid, String>,
    pub recording: bool,
    pub recording_epoch: u64,
    pub recording_conversation: Option<Uuid>,
    pub recording_started_at: Option<Instant>,
    pub last_input: Instant,
    pub auto_away: bool,
    pub previous_presence: Presence,
    pub last_online_alerts: HashMap<Uuid, Instant>,
    pub about_open: bool,
}

impl MessengerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::ui::theme::install(&cc.egui_ctx);
        cc.egui_ctx.options_mut(|options| {
            options.input_options.surrender_focus_on = egui::SurrenderFocusOn::Never;
        });
        #[cfg(target_os = "android")]
        cc.egui_ctx.set_embed_viewports(true);
        Self {
            shared: None,
            transport: Transport::new(cc.egui_ctx.clone()),
            generation: 0,
            login: Login::default(),
            data: None,
            connection: "Desconectado".into(),
            errors: VecDeque::new(),
            error_at: Instant::now(),
            notice: String::new(),
            available_release: None,
            release_downloading: false,
            search: String::new(),
            contact_filter: ContactFilter::All,
            collapsed_sections: HashSet::new(),
            profile_edit: None,
            profile_draft: String::new(),
            contact_forms: ContactForms::default(),
            conversation_forms: ConversationForms::default(),
            options: Options::default(),
            announcements: announcements::Announcements::default(),
            admin: admin::Admin::default(),
            #[cfg(any(not(target_os = "android"), test))]
            toasts: Vec::new(),
            root_focused: true,
            quitting: false,
            conversation_focus: HashSet::new(),
            #[cfg(target_os = "linux")]
            tray: crate::tray::start(),
            tray_presence: Presence::Available,
            tabs: Vec::new(),
            detached: HashSet::new(),
            active: None,
            histories: HashMap::new(),
            composers: HashMap::new(),
            pending: HashMap::new(),
            typing: HashMap::new(),
            reads: HashMap::new(),
            marked_read: HashMap::new(),
            read_retry: HashMap::new(),
            images: ImageCache::default(),
            local_assets: Vec::new(),
            file_info: HashMap::new(),
            image_viewer: None,
            forward_attachment: None,
            forward_search: String::new(),
            editing_message: None,
            edit_message_draft: String::new(),
            edit_message_busy: false,
            edit_message_error: None,
            nudges: HashMap::new(),
            winks: HashMap::new(),
            hover: HashMap::new(),
            now_playing: crate::media::now_playing::watch(),
            auto_music: None,
            published_music: None,
            music_pending: None,
            music_track_changed: false,
            games: HashMap::new(),
            authorized_calls: HashSet::new(),
            offered_calls: HashMap::new(),
            current_call: None,
            mobile_call_view: None,
            media_status: None,
            live_frames: HashMap::new(),
            camera_disabled: HashSet::new(),
            screen_frames: HashMap::new(),
            screen_disabled: HashSet::new(),
            remote_controls: HashSet::new(),
            remote_text: HashMap::new(),
            recording: false,
            recording_epoch: 0,
            recording_conversation: None,
            recording_started_at: None,
            last_input: Instant::now(),
            auto_away: false,
            previous_presence: Presence::Available,
            last_online_alerts: HashMap::new(),
            about_open: false,
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        let ctx = eframe::egui::Context::default();
        Self {
            shared: None,
            transport: Transport::new(ctx),
            generation: 0,
            login: Login::default(),
            data: None,
            connection: "Desconectado".into(),
            errors: VecDeque::new(),
            error_at: Instant::now(),
            notice: String::new(),
            available_release: None,
            release_downloading: false,
            search: String::new(),
            contact_filter: ContactFilter::All,
            collapsed_sections: HashSet::new(),
            profile_edit: None,
            profile_draft: String::new(),
            contact_forms: ContactForms::default(),
            conversation_forms: ConversationForms::default(),
            options: Options::default(),
            announcements: announcements::Announcements::default(),
            admin: admin::Admin::default(),
            #[cfg(any(not(target_os = "android"), test))]
            toasts: Vec::new(),
            root_focused: true,
            quitting: false,
            conversation_focus: HashSet::new(),
            #[cfg(target_os = "linux")]
            tray: crate::tray::start(),
            tray_presence: Presence::Available,
            tabs: Vec::new(),
            detached: HashSet::new(),
            active: None,
            histories: HashMap::new(),
            composers: HashMap::new(),
            pending: HashMap::new(),
            typing: HashMap::new(),
            reads: HashMap::new(),
            marked_read: HashMap::new(),
            read_retry: HashMap::new(),
            images: ImageCache::default(),
            local_assets: Vec::new(),
            file_info: HashMap::new(),
            image_viewer: None,
            forward_attachment: None,
            forward_search: String::new(),
            editing_message: None,
            edit_message_draft: String::new(),
            edit_message_busy: false,
            edit_message_error: None,
            nudges: HashMap::new(),
            winks: HashMap::new(),
            hover: HashMap::new(),
            now_playing: crate::media::now_playing::watch(),
            auto_music: None,
            published_music: None,
            music_pending: None,
            music_track_changed: false,
            games: HashMap::new(),
            authorized_calls: HashSet::new(),
            offered_calls: HashMap::new(),
            current_call: None,
            mobile_call_view: None,
            media_status: None,
            live_frames: HashMap::new(),
            camera_disabled: HashSet::new(),
            screen_frames: HashMap::new(),
            screen_disabled: HashSet::new(),
            remote_controls: HashSet::new(),
            remote_text: HashMap::new(),
            recording: false,
            recording_epoch: 0,
            recording_conversation: None,
            recording_started_at: None,
            last_input: Instant::now(),
            auto_away: false,
            previous_presence: Presence::Available,
            last_online_alerts: HashMap::new(),
            about_open: false,
        }
    }

    pub fn send(&self, action: Action) {
        self.transport.send(self.generation, action);
    }

    pub fn set_camera_enabled(&mut self, enabled: bool) {
        let Some(activity_id) = self.current_call else { return; };
        if let Some(status) = &mut self.media_status {
            status.camera_enabled = enabled;
            status.ready.remove(&crate::media::calls::Component::Camera);
            status.errors.remove(&crate::media::calls::Component::Camera);
        }
        if let Some(me) = self.data.as_ref().map(|data| data.me.id) {
            if enabled { self.camera_disabled.remove(&(activity_id, me)); }
            else {
                self.camera_disabled.insert((activity_id, me));
                self.live_frames.remove(&(activity_id, me));
            }
        }
        self.send(Action::SetCameraEnabled(enabled));
        self.send(Action::Realtime(ClientEvent::Signal {
            activity_id, data: serde_json::json!({"camera_enabled": enabled}),
        }));
    }

    pub fn set_screen_sharing(&mut self, enabled: bool) {
        let Some(activity_id) = self.current_call else { return; };
        if let Some(status) = &mut self.media_status {
            status.screen_sharing = enabled;
            status.errors.remove(&crate::media::calls::Component::Screen);
        }
        if let Some(me) = self.data.as_ref().map(|data| data.me.id) {
            if enabled { self.screen_disabled.remove(&(activity_id, me)); }
            else { self.screen_disabled.insert((activity_id, me)); self.screen_frames.remove(&(activity_id, me)); }
        }
        self.send(Action::SetScreenSharing(enabled));
    }

    pub fn call_busy(&self, except: Option<Uuid>) -> bool {
        self.current_call.is_some_and(|id| Some(id) != except)
            || !self.offered_calls.is_empty()
            || self.data.as_ref().is_some_and(|data| data.activities.iter().any(|activity| {
                Some(activity.id) != except
                    && matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall | ActivityKind::RemoteAssistance)
                    && (activity.status == ActivityStatus::Accepted
                        || (activity.status == ActivityStatus::Offered && activity.initiator_id == data.me.id))
            }))
    }

    pub fn humanize_error(raw: &str, is_logged_in: bool) -> String {
        let text = raw.trim();
        if text.contains("unauthorized") || text.contains("401") {
            if is_logged_in {
                return crate::i18n::tr("Your session has expired. Please sign in again.", "Sua sessão expirou. Conecte-se novamente.").into();
            } else {
                return crate::i18n::tr("Incorrect email or password. Check your credentials.", "E-mail ou senha incorretos. Verifique suas credenciais.").into();
            }
        }
        if text.contains("forbidden") || text.contains("403") {
            return crate::i18n::tr("You do not have permission for this operation.", "Acesso não permitido para esta operação.").into();
        }
        if text.contains("404") || text.contains("not_found") {
            return crate::i18n::tr("The requested item was not found on the server.", "Item ou recurso não encontrado no servidor.").into();
        }
        if text.contains("429") || text.contains("too_many") {
            return crate::i18n::tr("Too many attempts. Please try again shortly.", "Muitas tentativas em pouco tempo. Aguarde alguns instantes.").into();
        }
        if text.contains("413") || text.contains("payload_too_large") {
            return crate::i18n::tr("The file exceeds the allowed size.", "O arquivo ultrapassa o tamanho permitido.").into();
        }
        if text.contains("502") || text.contains("503") || text.contains("504") {
            return crate::i18n::tr("The server is temporarily unavailable. Please try again shortly.", "Servidor temporariamente indisponível. Tente novamente em instantes.").into();
        }
        if text.contains("error sending request")
            || text.contains("connection refused")
            || text.contains("Connection refused")
            || text.contains("Conexão recusada")
            || text.contains("tcp connect error")
            || text.contains("timed out")
            || text.contains("dns error")
        {
            return crate::i18n::tr(
                "Unable to connect to server. Check address, port and network.",
                "Não foi possível conectar ao servidor. Verifique o endereço, porta e rede.",
            ).into();
        }
        if let Some((_, rest)) = text.split_once(" · ") {
            let msg = if let Some((clean, _)) = rest.split_once(" (HTTP") {
                clean.trim()
            } else {
                rest.trim()
            };
            if !msg.is_empty() {
                if msg.contains("inválida") || msg.contains("expirada") {
                    if is_logged_in {
                        return crate::i18n::tr("Your session has expired. Please sign in again.", "Sua sessão expirou. Conecte-se novamente.").into();
                    } else {
                        return crate::i18n::tr("Incorrect email or password. Check your credentials.", "E-mail ou senha incorretos. Verifique suas credenciais.").into();
                    }
                }
                return msg.to_string();
            }
        }
        if let Some((clean, _)) = text.split_once(" (HTTP") {
            return clean.trim().to_string();
        }
        text.to_string()
    }

    pub fn error(&mut self, error: impl Into<String>) {
        let raw = error.into();
        let error = Self::humanize_error(&raw, self.data.is_some());
        if self.errors.back() == Some(&error) {
            self.error_at = Instant::now();
            return;
        }
        if self.errors.len() >= 8 {
            self.errors.pop_front();
        }
        self.errors.push_back(error);
        self.error_at = Instant::now();
    }

    pub fn sign_in(&mut self) {
        if self.login.busy {
            return;
        }
        if self.login.prefs.email.trim().is_empty()
            || self.login.password.is_empty()
            || self.login.prefs.device.trim().is_empty()
            || (self.login.register && self.login.name.trim().is_empty())
        {
            self.error(crate::i18n::tr("Enter your email, password and device name; new accounts also need a display name.", "Preencha e-mail, senha, dispositivo e, ao cadastrar, seu nome."));
            return;
        }
        self.generation += 1;
        self.login.busy = true;
        self.errors.clear();
        if !self.login.prefs.custom_server || self.login.prefs.server.trim().is_empty() {
            self.login.prefs.server = crate::env::default_server_url();
        } else {
            self.login.prefs.server = crate::env::format_server_url(&self.login.prefs.server);
        }
        self.send(Action::SavePreferences(self.login.prefs.clone()));
        let password = std::mem::take(&mut self.login.password);
        if self.login.prefs.auto_login {
            let server = self.login.prefs.server.clone();
            let email = self.login.prefs.email.trim().to_string();
            let pass = password.clone();
            self.send(Action::SaveSavedSession {
                server,
                email,
                password: pass,
            });
        } else {
            self.send(Action::ClearSavedSession);
        }
        let identity_password = (!self.login.identity_password.is_empty())
            .then(|| std::mem::take(&mut self.login.identity_password));
        self.send(Action::Login {
            server: self.login.prefs.server.clone(),
            email: self.login.prefs.email.trim().into(),
            password,
            identity_password,
            device: self.login.prefs.device.trim().into(),
            name: self.login.register.then(|| self.login.name.trim().into()),
            presence: self.login.prefs.presence,
        });
    }

    pub fn logout(&mut self) {
        self.generation += 1;
        self.login.prefs.auto_login = false;
        let prefs = self.login.prefs.clone();
        self.send(Action::SavePreferences(prefs));
        self.send(Action::ClearSavedSession);
        self.send(Action::Logout);
        self.clear_session();
    }

    pub fn clear_session(&mut self) {
        self.data = None;
        self.login.busy = false;
        self.login.password.zeroize();
        self.login.identity_password.zeroize();
        self.login.identity_recovery = false;
        self.tabs.clear();
        self.detached.clear();
        self.active = None;
        self.histories.clear();
        self.composers.clear();
        self.pending.clear();
        self.typing.clear();
        self.reads.clear();
        self.marked_read.clear();
        self.read_retry.clear();
        self.images = ImageCache::default();
        self.file_info.clear();
        self.image_viewer = None;
        self.forward_attachment = None;
        self.forward_search.clear();
        self.editing_message = None;
        self.edit_message_draft.clear();
        self.edit_message_busy = false;
        self.edit_message_error = None;
        self.games.clear();
        self.authorized_calls.clear();
        self.offered_calls.clear();
        self.current_call = None;
        self.mobile_call_view = None;
        self.camera_disabled.clear();
        self.screen_frames.clear();
        self.screen_disabled.clear();
        self.media_status = None;
        self.live_frames.clear();
        self.remote_controls.clear();
        self.remote_text.clear();
        self.recording = false;
        self.recording_conversation = None;
        self.recording_started_at = None;
        self.nudges.clear();
        self.winks.clear();
        #[cfg(any(not(target_os = "android"), test))]
        self.toasts.clear();
        self.hover.clear();
        self.published_music = None;
        self.music_pending = None;
        self.music_track_changed = self.auto_music.is_some();
        self.conversation_focus.clear();
        self.auto_away = false;
        self.options = Options::default();
        self.announcements = announcements::Announcements::default();
        self.admin = admin::Admin::default();
        self.contact_forms = ContactForms::default();
        self.profile_edit = None;
        self.profile_draft.clear();
        self.collapsed_sections.clear();
        self.conversation_forms = ConversationForms::default();
        self.connection = "Desconectado".into();
    }

    pub fn section_size(&self, section_key: &str) -> ContactDisplaySize {
        let size_str = self
            .login
            .prefs
            .section_sizes
            .get(section_key)
            .map(|s| s.as_str());
        let size = match size_str {
            Some("small") => ContactDisplaySize::Small,
            Some("medium") => ContactDisplaySize::Medium,
            Some("large") => ContactDisplaySize::Large,
            _ => match section_key {
                "favorites" => ContactDisplaySize::Large,
                _ => ContactDisplaySize::Small,
            },
        };
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
    }

    pub fn set_section_size(&mut self, section_key: &str, size: ContactDisplaySize) {
        let constrained = match section_key {
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
        };
        let s = match constrained {
            ContactDisplaySize::Small => "small",
            ContactDisplaySize::Medium => "medium",
            ContactDisplaySize::Large => "large",
        };
        self.login
            .prefs
            .section_sizes
            .insert(section_key.into(), s.into());
        self.send(Action::SavePreferences(self.login.prefs.clone()));
    }

    pub fn open_conversation(&mut self, id: Uuid) {
        if crate::platform::is_mobile() {
            self.detached.remove(&id);
        } else {
            self.detached.insert(id);
        }
        if self.active != Some(id)
            && let Some(old) = self.active
        {
            self.send(Action::Realtime(ClientEvent::Typing {
                conversation_id: old,
                is_typing: false,
            }));
        }
        self.active = Some(id);
        if !self.tabs.contains(&id) {
            self.tabs.push(id);
        }
        if !self.composers.contains_key(&id) {
            let mut composer = Composer::default();
            if let Some(data) = &self.data {
                composer.style = data.settings.font.clone();
            }
            self.composers.insert(id, composer);
        }
        let history = self.histories.entry(id).or_default();
        if !history.loaded && !history.loading {
            self.load_history(id, None, String::new());
        }
        self.watch();
    }

    pub fn watch(&self) {
        let watched = self
            .tabs
            .iter()
            .map(|id| {
                (
                    *id,
                    self.histories
                        .get(id)
                        .map(|h| crate::transport::Watch {
                            latest: h.messages.last().map(|m| m.id),
                            revision: h.revision,
                        })
                        .unwrap_or_default(),
                )
            })
            .collect();
        self.send(Action::Watch(watched));
    }

    fn visible_conversation_for_read(&self, focused: bool, mobile: bool) -> Option<Uuid> {
        if !focused { return None; }
        self.active.filter(|id| mobile || !self.detached.contains(id))
    }

    pub fn load_history(&mut self, id: Uuid, before: Option<Uuid>, query: String) {
        let history = self.histories.entry(id).or_default();
        if before.is_none() && history.query != query {
            history.messages.clear();
        }
        history.query = query.clone();
        history.loading = true;
        history.error = None;
        history.revision += 1;
        let revision = history.revision;
        self.send(Action::History {
            id,
            before,
            query,
            revision,
        });
        self.watch();
    }

    pub fn send_message(
        &mut self,
        id: Uuid,
        kind: MessageKind,
        body: String,
        attachment_id: Option<Uuid>,
    ) {
        if body.chars().count() > MAX_MESSAGE_CHARS {
            self.error("Mensagem excede 16.000 caracteres.");
            return;
        }
        let style = self
            .composers
            .get(&id)
            .map(|c| c.style.clone())
            .unwrap_or_default();
        let request = SendMessageRequest {
            client_id: Uuid::new_v4(),
            kind,
            body,
            style,
            attachment_id,
        };
        self.queue_message(id, request);
    }

    pub fn nudge(&mut self, id: Uuid, local: bool) {
        self.nudges.insert(id, Instant::now());
        if local
            && let Some(data) = &self.data
            && data.settings.sounds
        {
            let peer = data
                .conversations
                .iter()
                .find(|conversation| conversation.id == id)
                .and_then(|conversation| {
                    conversation
                        .members
                        .iter()
                        .find(|member| member.id != data.me.id)
                })
                .map_or(data.me.id, |member| member.id);
            self.play_sound("nudge", peer);
        }
        #[cfg(target_os = "android")]
        crate::android::attention();
        if !self.detached.contains(&id) {
            self.open_conversation(id);
        }
        let viewport = egui::ViewportId::from_hash_of(("nm-conversation", id));
        self.transport.repaint_viewport(viewport);
    }

    pub fn queue_message(&mut self, id: Uuid, request: SendMessageRequest) {
        self.pending.insert(
            request.client_id,
            PendingMessage {
                conversation: id,
                request: request.clone(),
                error: None,
            },
        );
        self.send(Action::Send(id, request));
        self.send(Action::Realtime(ClientEvent::Typing {
            conversation_id: id,
            is_typing: false,
        }));
    }

    pub fn conversation_title(&self, conversation: &Conversation) -> String {
        if let Some(title) = conversation.title.as_ref().filter(|s| !s.is_empty()) {
            return title.clone();
        }
        let me = self.data.as_ref().map(|d| d.me.id);
        conversation
            .members
            .iter()
            .filter(|m| Some(m.id) != me)
            .map(|m| m.display_name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[cfg(any(not(target_os = "android"), test))]
    pub fn conversation_window_title(&self, conversation: &Conversation) -> String {
        let me = self.data.as_ref().map(|d| d.me.id);
        if conversation.kind == ConversationKind::Direct {
            if let Some(other) = conversation.members.iter().find(|m| Some(m.id) != me) {
                let name = other.display_name.trim();
                let email = other.email.trim();
                if !name.is_empty() && !email.is_empty() && name != email {
                    return format!("{name} ({email}) - Conversa");
                } else if !name.is_empty() {
                    return format!("{name} - Conversa");
                } else if !email.is_empty() {
                    return format!("{email} - Conversa");
                }
            }
        }
        format!("{} - Conversa", self.conversation_title(conversation))
    }

    pub fn name(&self, id: Uuid) -> String {
        if let Some(data) = &self.data {
            if data.me.id == id {
                return data.me.display_name.clone();
            }
            if let Some(c) = data.contacts.iter().find(|c| c.user.id == id) {
                return c.name().into();
            }
            if let Some(m) = data
                .conversations
                .iter()
                .flat_map(|c| &c.members)
                .find(|p| p.id == id)
            {
                return m.display_name.clone();
            }
        }
        "Participante".into()
    }

    pub fn profile_of(&self, id: Uuid) -> Option<Profile> {
        let data = self.data.as_ref()?;
        if data.me.id == id {
            return Some(data.me.clone());
        }
        data.contacts
            .iter()
            .map(|contact| &contact.user)
            .chain(data.conversations.iter().flat_map(|c| c.members.iter()))
            .find(|profile| profile.id == id)
            .cloned()
    }

    pub fn direct_with(&self, user: Uuid) -> Option<Uuid> {
        self.data
            .as_ref()?
            .conversations
            .iter()
            .find(|conversation| {
                conversation.kind == ConversationKind::Direct
                    && conversation.members.iter().any(|member| member.id == user)
            })
            .map(|conversation| conversation.id)
    }

    pub fn push_toast(&mut self, user: Uuid, text: String, conversation: Option<Uuid>) {
        let profile = self.profile_of(user).unwrap_or_else(|| Profile {
            id: user, email: String::new(), display_name: "Nova mensagem".into(),
            personal_message: String::new(), presence: Presence::Offline, avatar_id: None,
            scene: String::new(), now_playing: None, created_at: chrono::Utc::now(),
            bio: String::new(), badges: Vec::new(),
        });
        if self.data.as_ref().is_some_and(|data| data.me.id == user) {
            return;
        }
        #[cfg(target_os = "android")]
        let _ = crate::android::notify_message(&profile.display_name, &text, conversation);
        #[cfg(any(not(target_os = "android"), test))]
        {
            self.toasts
                .retain(|toast| !(toast.profile.id == user || (conversation.is_some() && toast.conversation == conversation)));
            self.toasts.push(Toast {
                id: Uuid::new_v4(),
                profile,
                text,
                conversation,
                at: Instant::now(),
            });
            while self.toasts.len() > 3 {
                self.toasts.remove(0);
            }
        }
    }

    #[cfg(any(not(target_os = "android"), test))]
    pub fn dismiss_toast(&mut self, id: Uuid) {
        self.toasts.retain(|toast| toast.id != id);
    }

    pub fn notify(&self, event: &str, user: Uuid) {
        let Some(data) = &self.data else {
            return;
        };
        if !data.settings.sounds || data.me.presence == Presence::Busy || data.me.id == user {
            return;
        }
        self.play_sound(event, user);
    }

    fn play_sound(&self, event: &str, user: Uuid) {
        let Some(data) = &self.data else {
            return;
        };
        let contact_name = format!("contact:{user}");
        let event_name = format!("event:{event}");
        let sounds: Vec<_> = data
            .assets
            .iter()
            .rev()
            .filter(|a| a.kind == AssetKind::Sound)
            .collect();
        let file = sounds
            .iter()
            .find(|a| a.name == contact_name)
            .or_else(|| sounds.iter().find(|a| a.name == event_name))
            .map(|a| a.file_id);
        self.send(Action::Notification {
            file,
            event: event.into(),
            output: self.login.prefs.audio_output.clone(),
        });
    }

    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        source: ImageSource,
    ) -> Option<(egui::TextureId, egui::Vec2)> {
        if let Some(texture) = self.images.texture(ctx, &source) {
            return Some(texture);
        }
        if !self.images.failed.contains(&source) && self.images.pending.insert(source.clone()) {
            self.send(Action::LoadImage(source));
        }
        None
    }

    pub fn open_options(&mut self) {
        if let Some(data) = &self.data {
            self.options.settings = data.settings.clone();
            self.options.display_name = data.me.display_name.clone();
            self.options.personal_message = data.me.personal_message.clone();
            self.options.music = if self.published_music.is_some()
                && self.published_music.as_deref() == data.me.now_playing.as_deref()
            {
                String::new()
            } else {
                data.me.now_playing.clone().unwrap_or_default()
            };
            self.options.scene = data.me.scene.clone();
        }
        self.options.open = true;
        self.options.local = self.login.prefs.clone();
        self.options.loading = true;
        self.options.saving = false;
        self.options.error = None;
        self.send(Action::Settings(None));
        self.send(Action::Sessions);
        self.send(Action::DiscoverDevices);
    }

    pub fn media_config(&self) -> crate::media::MediaConfig {
        crate::media::MediaConfig {
            input: self.login.prefs.audio_input.clone(),
            output: self.login.prefs.audio_output.clone(),
            camera: self.login.prefs.camera_device.clone(),
            camera_enabled: self.media_status.as_ref().is_none_or(|status| status.camera_enabled),
            muted: self
                .media_status
                .as_ref()
                .is_some_and(|status| status.muted),
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        if self.data.is_some()
            && !self.announcements.loading
            && self
                .announcements
                .last_refresh
                .is_none_or(|time| time.elapsed() >= Duration::from_secs(60))
        {
            self.announcements.loading = true;
            self.send(Action::Announcements);
        }
        if self.recording {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.recording = false;
                self.recording_conversation = None;
                self.recording_started_at = None;
                self.send(Action::StopVoice { cancel: true });
            } else if ctx.input(|i| i.key_released(egui::Key::F2)) {
                self.recording = false;
                self.recording_conversation = None;
                self.recording_started_at = None;
                self.send(Action::StopVoice { cancel: false });
            }
        }
        self.typing
            .retain(|_, time| time.elapsed() < Duration::from_secs(6));
        self.nudges
            .retain(|_, time| time.elapsed() < Duration::from_millis(850));
        self.winks
            .retain(|_, (_, time)| time.elapsed() < Duration::from_secs(4));
        #[cfg(any(not(target_os = "android"), test))]
        self.toasts
            .retain(|toast| toast.at.elapsed().as_secs_f32() < TOAST_SECONDS);
        self.publish_now_playing();
        self.root_focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        #[cfg(target_os = "linux")]
        while let Some(action) = self.tray.as_ref().and_then(|tray| tray.try_recv().ok()) {
            match action {
                crate::tray::Action::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.request_repaint();
                }
                crate::tray::Action::Hide => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
                crate::tray::Action::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                crate::tray::Action::Presence(presence) => {
                    if self.data.is_some() {
                        self.auto_away = false;
                        self.send(Action::Profile(UpdateProfileRequest {
                            presence: Some(presence),
                            ..Default::default()
                        }));
                    } else {
                        self.login.prefs.presence = presence;
                    }
                }
            }
        }
        let current_presence = self
            .data
            .as_ref()
            .map(|data| data.me.presence)
            .unwrap_or(self.login.prefs.presence);
        if current_presence != self.tray_presence {
            self.tray_presence = current_presence;
            #[cfg(target_os = "linux")]
            crate::tray::presence(current_presence);
        }
        let mut stop_typing = Vec::new();
        for (id, composer) in &mut self.composers {
            if composer
                .last_edit
                .is_some_and(|t| t.elapsed() > Duration::from_secs(3))
                && composer.last_typing.is_some()
            {
                composer.last_typing = None;
                composer.last_edit = None;
                stop_typing.push(*id);
            }
        }
        for id in stop_typing {
            self.send(Action::Realtime(ClientEvent::Typing {
                conversation_id: id,
                is_typing: false,
            }));
        }
        let input = ctx.input(|i| !i.events.is_empty());
        if input {
            self.last_input = Instant::now();
        }
        if let Some(data) = &self.data {
            let minutes = data.settings.auto_away_minutes;
            if !self.auto_away
                && minutes > 0
                && data.me.presence == Presence::Available
                && self.last_input.elapsed() > Duration::from_secs(minutes as u64 * 60)
            {
                self.previous_presence = data.me.presence;
                self.auto_away = true;
                self.send(Action::Profile(UpdateProfileRequest {
                    presence: Some(Presence::Away),
                    ..Default::default()
                }));
            } else if self.auto_away
                && (input || self.last_input.elapsed() < Duration::from_millis(500))
            {
                self.auto_away = false;
                self.send(Action::Profile(UpdateProfileRequest {
                    presence: Some(self.previous_presence),
                    ..Default::default()
                }));
            }
        }
        if let Some(id) = self.visible_conversation_for_read(
            ctx.input(|input| input.viewport().focused.unwrap_or(true)), crate::platform::is_mobile())
        {
            self.mark_visible(id);
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    /// A newly detected track replaces a stale or manually entered track.
    fn publish_now_playing(&mut self) {
        let Some(watcher) = &self.now_playing else {
            return;
        };
        while let Some(track) = watcher.changed() {
            if track.is_some() && track != self.auto_music { self.music_track_changed = true; }
            self.auto_music = track;
        }
        if self
            .music_pending
            .as_ref()
            .is_some_and(|(_, sent)| sent.elapsed() < Duration::from_secs(15))
        {
            return;
        }
        self.music_pending = None;
        let Some(data) = &self.data else {
            return;
        };
        let enabled = data.settings.music_status;
        let current = data.me.now_playing.clone().unwrap_or_default();
        let desired = crate::media::now_playing::should_publish(
            enabled,
            &current,
            self.published_music.as_deref(),
            self.auto_music.as_deref(),
            self.music_track_changed,
        );
        if enabled && self.auto_music.as_deref() == Some(current.as_str()) {
            self.music_track_changed = false;
        }
        if let Some(value) = desired {
            self.music_track_changed = false;
            self.music_pending = Some((value.clone(), Instant::now()));
            self.send(Action::Profile(UpdateProfileRequest {
                now_playing: Some(value),
                ..Default::default()
            }));
        }
    }

    pub fn mark_visible(&mut self, id: Uuid) {
        if let Some(last) = self
            .histories
            .get(&id)
            .filter(|h| h.query.is_empty())
            .and_then(|h| h.messages.last())
            .map(|m| m.id)
            && self.marked_read.get(&id) != Some(&last)
            && self
                .read_retry
                .get(&id)
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(5))
        {
            self.marked_read.insert(id, last);
            self.send(Action::Read(id, Some(last)));
        }
    }
}

impl eframe::App for MessengerApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        crate::platform::poll_ime_events(ctx);
        if crate::platform::take_keyboard_back() {
            if self.profile_edit.is_some() {
                self.profile_edit = None;
                self.profile_draft.clear();
                crate::platform::hide_soft_input();
            } else if self.editing_message.is_some() {
                self.editing_message = None;
                self.edit_message_draft.clear();
                self.edit_message_busy = false;
                self.edit_message_error = None;
            }
        }
        while let Ok(packet) = self.transport.receiver.try_recv() {
            if packet.generation == self.generation {
                self.apply(ctx, packet.update);
            }
        }
        self.tick(ctx);
        if let Some(sizes) = crate::ui::window::take_size_changes(ctx) {
            self.login.prefs.window_sizes = sizes.clone();
            self.options.local.window_sizes = sizes;
            self.send(Action::SavePreferences(self.login.prefs.clone()));
        }
        #[cfg(target_os = "android")]
        if self.data.is_some() && let Some(conversation) = crate::android::take_notification_conversation()
            && self.data.as_ref().is_some_and(|data| data.conversations.iter().any(|item| item.id == conversation)) {
            self.open_conversation(conversation);
        }
        #[cfg(target_os = "android")]
        if self.data.is_some() && let Some(action) = crate::android::take_notification_action()
            && let Some(conversation) = action.get("conversation").and_then(|v| v.as_str()).and_then(|v| v.parse::<Uuid>().ok())
            && self.data.as_ref().is_some_and(|data| data.conversations.iter().any(|item| item.id == conversation)) {
            match action.get("type").and_then(|v| v.as_str()) {
                Some("read") => {
                    let message = action.get("message").and_then(|v| v.as_str()).and_then(|v| v.parse::<Uuid>().ok());
                    if let Some(message) = message { self.send(Action::Read(conversation, Some(message))); }
                }
                Some("reply") => {
                    if let Some(text) = action.get("text").and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()) {
                        self.send_message(conversation, MessageKind::Text, text.to_owned(), None);
                    }
                }
                _ => {}
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
        #[cfg(not(target_os = "android"))]
        if self.data.is_some() {
            crate::ui::toast::show(self, ctx);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::ui::shell(self, ui);
        if let Some(release) = self.available_release.clone() {
            let mut dismiss = false;
            egui::Window::new("Atualização disponível").collapsible(false).show(ui.ctx(), |ui| {
                ui.label(format!("A versão {} está disponível.", release.tag_name));
                if ui.add_enabled(!self.release_downloading, egui::Button::new("Baixar atualização verificada")).clicked() {
                    self.release_downloading = true;
                    self.send(Action::DownloadRelease(release.tag_name.clone()));
                }
                if self.release_downloading { ui.spinner(); }
                ui.hyperlink_to("Abrir release no GitHub", format!("https://github.com/lewisHeart/nmessenger/releases/tag/{}", release.tag_name));
                if ui.button("Depois").clicked() { dismiss = true; }
            });
            if dismiss { self.available_release = None; }
        }
        crate::platform::sync_ime(ui.ctx());
    }
}

impl Drop for MessengerApp {
    fn drop(&mut self) {
        self.login.password.zeroize();
        self.login.identity_password.zeroize();
        self.send(Action::StopVoice { cancel: true });
        self.send(Action::StopCall);
    }
}

#[cfg(test)]
mod tests;
