use super::*;
use crate::transport::Update;

impl MessengerApp {
    pub(super) fn apply(&mut self, ctx: &egui::Context, update: Update) {
        match update {
            Update::ReleaseDownload(result) => {
                self.release_downloading = false;
                match result {
                    Ok(Some(path)) => self.notice = format!("Atualização verificada salva em {}. Abra o arquivo para instalar.", path.display()),
                    Ok(None) => {},
                    Err(error) => self.error(error),
                }
            }
            Update::Release(release) => {
                self.available_release = Some(release);
            }
            Update::AdminOverview(value) => { self.admin.overview = Some(value); self.admin.busy = false; },
            Update::AdminUsers(value) => { self.admin.users = Some(value); self.admin.busy = false; },
            Update::AdminSessions(value) => { self.admin.sessions = Some(value); self.admin.busy = false; },
            Update::AdminAudit(value) => { self.admin.audit = Some(value); self.admin.busy = false; },
            Update::AdminBadges(value) => { self.admin.badges = Some(value); self.admin.busy = false; },
            Update::AdminFailed(error) => { self.admin.error = Some(error); self.admin.busy = false; },
            Update::AdminChanged(notice) => {
                self.admin.notice = notice; self.admin.pending = None; self.admin.busy = true;
                self.send(Action::AdminLoad { tab:self.admin.tab, search:self.admin.search.clone(), offset:self.admin.offset });
            },
            Update::EncryptionKeys(conversation, keys) => {
                if self.options.encryption_selected == Some(conversation) {
                    self.options.encryption_keys = Some((conversation, keys));
                }
            }
            Update::HistoryBackupFinished { imported, result } => {
                self.options.history_backup_busy = false;
                let succeeded = matches!(result, Ok(true));
                self.options.history_backup_status = Some(result);
                if imported && succeeded {
                    let ids: Vec<_> = self.tabs.clone();
                    for id in ids {
                        if let Some(history) = self.histories.get_mut(&id) {
                            history.messages.clear();
                            history.loaded = false;
                        }
                        self.load_history(id, None, String::new());
                    }
                }
            }
            Update::Preferences(prefs) => {
                crate::i18n::set_preference(prefs.language);
                crate::ui::window::load_sizes(ctx, prefs.window_sizes.clone());
                self.login.prefs = prefs;
                ctx.request_repaint();
            },
            Update::AutoLogin {
                server,
                email,
                password,
                device,
                presence,
            } => {
                if self.data.is_none() && !self.login.busy {
                    self.login.password = password.clone();
                    self.login.busy = true;
                    self.errors.clear();
                    self.send(Action::Login {
                        server,
                        email,
                        password,
                        identity_password: None,
                        device,
                        name: None,
                        presence,
                    });
                }
            }
            Update::LocalAssets(assets) => self.local_assets = assets,
            Update::Connection(value) => self.connection = value,
            Update::Notice(value) => self.notice = value,
            Update::Error(error) => self.error(error),
            Update::ActivityFailed { conversation, activity, error } => {
                if let Some(conversation) = conversation { self.offered_calls.remove(&conversation); }
                if let Some(activity) = activity { self.authorized_calls.remove(&activity); }
                self.error(error);
            }
            Update::AuthenticationFailed(error) => {
                self.login.password.zeroize();
                self.login.identity_recovery = error.contains("Não foi possível desbloquear a identidade local");
                self.login.identity_password.zeroize();
                if self.data.is_some() {
                    self.logout();
                }
                self.login.busy = false;
                self.send(Action::ClearSavedSession);
                self.error(error);
            }
            Update::LoggedOut => {
                if self.data.is_some() {
                    self.clear_session();
                    self.error("Sua sessão foi encerrada.");
                } else {
                    self.clear_session();
                }
            }
            Update::Bootstrap(bootstrap) => {
                self.login.password.zeroize();
                self.login.identity_password.zeroize();
                self.login.identity_recovery = false;
                self.login.busy = false;
                if self.current_call.is_some_and(|id| {
                    !bootstrap
                        .activities
                        .iter()
                        .any(|a| a.id == id && a.status == ActivityStatus::Accepted)
                }) {
                    self.send(Action::StopCall);
                }
                let avatar_file = bootstrap
                    .me
                    .avatar_id
                    .and_then(|id| {
                        bootstrap
                            .assets
                            .iter()
                            .find(|a| a.id == id)
                            .map(|a| a.file_id)
                    })
                    .or_else(|| crate::media::scene::Scene::parse(&bootstrap.me.scene).avatar_file);
                if let Some(file_id) = avatar_file {
                    self.send(Action::CacheAvatar {
                        email: bootstrap.me.email.clone(),
                        file_id,
                    });
                }
                let ids: HashSet<_> = bootstrap.conversations.iter().map(|c| c.id).collect();
                self.tabs.retain(|id| ids.contains(id));
                self.detached.retain(|id| ids.contains(id));
                self.histories.retain(|id, _| ids.contains(id));
                if self.active.is_some_and(|id| !ids.contains(&id)) {
                    self.active = self.tabs.first().copied();
                }
                self.data = Some(*bootstrap);
                if self
                    .media_status
                    .as_ref()
                    .is_some_and(|status| status.paused)
                    && let Some(id) = self.current_call
                    && self.authorized_calls.contains(&id)
                    && let Some(activity) = self
                        .data
                        .as_ref()
                        .and_then(|data| {
                            data.activities
                                .iter()
                                .find(|a| a.id == id && a.status == ActivityStatus::Accepted)
                        })
                        .cloned()
                {
                    let config = self.media_config();
                    if let Some(status) = &mut self.media_status {
                        status.paused = false;
                        status.ready.clear();
                    }
                    self.send(Action::StartCall {
                        activity,
                        config,
                        restart: true,
                    });
                }
            }
            Update::Profile(profile) => self.update_profile(profile),
            Update::Settings(settings) => {
                if self.options.loading {
                    self.options.settings = settings.clone();
                    self.options.loading = false;
                }
                if let Some(data) = &mut self.data {
                    data.settings = settings;
                }
            }
            Update::Devices(devices) => self.options.devices = devices,
            Update::SoundPlayed => {}
            Update::SoundFailed(error) => self.options.sound_status = error,
            Update::Announcements(feed) => {
                if !feed.can_manage {
                    self.announcements.open = false;
                    self.admin = admin::Admin::default();
                }
                self.announcements.feed = feed;
                self.announcements.loading = false;
                self.announcements.last_refresh = Some(Instant::now());
            }
            Update::AnnouncementSaved(item) => {
                self.announcements.feed.items.retain(|a| a.id != item.id);
                self.announcements.edit(Some(&item));
                self.announcements.feed.items.insert(0, item);
                self.announcements.busy = false;
                self.announcements.notice = "Anúncio salvo".into();
            }
            Update::AnnouncementDeleted(id) => {
                self.announcements.feed.items.retain(|a| a.id != id);
                if self.announcements.selected == Some(id) {
                    self.announcements.edit(None);
                }
                self.announcements.busy = false;
                self.announcements.notice = "Anúncio excluído".into();
            }
            Update::AnnouncementFailed(error) => {
                self.announcements.loading = false;
                self.announcements.last_refresh = Some(Instant::now());
                self.announcements.busy = false;
                self.announcements.error = Some(error);
            }
            Update::AnnouncementImage(info) => {
                if let Some(info) = info {
                    self.announcements.draft.image_file_id = Some(info.id);
                    self.file_info.insert(info.id, info);
                }
                self.announcements.busy = false;
            }
            Update::BioImage(info) => {
                if let Some(info) = info {
                    let tag = format!("\n![{}]({})\n", info.filename, info.id);
                    self.contact_forms.edit_bio.push_str(&tag);
                    self.file_info.insert(info.id, info);
                }
            }
            Update::SettingsSaved {
                profile,
                settings,
                mut local,
            } => {
                let media_changed = self.login.prefs.audio_input != local.audio_input
                    || self.login.prefs.audio_output != local.audio_output
                    || self.login.prefs.camera_device != local.camera_device;
                self.update_profile(profile);
                if let Some(data) = &mut self.data {
                    data.settings = settings.clone();
                }
                self.options.settings = settings;
                if local.window_sizes != self.login.prefs.window_sizes {
                    local.window_sizes = self.login.prefs.window_sizes.clone();
                    self.send(Action::SavePreferences(local.clone()));
                }
                crate::i18n::set_preference(local.language);
                self.login.prefs = local;
                ctx.request_repaint();
                if media_changed {
                    crate::media::playback::stop();
                    if let Some(id) = self.current_call
                        && let Some(activity) = self
                            .data
                            .as_ref()
                            .and_then(|data| {
                                data.activities
                                    .iter()
                                    .find(|a| a.id == id && a.status == ActivityStatus::Accepted)
                            })
                            .cloned()
                    {
                        self.send(Action::StartCall {
                            activity,
                            config: self.media_config(),
                            restart: true,
                        });
                    }
                }
                self.options.saving = false;
                self.options.open = false;
                self.notice = "Configurações salvas".into();
            }
            Update::SettingsSaveFailed(error) => {
                self.options.saving = false;
                self.options.error = Some(error);
            }
            Update::SettingsLoadFailed(error) => {
                self.options.loading = false;
                self.options.error = Some(error);
            }
            Update::Sessions(sessions) => {
                if let Some(data) = &mut self.data {
                    data.sessions = sessions;
                }
            }
            Update::Contacts(contacts) => {
                if let Some(data) = &mut self.data {
                    data.contacts = contacts;
                }
            }
            Update::Contact(contact) => {
                if let Some(data) = &mut self.data {
                    data.contacts.retain(|c| c.user.id != contact.user.id);
                    data.contacts.push(contact);
                }
            }
            Update::ContactRemoved(id) => {
                if let Some(data) = &mut self.data {
                    data.contacts.retain(|c| c.user.id != id);
                }
            }
            Update::Category(category) => {
                if let Some(data) = &mut self.data {
                    data.categories.retain(|c| c.id != category.id);
                    data.categories.push(category);
                }
            }
            Update::CategoryRemoved(id) => {
                if let Some(data) = &mut self.data {
                    data.categories.retain(|c| c.id != id);
                    for contact in &mut data.contacts {
                        contact.category_ids.retain(|c| *c != id);
                    }
                }
            }
            Update::Conversations(conversations) => {
                if let Some(data) = &mut self.data {
                    data.conversations = conversations;
                }
            }
            Update::ConversationInvites(invites) => {
                if let Some(data) = &mut self.data {
                    data.conversation_invites = invites;
                }
            }
            Update::InviteDeclined(id) => {
                if let Some(data) = &mut self.data {
                    data.conversation_invites.retain(|i| i.conversation_id != id);
                }
            }
            Update::Conversation { conversation, open } => {
                let id = conversation.id;
                if let Some(data) = &mut self.data {
                    data.conversations.retain(|c| c.id != id);
                    data.conversations.push(conversation);
                }
                if open {
                    self.open_conversation(id);
                }
            }
            Update::MemberRemoved(id, user) => {
                if let Some(data) = &mut self.data {
                    if user == data.me.id {
                        data.conversations.retain(|c| c.id != id);
                        self.tabs.retain(|c| *c != id);
                        self.detached.remove(&id);
                        self.histories.remove(&id);
                        if self.active == Some(id) {
                            self.active = self.tabs.first().copied();
                        }
                    } else if let Some(c) = data.conversations.iter_mut().find(|c| c.id == id) {
                        c.members.retain(|m| m.id != user);
                    }
                }
            }
            Update::History {
                id,
                before,
                query,
                messages,
                revision,
            } => {
                let history = self.histories.entry(id).or_default();
                if history.query != query || history.revision != revision {
                    return;
                }
                history.loading = false;
                history.loaded = true;
                history.error = None;
                if before.is_some() || history.messages.is_empty() {
                    history.more = messages.len() >= 50;
                }
                history.messages.extend(messages);
                history.messages.sort_by_key(|m| (m.created_at, m.id));
                history.messages.dedup_by_key(|m| m.id);
                self.watch();
            }
            Update::HistoryFailed(id, revision, error) => {
                let history = self.histories.entry(id).or_default();
                if history.revision != revision {
                    return;
                }
                history.loading = false;
                history.error = Some(error.clone());
                self.error(error);
            }
            Update::Message(message) => self.receive_message(message, false),
            Update::PreparedMessage {
                conversation,
                request,
            } => self.queue_message(conversation, request),
            Update::SendFailed(id, error) => {
                if let Some(pending) = self.pending.get_mut(&id) {
                    pending.error = Some(error.clone());
                    self.error(error);
                }
            }
            Update::MessageEditFailed(id, error) => {
                if self.editing_message.as_ref().is_some_and(|message| message.id == id) {
                    self.edit_message_busy = false;
                    self.edit_message_error = Some(error);
                } else { self.error(error); }
            }
            Update::HistoryCleared(id) => {
                let revision = self.histories.get(&id).map_or(1, |h| h.revision + 1);
                self.histories.insert(
                    id,
                    History {
                        loaded: true,
                        revision,
                        ..Default::default()
                    },
                );
                self.marked_read.remove(&id);
                self.notice = "Histórico ocultado para esta conta.".into();
                self.watch();
            }
            Update::ReadComplete(id, message) => {
                self.read_retry.remove(&id);
                if let Some(c) = self
                    .data
                    .as_mut()
                    .and_then(|d| d.conversations.iter_mut().find(|c| c.id == id))
                    && c.last_message.as_ref().map(|m| m.id) == message
                {
                    c.unread = 0;
                }
            }
            Update::ReadFailed(id, message, error) => {
                if self.marked_read.get(&id).copied() == message {
                    self.marked_read.remove(&id);
                }
                self.read_retry.insert(id, Instant::now());
                self.error(format!("Confirmação de leitura: {error}"));
            }
            Update::Asset(asset) => {
                if let Some(data) = &mut self.data {
                    data.assets.retain(|a| a.id != asset.id);
                    data.assets.push(asset);
                }
            }
            Update::EmoticonUploadFinished(result) => {
                self.options.emoticon_busy = false;
                if matches!(result, Ok(true)) {
                    self.options.asset_name.clear();
                    self.options.shortcut.clear();
                }
                self.options.emoticon_status = Some(result);
            }
            Update::AssetRemoved(id) => {
                if let Some(data) = &mut self.data {
                    data.assets.retain(|a| a.id != id);
                }
            }
            Update::Image { source, frames } => self.images.insert(ctx, source, frames),
            Update::ImageFailed(source, error) => {
                self.images.pending.remove(&source);
                self.images.failed.insert(source);
                self.error(format!("Imagem: {error}"));
            }
            Update::FileInfo(info) => {
                self.file_info.insert(info.id, info);
            }
            Update::Event(event) => self.server_event(event),
            Update::Recording {
                epoch,
                conversation,
                active,
            } => {
                if epoch >= self.recording_epoch {
                    self.recording_epoch = epoch;
                    self.recording = active;
                    self.recording_conversation = active.then_some(conversation);
                    self.recording_started_at = if active {
                        self.recording_started_at.or_else(|| Some(Instant::now()))
                    } else {
                        None
                    };
                }
            }
            Update::CallStopped => {
                self.current_call = None;
                self.mobile_call_view = None;
                self.media_status = None;
                self.authorized_calls.clear();
                self.live_frames.clear();
                self.camera_disabled.clear();
                self.screen_frames.clear();
                self.screen_disabled.clear();
                self.remote_controls.clear();
            }
            Update::CallPaused => {
                if let Some(status) = &mut self.media_status {
                    status.paused = true;
                }
                self.live_frames.clear();
                self.screen_frames.clear();
            }
            Update::CallMedia(event) => {
                use crate::media::calls::{CallEvent, CallStatus};
                match event {
                    CallEvent::Starting { activity, epoch }
                        if self.current_call == Some(activity) =>
                    {
                        let muted = self
                            .media_status
                            .as_ref()
                            .is_some_and(|status| status.activity == activity && status.muted);
                        let default_camera = self.data.as_ref().is_some_and(|data| data.activities.iter()
                            .any(|item| item.id == activity && item.kind == ActivityKind::VideoCall));
                        let camera_enabled = self.data.as_ref().is_none_or(|data| !self.camera_disabled.contains(&(activity, data.me.id)))
                            && self.media_status.as_ref().filter(|status| status.activity == activity).map_or(default_camera, |status| status.camera_enabled);
                        self.live_frames.clear();
                        self.screen_frames.clear();
                        self.media_status = Some(CallStatus {
                            activity,
                            epoch,
                            ready: HashSet::new(),
                            errors: std::collections::BTreeMap::new(),
                            paused: false,
                            muted,
                            camera_enabled,
                            screen_sharing: false,
                        });
                    }
                    CallEvent::ScreenSharing { activity, epoch, enabled } => {
                        if let Some(status) = &mut self.media_status && status.activity == activity && status.epoch == epoch {
                            status.screen_sharing = enabled;
                            if !enabled && let Some(me) = self.data.as_ref().map(|data| data.me.id) {
                                self.screen_frames.remove(&(activity, me));
                            }
                        }
                    }
                    CallEvent::Ready {
                        activity,
                        epoch,
                        component,
                    } => {
                        if let Some(status) = &mut self.media_status
                            && status.activity == activity
                            && status.epoch == epoch
                            && !status.paused
                        {
                            status.ready.insert(component);
                            status.errors.remove(&component);
                        }
                    }
                    CallEvent::Failed {
                        activity,
                        epoch,
                        component,
                        error,
                    } => {
                        let mut camera_failed = false;
                        let video_call = self.data.as_ref().is_some_and(|data| data.activities.iter()
                            .any(|item| item.id == activity && matches!(item.kind, ActivityKind::AudioCall | ActivityKind::VideoCall)));
                        if let Some(status) = &mut self.media_status
                            && status.activity == activity
                            && status.epoch == epoch
                            && !status.paused
                        {
                            status.errors.insert(component, error);
                            if video_call && component == crate::media::calls::Component::Camera {
                                status.camera_enabled = false;
                                camera_failed = true;
                            }
                        }
                        if camera_failed {
                            if let Some(me) = self.data.as_ref().map(|data| data.me.id) {
                                self.camera_disabled.insert((activity, me));
                                self.live_frames.remove(&(activity, me));
                            }
                            self.send(Action::Realtime(ClientEvent::Signal { activity_id: activity, data: serde_json::json!({"camera_enabled": false}) }));
                        }
                    }
                    _ => {}
                }
            }
            Update::AudioChanged => {}
            Update::AvatarFilePicked(path) => {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.options.avatar_choice = Some(AvatarChoice::Local { path, name });
            }
            Update::ReceiveDirectoryPicked(path) => {
                self.options.settings.receive_directory = path.to_string_lossy().into_owned();
            }
            Update::AvatarApplied => {
                self.options.avatar_saving = false;
                self.options.avatar_picker = false;
                self.options.avatar_choice = None;
            }
            Update::AvatarFailed(error) => {
                self.options.avatar_saving = false;
                self.options.avatar_error = Some(error);
            }
            Update::LiveFrame {
                activity,
                epoch,
                from,
                stream,
                frame,
            } => {
                if self.current_call != Some(activity)
                    || (stream == MediaStream::Camera && self.camera_disabled.contains(&(activity, from)))
                    || (stream == MediaStream::Screen && self.screen_disabled.contains(&(activity, from)))
                    || !self
                        .media_status
                        .as_ref()
                        .is_some_and(|status| status.epoch == epoch && !status.paused)
                {
                    return;
                }
                let image = frame.image;
                let frames = if stream == MediaStream::Screen { &mut self.screen_frames } else { &mut self.live_frames };
                if let Some(texture) = frames.get_mut(&(activity, from)) {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    frames.insert(
                        (activity, from),
                        ctx.load_texture("nm-camera", image, egui::TextureOptions::LINEAR),
                    );
                }
            }
        }
    }

    fn receive_message(&mut self, message: Message, realtime: bool) {
        let id = message.conversation_id;
        self.pending.remove(&message.client_id);
        let duplicate = self
            .histories
            .get(&id)
            .is_some_and(|h| h.messages.iter().any(|m| m.id == message.id));
        if let Some(data) = &mut self.data {
            if let Some(c) = data.conversations.iter_mut().find(|c| c.id == id) {
                let last_duplicate = c.last_message.as_ref().is_some_and(|m| m.id == message.id);
                if realtime && !duplicate && !last_duplicate && message.sender_id != data.me.id {
                    c.unread += 1;
                }
                if c.last_message
                    .as_ref()
                    .is_none_or(|m| m.created_at <= message.created_at)
                {
                    c.last_message = Some(message.clone());
                    c.updated_at = message.created_at;
                }
            } else {
                self.send(Action::Conversations);
            }
        }
        if realtime && !duplicate {
            self.notify(
                if message.kind == MessageKind::Nudge {
                    "nudge"
                } else {
                    "message"
                },
                message.sender_id,
            );
            let nudges = self.data.as_ref().is_some_and(|d| d.settings.nudges);
            match message.kind {
                MessageKind::Nudge if nudges => {
                    self.nudge(id, false);
                }
                MessageKind::Wink => {
                    if self
                        .data
                        .as_ref()
                        .is_some_and(|data| message.sender_id != data.me.id)
                    {
                        self.open_conversation(id);
                    }
                    self.winks
                        .insert(id, (message.body.clone(), Instant::now()));
                }
                _ => {}
            }
            let alerts = self
                .data
                .as_ref()
                .is_some_and(|d| d.settings.message_alerts);
            let mine = self
                .data
                .as_ref()
                .is_some_and(|d| d.me.id == message.sender_id);
            if alerts
                && !mine
                && should_notify(
                    self.active,
                    id,
                    self.root_focused,
                    self.conversation_focus.contains(&id),
                )
            {
                let preview = if self
                    .data
                    .as_ref()
                    .is_some_and(|data| data.settings.notification_previews)
                {
                    toast_preview(&message)
                } else {
                    "Você recebeu uma nova mensagem".into()
                };
                self.push_toast(message.sender_id, preview, Some(id));
            }
            if self.active != Some(id) && alerts {
                self.notice = format!("Nova mensagem de {}", self.name(message.sender_id));
            }
        }
        if let Some(history) = self.histories.get_mut(&id)
            && !duplicate
            && (history.query.is_empty()
                || message
                    .body
                    .to_lowercase()
                    .contains(&history.query.to_lowercase()))
        {
            history.messages.push(message);
            history.messages.sort_by_key(|m| (m.created_at, m.id));
        }
        self.watch();
    }

    fn update_profile(&mut self, profile: Profile) {
        if let Some(data) = &mut self.data {
            if data.me.id == profile.id {
                if self.music_pending.as_ref().is_some_and(|(value, _)| {
                    profile.now_playing.as_deref().unwrap_or_default() == value
                }) {
                    self.published_music = profile
                        .now_playing
                        .clone()
                        .filter(|value| !value.is_empty());
                    self.music_pending = None;
                }
                data.me = profile.clone();
            }
            for c in &mut data.contacts {
                if c.user.id == profile.id {
                    c.user = profile.clone();
                }
            }
            for c in &mut data.conversations {
                for m in &mut c.members {
                    if m.id == profile.id {
                        *m = profile.clone();
                    }
                }
            }
        }
    }

    fn server_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::AnnouncementsChanged => self.send(Action::Announcements),
            ServerEvent::Message { message } => self.receive_message(message, true),
            ServerEvent::MessageUpdated { message } => {
                if self.edit_message_busy && self.editing_message.as_ref().is_some_and(|item| item.id == message.id) {
                    self.editing_message = None;
                    self.edit_message_draft.clear();
                    self.edit_message_busy = false;
                    self.edit_message_error = None;
                }
                if let Some(history) = self.histories.get_mut(&message.conversation_id) {
                    if let Some(existing) = history.messages.iter_mut().find(|item| item.id == message.id) {
                        *existing = message.clone();
                    } else if history.query.is_empty() || message.body.to_lowercase().contains(&history.query.to_lowercase()) {
                        history.messages.push(message.clone());
                        history.messages.sort_by_key(|item| (item.created_at, item.id));
                    }
                }
                if let Some(conversation) = self.data.as_mut().and_then(|data| data.conversations.iter_mut()
                    .find(|item| item.id == message.conversation_id))
                    && conversation.last_message.as_ref().is_some_and(|last| last.id == message.id) {
                    conversation.last_message = Some(message);
                }
            }
            ServerEvent::MessageDeleted { conversation_id, message_id } => {
                if self.editing_message.as_ref().is_some_and(|message| message.id == message_id) {
                    self.editing_message = None;
                    self.edit_message_draft.clear();
                    self.edit_message_busy = false;
                    self.edit_message_error = None;
                }
                if let Some(history) = self.histories.get_mut(&conversation_id) {
                    history.messages.retain(|message| message.id != message_id);
                }
                self.send(Action::Conversations);
            }
            ServerEvent::Presence { user_id, presence } => {
                let is_online = !matches!(presence, Presence::Offline | Presence::Invisible);
                let was_offline = self.data.as_ref().is_some_and(|d| {
                    d.contacts.iter().any(|c| {
                        c.user.id == user_id
                            && matches!(c.user.presence, Presence::Offline | Presence::Invisible)
                    })
                });
                let is_me = self.data.as_ref().is_some_and(|d| d.me.id == user_id);
                let direct_convo = self.direct_with(user_id);
                let is_chatting_now = direct_convo
                    .is_some_and(|c_id| self.active == Some(c_id) || self.detached.contains(&c_id));
                let recently_alerted = self
                    .last_online_alerts
                    .get(&user_id)
                    .is_some_and(|last| last.elapsed() < Duration::from_secs(300));
                let notify = self
                    .data
                    .as_ref()
                    .is_some_and(|d| d.settings.online_alerts && is_online && was_offline)
                    && !is_me
                    && !is_chatting_now
                    && !recently_alerted;
                if let Some(data) = &mut self.data {
                    if notify
                        && let Some(contact) = data.contacts.iter().find(|c| c.user.id == user_id)
                    {
                        self.notice = format!("{} acabou de entrar", contact.name());
                    }
                    for c in &mut data.contacts {
                        if c.user.id == user_id {
                            c.user.presence = presence;
                        }
                    }
                    for c in &mut data.conversations {
                        for m in &mut c.members {
                            if m.id == user_id {
                                m.presence = presence;
                            }
                        }
                    }
                }
                if notify {
                    self.last_online_alerts.insert(user_id, Instant::now());
                    self.notify("online", user_id);
                    self.push_toast(user_id, "acabou de entrar".into(), direct_convo);
                }
            }
            ServerEvent::ProfileUpdated { profile } => self.update_profile(profile),
            ServerEvent::Typing {
                conversation_id,
                user_id,
                is_typing,
            } => {
                if self.data.as_ref().is_some_and(|d| d.me.id == user_id) {
                    return;
                }
                if is_typing {
                    self.typing
                        .insert((conversation_id, user_id), Instant::now());
                } else {
                    self.typing.remove(&(conversation_id, user_id));
                }
            }
            ServerEvent::Read {
                conversation_id,
                user_id,
                message_id,
            } => {
                self.reads.insert((conversation_id, user_id), message_id);
            }
            ServerEvent::Activity { activity } => {
                if activity.status == ActivityStatus::Offered {
                    self.notify("call", activity.initiator_id);
                }
                let me = self.data.as_ref().map(|d| d.me.id);
                if activity.status == ActivityStatus::Offered && me != Some(activity.initiator_id) {
                    let text = match activity.kind {
                        ActivityKind::AudioCall => "está chamando você".into(),
                        ActivityKind::VideoCall => "iniciou uma videochamada".into(),
                        ActivityKind::RemoteAssistance => "pediu para compartilhar a tela".into(),
                        ActivityKind::Game => "convidou você para jogar".into(),
                    };
                    self.push_toast(activity.initiator_id, text, Some(activity.conversation_id));
                }
                if matches!(
                    activity.kind,
                    ActivityKind::AudioCall
                        | ActivityKind::VideoCall
                        | ActivityKind::RemoteAssistance
                ) {
                    if activity.status == ActivityStatus::Offered
                        && me == Some(activity.initiator_id)
                        && self.offered_calls.get(&activity.conversation_id) == Some(&activity.kind)
                    {
                        self.offered_calls.remove(&activity.conversation_id);
                        self.authorized_calls.insert(activity.id);
                    }
                    if activity.status == ActivityStatus::Accepted
                        && self.authorized_calls.contains(&activity.id)
                        && self.current_call.is_none_or(|id| id == activity.id)
                    {
                        if cfg!(target_os = "android") && self.current_call != Some(activity.id)
                            && matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) {
                            self.mobile_call_view = Some(activity.id);
                        }
                        self.current_call = Some(activity.id);
                        self.send(Action::StartCall {
                            activity: activity.clone(),
                            config: self.media_config(),
                            restart: false,
                        });
                    } else if matches!(
                        activity.status,
                        ActivityStatus::Ended | ActivityStatus::Declined
                    ) {
                        if self.current_call == Some(activity.id) {
                            self.send(Action::StopCall);
                            self.current_call = None;
                            self.mobile_call_view = None;
                        }
                        self.authorized_calls.remove(&activity.id);
                    }
                }
                if activity.kind == ActivityKind::Game
                    && activity.status == ActivityStatus::Accepted
                {
                    self.games.entry(activity.id).or_default();
                    self.send(Action::Realtime(ClientEvent::Signal {
                        activity_id: activity.id,
                        data: serde_json::json!({"game":"tic_tac_toe","action":"sync"}),
                    }));
                }
                if let Some(data) = &mut self.data {
                    data.activities.retain(|a| a.id != activity.id);
                    data.activities.push(activity);
                }
            }
            ServerEvent::Signal {
                activity_id,
                from_user_id,
                data,
            } => {
                if self.current_call == Some(activity_id) && let Some(enabled) = data.get("camera_enabled").and_then(|value| value.as_bool()) {
                    if enabled { self.camera_disabled.remove(&(activity_id, from_user_id)); }
                    else {
                        self.camera_disabled.insert((activity_id, from_user_id));
                        self.live_frames.remove(&(activity_id, from_user_id));
                    }
                }
                if self.current_call == Some(activity_id) && let Some(enabled) = data.get("screen_sharing").and_then(|value| value.as_bool()) {
                    if enabled { self.screen_disabled.remove(&(activity_id, from_user_id)); }
                    else { self.screen_disabled.insert((activity_id, from_user_id)); self.screen_frames.remove(&(activity_id, from_user_id)); }
                }
                self.game_signal(activity_id, from_user_id, data)
            },
            ServerEvent::Error { code, message } => self.error(format!("{code}: {message}")),
            ServerEvent::SessionRevoked { .. } => {}
            ServerEvent::ContactsChanged
            | ServerEvent::ConversationsChanged
            | ServerEvent::ResyncRequired => self.send(Action::Refresh),
            _ => {}
        }
    }

    pub fn game_signal(&mut self, id: Uuid, from: Uuid, value: serde_json::Value) {
        let Some(data) = &self.data else {
            return;
        };
        let Some(activity) = data.activities.iter().find(|a| {
            a.id == id && a.kind == ActivityKind::Game && a.status == ActivityStatus::Accepted
        }) else {
            return;
        };
        let Some(conversation) = data
            .conversations
            .iter()
            .find(|c| c.id == activity.conversation_id && c.members.len() == 2)
        else {
            return;
        };
        if !conversation.members.iter().any(|m| m.id == from) || value["game"] != "tic_tac_toe" {
            return;
        }
        let initiator = activity.initiator_id;
        let Some(other) = conversation
            .members
            .iter()
            .find(|m| m.id != initiator)
            .map(|m| m.id)
        else {
            return;
        };
        let game = self.games.entry(id).or_default();
        match value["action"].as_str() {
            Some("move") => {
                if let Some(cell) = value["cell"].as_u64() {
                    game.apply(from, cell as usize, initiator, other);
                }
            }
            Some("pass_turn") => {
                game.pass_turn();
            }
            Some("sync") => {
                let board = game.board;
                let pass_count = game.pass_count;
                self.send(Action::Realtime(ClientEvent::Signal {
                    activity_id: id,
                    data: serde_json::json!({"game":"tic_tac_toe","action":"state","board":board,"pass_count":pass_count}),
                }));
            }
            Some("state") if game.board.iter().all(|c| c.is_none()) => {
                if let Some(p) = value["pass_count"].as_u64() {
                    game.pass_count = p as usize;
                }
                if let Ok(board) =
                    serde_json::from_value::<[Option<Uuid>; 9]>(value["board"].clone())
                {
                    let x = board.iter().filter(|c| **c == Some(initiator)).count();
                    let o = board.iter().filter(|c| **c == Some(other)).count();
                    if board
                        .iter()
                        .all(|c| c.is_none() || *c == Some(initiator) || *c == Some(other))
                        && (x == o || x == o + 1)
                    {
                        game.board = board;
                    }
                }
            }
            _ => {}
        }
    }
}

fn toast_preview(message: &Message) -> String {
    match message.kind {
        MessageKind::Text => {
            let text: String = message.body.chars().take(140).collect();
            if text.trim().is_empty() {
                "Enviou uma mensagem".into()
            } else {
                text
            }
        }
        MessageKind::Nudge => "chamou sua atenção".into(),
        MessageKind::Wink => "enviou uma piscadela".into(),
        MessageKind::Voice => "enviou uma mensagem de voz".into(),
        MessageKind::Photo => "compartilhou uma foto".into(),
        MessageKind::File => "enviou um arquivo".into(),
        MessageKind::Ink => "enviou uma mensagem manuscrita".into(),
        _ => "Nova atividade na conversa".into(),
    }
}
