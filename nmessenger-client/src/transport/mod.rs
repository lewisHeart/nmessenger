mod actions;
pub(crate) mod api;
mod crypto_events;
mod crypto_files;
mod crypto_http;
mod files;
mod outbox;
mod opaque;
#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod tests;
mod websocket;

pub use crate::media::images::{DecodedFrame, ImageSource, LocalAsset};
use crate::{media::calls::MediaController, storage::Preferences};
pub use api::{Api, base as normalize_server};
use eframe::egui;
use nm_protocol::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
};
use tokio::sync::mpsc as async_mpsc;
use uuid::Uuid;

#[derive(Clone, Copy, Default)]
pub struct Watch {
    pub latest: Option<Uuid>,
    pub revision: u64,
}

pub enum Action {
    DownloadRelease(String),
    Login {
        server: String,
        email: String,
        password: String,
        identity_password: Option<String>,
        device: String,
        name: Option<String>,
        presence: Presence,
    },
    Logout,
    SaveSavedSession {
        server: String,
        email: String,
        password: String,
    },
    ClearSavedSession,
    Health(String),
    SavePreferences(Preferences),
    ApplySettings {
        profile: UpdateProfileRequest,
        settings: UserSettings,
        local: Preferences,
    },
    DiscoverDevices,
    Announcements,
    AdminLoad { tab: usize, search: String, offset: i64 },
    AdminUpdateUser(Uuid, AdminUpdateUser),
    AdminRevokeUser(Uuid),
    AdminResetPassword(Uuid, String, String),
    AdminModerateUser(Uuid, Option<bool>, String),
    AdminRevokeSession(Uuid),
    AdminCreateBadge(CreateBadgeRequest),
    AdminUpdateBadge(Uuid, UpdateBadgeRequest),
    AdminDeleteBadge(Uuid),
    AdminMutateUserBadge { user_id: Uuid, mutation: UserBadgeMutation },
    SaveAnnouncement(Option<Uuid>, SaveAnnouncementRequest),
    DeleteAnnouncement(Uuid),
    UploadAnnouncementImage,
    UploadBioImage,
    DiscoverAssets(String),
    Refresh,
    Watch(HashMap<Uuid, Watch>),
    Profile(UpdateProfileRequest),
    Settings(Option<UserSettings>),
    Sessions,
    EncryptionKeys(Uuid),
    ExportIdentityBackup,
    ImportIdentityBackup { password: String },
    VerifyConversationRoster {
        conversation: Uuid,
        fingerprint: String,
    },
    VerifyEncryptionKey {
        conversation: Uuid,
        device: Uuid,
        fingerprint: String,
    },
    RevokeSession(Uuid),
    #[allow(dead_code)]
    Contacts,
    AddContact(AddContactRequest),
    Contact(Uuid, UpdateContactRequest),
    RemoveContact(Uuid),
    ImportContacts,
    ExportContacts(String),
    Category(Option<Uuid>, String),
    RemoveCategory(Uuid),
    Conversations,
    CreateConversation(CreateConversationRequest),
    RenameConversation(Uuid, String),
    SetGroupInvitePermission(Uuid, bool),
    AcceptGroupInvite(Uuid),
    DeclineGroupInvite(Uuid),
    AddMember(Uuid, Uuid),
    TransferGroupOwner(Uuid, Uuid),
    RemoveMember(Uuid, Uuid),
    History {
        id: Uuid,
        before: Option<Uuid>,
        query: String,
        revision: u64,
    },
    Send(Uuid, SendMessageRequest),
    EditMessage { conversation: Uuid, message: Uuid, request: SendMessageRequest },
    DeleteMessage { conversation: Uuid, message: Uuid },
    Read(Uuid, Option<Uuid>),
    ClearHistory(Uuid),
    ExportHistory {
        id: Uuid,
        title: String,
        format: String,
        names: Vec<(Uuid, String)>,
    },
    Upload {
        conversation: Uuid,
        kind: MessageKind,
    },
    UploadAsset {
        kind: AssetKind,
        name: String,
        shortcut: Option<String>,
        local: Option<std::path::PathBuf>,
    },
    DeleteAsset(Uuid),
    UseAsset(UserAsset),
    UseAvatarPreset {
        name: String,
        asset: String,
    },
    PickAvatarFile,
    PickReceiveDirectory(Option<String>),
    SendEmoticon {
        conversation: Uuid,
        asset: UserAsset,
    },
    LoadImage(ImageSource),
    CacheAvatar {
        email: String,
        file_id: Uuid,
    },
    FileInfo(Uuid),
    Download(Uuid),
    ForwardAttachment { file: Uuid, conversation: Uuid, kind: MessageKind },
    PlayVoice {
        file: Uuid,
        output: String,
    },
    LoadVoice {
        file: Uuid,
    },
    Notification {
        file: Option<Uuid>,
        event: String,
        output: String,
    },
    StopPlayback,
    Activity(Uuid, ActivityRequest),
    UpdateActivity(Uuid, UpdateActivityRequest),
    Realtime(ClientEvent),
    StartVoice {
        conversation: Uuid,
        input: String,
    },
    StopVoice {
        cancel: bool,
    },
    StartCall {
        activity: Activity,
        config: crate::media::MediaConfig,
        restart: bool,
    },
    MuteMicrophone(bool),
    SetCameraEnabled(bool),
    SetScreenSharing(bool),
    StopCall,
    RemoteControl {
        activity: Uuid,
        enabled: bool,
    },
}

pub enum Update {
    ReleaseDownload(Result<Option<std::path::PathBuf>, String>),
    Preferences(Preferences),
    AutoLogin {
        server: String,
        email: String,
        password: String,
        device: String,
        presence: Presence,
    },
    LocalAssets(Vec<LocalAsset>),
    Connection(String),
    Notice(String),
    Release(ReleaseInfo),
    Error(String),
    ActivityFailed { conversation: Option<Uuid>, activity: Option<Uuid>, error: String },
    AuthenticationFailed(String),
    LoggedOut,
    Bootstrap(Box<Bootstrap>),
    Profile(Profile),
    Settings(UserSettings),
    Sessions(Vec<SessionInfo>),
    EncryptionKeys(Uuid, Vec<EncryptionKey>),
    HistoryBackupFinished { imported: bool, result: Result<bool, String> },
    HistoryKeysRestored,
    Contacts(Vec<Contact>),
    Contact(Contact),
    ContactRemoved(Uuid),
    Category(Category),
    CategoryRemoved(Uuid),
    Conversations(Vec<Conversation>),
    ConversationInvites(Vec<ConversationInvite>),
    InviteDeclined(Uuid),
    Conversation {
        conversation: Conversation,
        open: bool,
    },
    MemberRemoved(Uuid, Uuid),
    History {
        id: Uuid,
        before: Option<Uuid>,
        query: String,
        messages: Vec<Message>,
        revision: u64,
    },
    HistoryFailed(Uuid, u64, String),
    Message(Message),
    PreparedMessage {
        conversation: Uuid,
        request: SendMessageRequest,
    },
    SendFailed(Uuid, String),
    MessageEditFailed(Uuid, String),
    HistoryCleared(Uuid),
    ReadComplete(Uuid, Option<Uuid>),
    ReadFailed(Uuid, Option<Uuid>, String),
    Asset(UserAsset),
    EmoticonUploadFinished(Result<bool, String>),
    AssetRemoved(Uuid),
    Image {
        source: ImageSource,
        frames: Vec<DecodedFrame>,
    },
    ImageFailed(ImageSource, String),
    FileInfo(FileInfo),
    Event(ServerEvent),
    Recording {
        epoch: u64,
        conversation: Uuid,
        active: bool,
    },
    CallStopped,
    CallPaused,
    CallMedia(crate::media::calls::CallEvent),
    AudioChanged,
    SoundPlayed,
    SoundFailed(String),
    AvatarFilePicked(std::path::PathBuf),
    ReceiveDirectoryPicked(std::path::PathBuf),
    AvatarApplied,
    AvatarFailed(String),
    Devices(crate::media::devices::DeviceCatalog),
    SettingsSaved {
        profile: Profile,
        settings: UserSettings,
        local: Preferences,
    },
    SettingsSaveFailed(String),
    SettingsLoadFailed(String),
    Announcements(AnnouncementFeed),
    AdminOverview(AdminOverview),
    AdminUsers(AdminPage<AdminUser>),
    AdminSessions(AdminPage<AdminSession>),
    AdminAudit(AdminPage<AdminAuditEntry>),
    AdminBadges(Vec<AdminBadge>),
    AdminChanged(String),
    AdminFailed(String),
    AnnouncementSaved(Announcement),
    AnnouncementDeleted(Uuid),
    AnnouncementFailed(String),
    AnnouncementImage(Option<FileInfo>),
    BioImage(Option<FileInfo>),
    LiveFrame {
        activity: Uuid,
        epoch: u64,
        from: Uuid,
        stream: MediaStream,
        frame: DecodedFrame,
    },
}

pub struct Packet {
    pub generation: u64,
    pub update: Update,
}

#[derive(Clone)]
pub struct Bus {
    generation: u64,
    sender: mpsc::Sender<Packet>,
    context: egui::Context,
}

#[cfg(test)]
pub(crate) fn test_bus() -> (Bus, mpsc::Receiver<Packet>) {
    let (sender, receiver) = mpsc::channel();
    (
        Bus {
            generation: 0,
            sender,
            context: egui::Context::default(),
        },
        receiver,
    )
}

impl Bus {
    pub fn send(&self, update: Update) {
        let _ = self.sender.send(Packet {
            generation: self.generation,
            update,
        });
        self.context.request_repaint_of(egui::ViewportId::ROOT);
    }
}

pub struct Transport {
    sender: async_mpsc::UnboundedSender<(u64, Action)>,
    pub receiver: mpsc::Receiver<Packet>,
    context: egui::Context,
}

impl Transport {
    pub fn new(context: egui::Context) -> Self {
        let ui_context = context.clone();
        let (sender, receiver) = async_mpsc::unbounded_channel();
        let (updates, ui_receiver) = mpsc::channel();
        let bus = Bus {
            generation: 0,
            sender: updates,
            context,
        };
        std::thread::Builder::new()
            .name("nm-network".into())
            .spawn(move || {
                match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(3)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(worker(receiver, bus)),
                    Err(e) => bus.send(Update::Error(format!("Inicialização do Tokio: {e}"))),
                }
            })
            .expect("Não foi possível iniciar o worker de rede");
        Self {
            sender,
            receiver: ui_receiver,
            context: ui_context,
        }
    }

    pub fn send(&self, generation: u64, action: Action) {
        let _ = self.sender.send((generation, action));
    }

    pub fn repaint_viewport(&self, viewport: egui::ViewportId) {
        self.context.send_viewport_cmd_to(
            viewport,
            egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational),
        );
        self.context.request_repaint_of(viewport);
        self.context.request_repaint();
    }
}

async fn worker(mut commands: async_mpsc::UnboundedReceiver<(u64, Action)>, initial_bus: Bus) {
    match crate::storage::load_preferences().await {
        Ok(prefs) => {
            let directory = prefs.content_directory.clone();
            let auto_login = prefs.auto_login;
            let email = prefs.email.clone();
            let device = prefs.device.clone();
            let presence = prefs.presence;
            initial_bus.send(Update::Preferences(prefs));
            discover(directory, initial_bus.clone()).await;
            if auto_login
                && !email.is_empty()
                && let Ok(Some(saved)) = crate::storage::load_saved_session().await
                && saved.email == email
                && !saved.password.is_empty()
            {
                let server = if saved.server.trim().is_empty() {
                    crate::env::default_server_url()
                } else {
                    crate::env::format_server_url(&saved.server)
                };
                initial_bus.send(Update::AutoLogin {
                    server,
                    email: saved.email,
                    password: saved.password,
                    device,
                    presence,
                });
            }
        }
        Err(e) => initial_bus.send(Update::Error(e)),
    }
    let mut session: Option<Api> = None;
    let mut ws_task: Option<tokio::task::JoinHandle<()>> = None;
    let (mut ws_sender, _) = async_mpsc::channel(128);
    let watches = Arc::new(Mutex::new(HashMap::new()));
    let media = Arc::new(MediaController::default());
    let mut remember_login: Option<(String, String, zeroize::Zeroizing<String>)> = None;
    while let Some((generation, action)) = commands.recv().await {
        let bus = Bus {
            generation,
            ..initial_bus.clone()
        };
        match action {
            Action::Login {
                server,
                email,
                password,
                identity_password,
                device,
                name,
                presence,
            } => {
                #[cfg(target_os = "android")]
                crate::android::stop_message_notifications();
                if let Some(task) = ws_task.take() {
                    task.abort();
                }
                media.stop_all();
                session = None;
                watches.lock().unwrap().clear();
                bus.send(Update::Connection("Entrando…".into()));
                match tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    api::authenticate(server, email, password, device, name, presence, identity_password),
                )
                .await
                {
                    Ok(Ok((api, bootstrap))) => {
                        #[cfg(target_os = "android")]
                        if !matches!(crate::android::start_message_notifications(&api.base, api.token.as_str(), &crate::secure::account(&api.base, &api.user_id.to_string())), Ok(true)) {
                            bus.send(Update::Error("Não foi possível iniciar as notificações em segundo plano.".into()));
                        }
                        if let Some((server, email, password)) = remember_login.take()
                            && let Err(error) =
                                crate::storage::save_saved_session(&server, &email, &password).await
                        {
                            bus.send(Update::Error(error));
                        }
                        bus.send(Update::Bootstrap(Box::new(bootstrap)));
                        match outbox::load(&api).await {
                            Ok(messages) => {
                                for pending in messages {
                                    bus.send(Update::PreparedMessage {
                                        conversation: pending.conversation,
                                        request: pending.request,
                                    });
                                }
                            }
                            Err(error) => {
                                bus.send(Update::Error(format!("Fila de mensagens: {error}")))
                            }
                        }
                        let (tx, rx) = async_mpsc::channel(128);
                        ws_sender = tx;
                        ws_task = Some(tokio::spawn(websocket::run(
                            api.clone(),
                            rx,
                            bus.clone(),
                            watches.clone(),
                            media.clone(),
                        )));
                        session = Some(api);
                    }
                    Ok(Err(e)) => {
                        remember_login = None;
                        bus.send(Update::AuthenticationFailed(e));
                    }
                    Err(_) => {
                        remember_login = None;
                        bus.send(Update::AuthenticationFailed(
                            "Tempo limite esgotado ao conectar ao servidor. Verifique o endereço e a conexão.".into(),
                        ));
                    }
                }
            }
            Action::Logout => {
                remember_login = None;
                #[cfg(target_os = "android")]
                crate::android::stop_message_notifications();
                if let Some(task) = ws_task.take() {
                    task.abort();
                }
                media.stop_all();
                tokio::spawn(async move {
                    let _ = crate::storage::clear_saved_session().await;
                });
                if let Some(api) = session.take() {
                    tokio::spawn(async move {
                        let _ = api.empty(reqwest::Method::POST, "/auth/logout").await;
                        bus.send(Update::LoggedOut);
                    });
                } else {
                    bus.send(Update::LoggedOut);
                }
            }
            Action::SaveSavedSession {
                server,
                email,
                password,
            } => {
                remember_login = Some((server, email, zeroize::Zeroizing::new(password)));
            }
            Action::ClearSavedSession => {
                remember_login = None;
                if let Err(error) = crate::storage::clear_saved_session().await {
                    bus.send(Update::Error(error));
                }
            }
            Action::SavePreferences(prefs) => {
                tokio::spawn(async move {
                    if let Err(e) = crate::storage::save_preferences(&prefs).await {
                        bus.send(Update::Error(e));
                    }
                });
            }
            Action::DiscoverAssets(directory) => {
                tokio::spawn(discover(directory, bus));
            }
            Action::DiscoverDevices => {
                tokio::task::spawn_blocking(move || {
                    bus.send(Update::Devices(crate::media::devices::discover()))
                });
            }
            Action::Health(server) => {
                tokio::spawn(async move {
                    match api::health(&server).await {
                        Ok(result) => bus.send(Update::Notice(result)),
                        Err(e) => bus.send(Update::Error(e)),
                    }
                });
            }
            Action::Watch(ids) => {
                *watches.lock().unwrap() = ids;
            }
            Action::Realtime(event) => {
                if ws_sender.try_send(event).is_err() {
                    bus.send(Update::Error(
                        "Tempo real desconectado; reconecte para enviar sinais.".into(),
                    ));
                }
            }
            Action::StopVoice { cancel } => media.stop_voice(cancel),
            Action::StopPlayback => {
                crate::media::audio::stop_playback();
                crate::media::playback::stop();
            }
            Action::StopCall => {
                media.stop_call();
                bus.send(Update::CallStopped);
            }
            Action::StartCall {
                activity,
                config,
                restart,
            } => {
                if let Some(api) = &session {
                    media.start_call(
                        activity,
                        config,
                        api.user_id,
                        ws_sender.clone(),
                        bus,
                        restart,
                    );
                }
            }
            Action::MuteMicrophone(muted) => media.mute(muted),
            Action::SetCameraEnabled(enabled) => media.camera(enabled),
            Action::SetScreenSharing(enabled) => media.screen(enabled, ws_sender.clone(), bus),
            Action::RemoteControl { activity, enabled } => media.remote_control(activity, enabled),
            Action::StartVoice {
                conversation,
                input,
            } => {
                if let Some(api) = session.clone() {
                    media.start_voice(conversation, input, api, bus);
                }
            }
            Action::CacheAvatar { email, file_id } => {
                if let Some(api) = session.clone() {
                    tokio::spawn(async move {
                        if let Ok(bytes) = api.bytes(file_id).await {
                            let _ = crate::storage::save_cached_avatar(&api.base, &email, &bytes).await;
                        }
                    });
                }
            }
            Action::LoadImage(source) => {
                let bus = bus.clone();
                match &source {
                    ImageSource::Local(path) => {
                        let path = path.clone();
                        let src = source.clone();
                        tokio::spawn(async move {
                            match tokio::fs::read(&path).await {
                                Ok(bytes) => {
                                    match tokio::task::spawn_blocking(move || {
                                        crate::media::images::decode_sized(&bytes, None)
                                    })
                                    .await
                                    {
                                        Ok(Ok(frames)) => bus.send(Update::Image {
                                            source: src,
                                            frames,
                                        }),
                                        Ok(Err(e)) => bus.send(Update::ImageFailed(src, e)),
                                        Err(e) => bus.send(Update::ImageFailed(src, e.to_string())),
                                    }
                                }
                                Err(e) => bus.send(Update::ImageFailed(src, e.to_string())),
                            }
                        });
                    }
                    ImageSource::Bundled(name) => {
                        let src = source.clone();
                        let name = (*name).to_string();
                        tokio::spawn(async move {
                            if let Some(bytes) = crate::resources::bytes(&name) {
                                let bytes = bytes.to_vec();
                                match tokio::task::spawn_blocking(move || {
                                    crate::media::images::decode_sized(&bytes, Some(48))
                                })
                                .await
                                {
                                    Ok(Ok(frames)) => bus.send(Update::Image {
                                        source: src,
                                        frames,
                                    }),
                                    Ok(Err(e)) => bus.send(Update::ImageFailed(src, e)),
                                    Err(e) => bus.send(Update::ImageFailed(src, e.to_string())),
                                }
                            } else {
                                bus.send(Update::ImageFailed(src, "Imagem não encontrada".into()));
                            }
                        });
                    }
                    ImageSource::File(_) => {
                        if let Some(api) = session.clone() {
                            tokio::spawn(async move {
                                if let Err(error) =
                                    files::load_image(&api, &bus, source.clone()).await
                                {
                                    bus.send(Update::ImageFailed(source, error));
                                }
                            });
                        } else {
                            bus.send(Update::ImageFailed(source, "Sessão não iniciada".into()));
                        }
                    }
                }
            }
            other => {
                if let Some(api) = session.clone() {
                    tokio::spawn(async move {
                        actions::execute(api, other, bus).await;
                    });
                }
            }
        }
    }
    if let Some(task) = ws_task {
        task.abort();
    }
    media.stop_all();
}

async fn discover(directory: String, bus: Bus) {
    let path = (!directory.trim().is_empty()).then(|| directory.into());
    match tokio::task::spawn_blocking(move || crate::media::images::discover(path)).await {
        Ok(Ok(assets)) => bus.send(Update::LocalAssets(assets)),
        Ok(Err(e)) => bus.send(Update::Error(e)),
        Err(e) => bus.send(Update::Error(e.to_string())),
    }
}
