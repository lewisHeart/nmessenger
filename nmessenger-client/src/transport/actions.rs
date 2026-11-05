use super::{Action, Api, Bus, Update, files};
use nm_protocol::*;
use reqwest::Method;

pub async fn execute(api: Api, action: Action, bus: Bus) {
    let editing_message = match &action { Action::EditMessage { message, .. } => Some(*message), _ => None };
    let activity_request = match &action {
        Action::Activity(conversation, request) if matches!(request.kind, ActivityKind::AudioCall | ActivityKind::VideoCall | ActivityKind::RemoteAssistance) => Some((Some(*conversation), None)),
        Action::UpdateActivity(activity, request) if request.status == ActivityStatus::Accepted => Some((None, Some(*activity))),
        _ => None,
    };
    let history_backup = match &action {
        Action::ExportIdentityBackup => Some(false),
        Action::ImportIdentityBackup { .. } => Some(true),
        _ => None,
    };
    let admin = matches!(
        &action,
        Action::AdminLoad { .. }
            | Action::AdminUpdateUser(..)
            | Action::AdminRevokeUser(_)
            | Action::AdminResetPassword(..)
            | Action::AdminModerateUser(..)
            | Action::AdminRevokeSession(_)
            | Action::AdminCreateBadge(_)
            | Action::AdminUpdateBadge(..)
            | Action::AdminDeleteBadge(_)
            | Action::AdminMutateUserBadge { .. }
    );
    let announcement = matches!(
        &action,
        Action::Announcements
            | Action::SaveAnnouncement(..)
            | Action::DeleteAnnouncement(_)
            | Action::UploadAnnouncementImage
    );
    let avatar = matches!(&action, Action::UseAvatarPreset { .. })
        || matches!(&action, Action::UseAsset(asset) if asset.kind == AssetKind::Avatar)
        || matches!(
            &action,
            Action::UploadAsset {
                kind: AssetKind::Avatar,
                ..
            }
        );
    let emoticon = matches!(&action, Action::UploadAsset { kind: AssetKind::Emoticon, .. });
    if let Err(error) = run(&api, action, &bus).await {
        if let Some(message) = editing_message {
            bus.send(Update::MessageEditFailed(message, error));
        } else if let Some(imported) = history_backup {
            bus.send(Update::HistoryBackupFinished { imported, result: Err(error) });
        } else if let Some((conversation, activity)) = activity_request {
            bus.send(Update::ActivityFailed { conversation, activity, error });
        } else if admin {
            bus.send(Update::AdminFailed(error));
        } else if announcement {
            bus.send(Update::AnnouncementFailed(error));
        } else if avatar {
            bus.send(Update::AvatarFailed(error));
        } else if emoticon {
            bus.send(Update::EmoticonUploadFinished(Err(error)));
        } else {
            bus.send(Update::Error(error));
        }
    } else if avatar {
        bus.send(Update::AvatarApplied);
    }
}

async fn run(api: &Api, action: Action, bus: &Bus) -> Result<(), String> {
    match action {
        Action::DownloadRelease(tag) => {
            bus.send(Update::ReleaseDownload(crate::updater::download(&tag).await));
        }
        Action::ExportIdentityBackup => files::export_identity_backup(api, bus).await?,
        Action::ImportIdentityBackup { password } => files::import_identity_backup(api, bus, password).await?,
        Action::AdminLoad { tab, search, offset } => {
            if tab == 0 { bus.send(Update::AdminOverview(api.get("/admin/overview").await?)); }
            else if tab == 5 {
                bus.send(Update::AdminBadges(api.get("/admin/badges").await?));
            } else {
                let path = match tab { 1 => "/admin/users", 2 => "/admin/sessions", 4 => "/admin/audit", _ => return Err("Aba administrativa inválida".into()) };
                let response = super::api::checked(api.request(Method::GET, path).query(&serde_json::json!({"q":search,"offset":offset,"limit":25})).send().await.map_err(|error| error.to_string())?).await?;
                match tab {
                    1 => bus.send(Update::AdminUsers(api.decode_response(response).await?)),
                    2 => bus.send(Update::AdminSessions(api.decode_response(response).await?)),
                    _ => bus.send(Update::AdminAudit(api.decode_response(response).await?)),
                }
            }
        }
        Action::AdminUpdateUser(id, mut request) => {
            use zeroize::Zeroize;
            let result = async { super::api::checked(api.request(Method::PATCH, &format!("/admin/users/{id}")).json(&request).send().await.map_err(|error| error.to_string())?).await.map(|_| ()) }.await;
            if let Some(password) = &mut request.confirmation_password { password.zeroize(); }
            result?;
            bus.send(Update::AdminChanged("Acesso do usuário atualizado.".into()));
        }
        Action::AdminRevokeUser(id) => {
            api.empty(Method::POST, &format!("/admin/users/{id}/revoke-sessions")).await?;
            bus.send(Update::AdminChanged("Sessões do usuário encerradas.".into()));
        }
        Action::AdminResetPassword(id, mut password, mut confirmation_password) => {
            use zeroize::Zeroize;
            let result = async {
                super::api::checked(api.request(Method::POST, &format!("/admin/users/{id}/password"))
                    .json(&serde_json::json!({"password": password, "confirmation_password": confirmation_password}))
                    .send().await.map_err(|error| error.to_string())?).await.map(|_| ())
            }.await;
            password.zeroize();
            confirmation_password.zeroize();
            result?;
            bus.send(Update::AdminChanged("Senha redefinida e sessões encerradas.".into()));
        }
        Action::AdminModerateUser(id, banned, mut confirmation_password) => {
            use zeroize::Zeroize;
            let result = async {
                super::api::checked(api.request(Method::POST, &format!("/admin/users/{id}/moderate"))
                    .json(&serde_json::json!({"banned": banned, "confirmation_password": confirmation_password}))
                    .send().await.map_err(|error| error.to_string())?).await.map(|_| ())
            }.await;
            confirmation_password.zeroize();
            result?;
            bus.send(Update::AdminChanged(match banned { Some(true) => "Usuário banido.", Some(false) => "Banimento removido.", None => "Conta excluída e histórico preservado." }.into()));
        }
        Action::AdminRevokeSession(id) => {
            api.empty(Method::DELETE, &format!("/admin/sessions/{id}")).await?;
            bus.send(Update::AdminChanged("Sessão encerrada.".into()));
        }
        Action::AdminCreateBadge(request) => {
            let response = super::api::checked(
                api.request(Method::POST, "/admin/badges")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .await?;
            let _: AdminBadge = api.decode_response(response).await?;
            bus.send(Update::AdminChanged("Selo criado com sucesso.".into()));
        }
        Action::AdminUpdateBadge(id, request) => {
            let response = super::api::checked(
                api.request(Method::PUT, &format!("/admin/badges/{id}"))
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .await?;
            let _: AdminBadge = api.decode_response(response).await?;
            bus.send(Update::AdminChanged("Selo atualizado com sucesso.".into()));
        }
        Action::AdminDeleteBadge(id) => {
            api.empty(Method::DELETE, &format!("/admin/badges/{id}")).await?;
            bus.send(Update::AdminChanged("Selo excluído com sucesso.".into()));
        }
        Action::AdminMutateUserBadge { user_id, mutation } => {
            let grant = mutation.grant;
            super::api::checked(
                api.request(Method::POST, &format!("/admin/users/{user_id}/badges"))
                    .json(&mutation)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?,
            )
            .await?;
            bus.send(Update::AdminChanged(if grant {
                "Selo concedido com sucesso ao usuário!".into()
            } else {
                "Selo revogado do usuário.".into()
            }));
        }
        Action::VerifyConversationRoster {
            conversation,
            fingerprint,
        } => {
            let _: Vec<Conversation> = api.get("/conversations").await?;
            let keys = api.raw_encryption_keys(conversation).await?;
            api.crypto
                .as_ref()
                .ok_or("Criptografia indisponível")?
                .verify_roster(conversation, &keys, &fingerprint)
                .await?;
            bus.send(Update::EncryptionKeys(conversation, keys));
            bus.send(Update::Notice(
                "Participantes da conversa verificados.".into(),
            ));
        }
        Action::EncryptionKeys(conversation) => {
            let keys = api.raw_encryption_keys(conversation).await?;
            bus.send(Update::EncryptionKeys(conversation, keys));
        }
        Action::VerifyEncryptionKey {
            conversation,
            device,
            fingerprint,
        } => {
            let keys = api.raw_encryption_keys(conversation).await?;
            let key = keys
                .iter()
                .find(|key| key.device_id == device)
                .ok_or("Dispositivo não pertence mais à conversa")?
                .clone();
            api.crypto
                .as_ref()
                .ok_or("Criptografia indisponível")?
                .verify_identity(key, &fingerprint)
                .await?;
            bus.send(Update::EncryptionKeys(conversation, keys));
            bus.send(Update::Notice(
                "Identidade criptográfica verificada.".into(),
            ));
        }
        Action::Announcements => bus.send(Update::Announcements(api.get("/announcements").await?)),
        Action::SaveAnnouncement(id, request) => {
            let (method, path) = id
                .map(|id| (Method::PUT, format!("/announcements/{id}")))
                .unwrap_or((Method::POST, "/announcements".into()));
            bus.send(Update::AnnouncementSaved(
                api.json(method, &path, &request).await?,
            ));
        }
        Action::DeleteAnnouncement(id) => {
            api.empty(Method::DELETE, &format!("/announcements/{id}"))
                .await?;
            bus.send(Update::AnnouncementDeleted(id));
        }
        Action::UploadAnnouncementImage => files::upload_announcement_image(api, bus).await?,
        Action::UploadBioImage => files::upload_bio_image(api, bus).await?,
        Action::PickAvatarFile => files::pick_avatar(bus).await?,
        Action::PickReceiveDirectory(current) => files::pick_receive_directory(bus, current).await?,
        Action::Refresh => bus.send(Update::Bootstrap(Box::new(api.get("/bootstrap").await?))),
        Action::ApplySettings {
            profile,
            settings,
            local,
        } => {
            let result = async {
                let profile: Profile = api.json(Method::PATCH, "/me", &profile).await?;
                let settings: UserSettings = api.json(Method::PUT, "/settings", &settings).await?;
                crate::storage::save_preferences(&local).await?;
                Ok::<_, String>((profile, settings))
            }
            .await;
            match result {
                Ok((profile, settings)) => bus.send(Update::SettingsSaved {
                    profile,
                    settings,
                    local,
                }),
                Err(error) => bus.send(Update::SettingsSaveFailed(error)),
            }
        }
        Action::Profile(request) => bus.send(Update::Profile(
            api.json(Method::PATCH, "/me", &request).await?,
        )),
        Action::Settings(settings) => {
            let value = match settings {
                Some(value) => api.json(Method::PUT, "/settings", &value).await?,
                None => match api.get("/settings").await {
                    Ok(settings) => settings,
                    Err(error) => {
                        bus.send(Update::SettingsLoadFailed(error));
                        return Ok(());
                    }
                },
            };
            bus.send(Update::Settings(value));
        }
        Action::Sessions => bus.send(Update::Sessions(api.get("/sessions").await?)),
        Action::RevokeSession(id) => {
            api.empty(Method::DELETE, &format!("/sessions/{id}"))
                .await?;
            if id == api.session_id {
                bus.send(Update::LoggedOut);
            } else {
                bus.send(Update::Sessions(api.get("/sessions").await?));
            }
        }
        Action::Contacts => bus.send(Update::Contacts(api.get("/contacts").await?)),
        Action::AddContact(request) => bus.send(Update::Contact(
            api.json(Method::POST, "/contacts", &request).await?,
        )),
        Action::Contact(id, request) => bus.send(Update::Contact(
            api.json(Method::PATCH, &format!("/contacts/{id}"), &request)
                .await?,
        )),
        Action::RemoveContact(id) => {
            api.empty(Method::DELETE, &format!("/contacts/{id}"))
                .await?;
            bus.send(Update::ContactRemoved(id));
        }
        Action::ImportContacts => files::import_contacts(api, bus).await?,
        Action::ExportContacts(format) => files::export_contacts(api, bus, &format).await?,
        Action::Category(id, name) => {
            let (method, path) = id
                .map(|id| (Method::PUT, format!("/categories/{id}")))
                .unwrap_or_else(|| (Method::POST, "/categories".into()));
            bus.send(Update::Category(
                api.json(method, &path, &CategoryRequest { name }).await?,
            ));
        }
        Action::RemoveCategory(id) => {
            api.empty(Method::DELETE, &format!("/categories/{id}"))
                .await?;
            bus.send(Update::CategoryRemoved(id));
        }
        Action::Conversations => {
            let convs = api.get("/conversations").await?;
            bus.send(Update::Conversations(convs));
            if let Ok(invites) = api.get("/conversations/invites").await {
                bus.send(Update::ConversationInvites(invites));
            }
        }
        Action::AcceptGroupInvite(id) => {
            let conv = api
                .json(
                    Method::POST,
                    &format!("/conversations/{id}/invites/accept"),
                    &serde_json::json!({}),
                )
                .await?;
            bus.send(Update::Conversation {
                conversation: conv,
                open: true,
            });
            bus.send(Update::InviteDeclined(id));
        }
        Action::DeclineGroupInvite(id) => {
            api.empty(
                Method::POST,
                &format!("/conversations/{id}/invites/decline"),
            )
            .await?;
            bus.send(Update::InviteDeclined(id));
        }
        Action::CreateConversation(request) => bus.send(Update::Conversation {
            conversation: api.json(Method::POST, "/conversations", &request).await?,
            open: true,
        }),
        Action::RenameConversation(id, title) => bus.send(Update::Conversation {
            conversation: api
                .json(
                    Method::PATCH,
                    &format!("/conversations/{id}"),
                    &UpdateConversationRequest {
                        title,
                        members_can_invite: None,
                    },
                )
                .await?,
            open: false,
        }),
        Action::SetGroupInvitePermission(id, members_can_invite) => bus.send(Update::Conversation {
            conversation: api
                .json(
                    Method::PATCH,
                    &format!("/conversations/{id}"),
                    &UpdateConversationRequest {
                        title: String::new(),
                        members_can_invite: Some(members_can_invite),
                    },
                )
                .await?,
            open: false,
        }),
        Action::AddMember(id, user_id) => bus.send(Update::Conversation {
            conversation: api
                .json(
                    Method::POST,
                    &format!("/conversations/{id}/members"),
                    &MemberRequest { user_id },
                )
                .await?,
            open: false,
        }),
        Action::TransferGroupOwner(id, user_id) => bus.send(Update::Conversation {
            conversation: api.json(Method::PUT, &format!("/conversations/{id}/owner"), &MemberRequest { user_id }).await?,
            open: false,
        }),
        Action::RemoveMember(id, user_id) => {
            api.empty(
                Method::DELETE,
                &format!("/conversations/{id}/members/{user_id}"),
            )
            .await?;
            bus.send(Update::MemberRemoved(id, user_id));
        }
        Action::History {
            id,
            before,
            query,
            revision,
        } => match api.history(id, before, &query).await {
            Ok(messages) => bus.send(Update::History {
                id,
                before,
                query,
                messages,
                revision,
            }),
            Err(error) => bus.send(Update::HistoryFailed(id, revision, error)),
        },
        Action::Send(id, request) => {
            let pending = match super::outbox::store(api, id, request.clone()).await {
                Ok(pending) => pending,
                Err(error) => {
                    bus.send(Update::SendFailed(
                        request.client_id,
                        format!("Não foi possível guardar a mensagem: {error}"),
                    ));
                    return Ok(());
                }
            };
            match api
                .json::<Message, _>(
                    Method::POST,
                    &format!("/conversations/{}/messages", pending.conversation),
                    &pending.request,
                )
                .await
            {
                Ok(message) => {
                    super::outbox::acknowledged(api, message.client_id).await;
                    bus.send(Update::Message(message));
                }
                Err(error) => bus.send(Update::SendFailed(request.client_id, error)),
            }
        }
        Action::Read(id, message) => match api.read(id, message).await {
            Ok(()) => bus.send(Update::ReadComplete(id, message)),
            Err(error) => bus.send(Update::ReadFailed(id, message, error)),
        },
        Action::EditMessage { conversation, message, request } => {
            let message: Message = api.json(Method::PATCH, &format!("/conversations/{conversation}/messages/{message}"), &request).await?;
            bus.send(Update::Event(ServerEvent::MessageUpdated { message }));
        }
        Action::DeleteMessage { conversation, message } => {
            api.empty(Method::DELETE, &format!("/conversations/{conversation}/messages/{message}")).await?;
            bus.send(Update::Event(ServerEvent::MessageDeleted { conversation_id: conversation, message_id: message }));
        }
        Action::ClearHistory(id) => {
            api.empty(Method::DELETE, &format!("/conversations/{id}/history"))
                .await?;
            bus.send(Update::HistoryCleared(id));
        }
        Action::ExportHistory {
            id,
            title,
            format,
            names,
        } => files::export_history(api, bus, id, title, format, names).await?,
        Action::Upload { conversation, kind } => {
            files::upload_message(api, bus, conversation, kind).await?
        }
        Action::UploadAsset {
            kind,
            name,
            shortcut,
            local,
        } => files::upload_asset(api, bus, kind, name, shortcut, local).await?,
        Action::UseAsset(asset) => files::apply_asset(api, bus, &asset).await?,
        Action::UseAvatarPreset { name, asset } => {
            let bytes = crate::resources::avatar_bytes(&asset)
                .ok_or("Imagem predefinida não encontrada")?;
            let mime = if asset.ends_with(".gif") {
                "image/gif"
            } else {
                "image/png"
            };
            let info = api
                .upload(
                    None,
                    asset.rsplit('/').next().unwrap_or("avatar.png").into(),
                    mime,
                    bytes.to_vec(),
                )
                .await?;
            let avatar: UserAsset = api
                .json(
                    Method::POST,
                    "/assets",
                    &CreateAssetRequest {
                        kind: AssetKind::Avatar,
                        name,
                        shortcut: None,
                        file_id: info.id,
                    },
                )
                .await?;
            bus.send(Update::Asset(avatar.clone()));
            files::apply_asset(api, bus, &avatar).await?;
        }
        Action::SendEmoticon {
            conversation,
            asset,
        } => files::send_emoticon(api, bus, conversation, asset).await?,
        Action::DeleteAsset(id) => {
            api.empty(Method::DELETE, &format!("/assets/{id}")).await?;
            bus.send(Update::AssetRemoved(id));
        }
        Action::LoadImage(source) => {
            if let Err(error) = files::load_image(api, bus, source.clone()).await {
                bus.send(Update::ImageFailed(source, error));
            }
        }
        Action::FileInfo(id) => bus.send(Update::FileInfo(
            api.get(&format!("/files/{id}/info")).await?,
        )),
        Action::Download(id) => files::download(api, bus, id).await?,
        Action::ForwardAttachment { file, conversation, kind } => {
            files::forward_attachment(api, bus, file, conversation, kind).await?;
        }
        Action::LoadVoice { file: id } => {
            let result = async {
                let info: FileInfo = api.get(&format!("/files/{id}/info")).await?;
                if !info.mime.starts_with("audio/") {
                    return Err("O anexo não é um arquivo de áudio.".into());
                }
                let bytes = api.bytes(id).await?;
                tokio::task::spawn_blocking(move || crate::media::audio::decode_wav(&bytes))
                    .await
                    .map_err(|error| error.to_string())?
            }
            .await;
            crate::media::playback::loaded(id, result);
            bus.send(Update::AudioChanged);
        }
        Action::PlayVoice { file, output } => {
            if crate::media::playback::snapshot(file).duration == 0.0 {
                let bytes = api.bytes(file).await?;
                let samples =
                    tokio::task::spawn_blocking(move || crate::media::audio::decode_wav(&bytes))
                        .await
                        .map_err(|e| e.to_string())??;
                crate::media::playback::loaded(file, Ok(samples));
            }
            crate::media::playback::toggle(file, output)?;
            bus.send(Update::AudioChanged);
        }
        Action::Notification {
            file,
            event,
            output,
        } => {
            if !crate::media::audio::AVAILABLE {
                return Ok(());
            }
            let bytes = if let Some(id) = file {
                api.bytes(id).await?
            } else {
                crate::media::notifications::tone(&event)?
            };
            let bus = bus.clone();
            tokio::task::spawn_blocking(move || {
                // A busy named device is common; fall back to the system default.
                let played = crate::media::audio::play_notification(&bytes, &output)
                    .or_else(|_| crate::media::audio::play_notification(&bytes, ""));
                match played {
                    Ok(()) => {
                        let _ = event;
                        bus.send(Update::SoundPlayed);
                    }
                    Err(error) => {
                        bus.send(Update::SoundFailed(format!("Som indisponível ({error})")))
                    }
                }
            });
        }
        Action::Activity(id, request) => {
            let activity = api
                .json(
                    Method::POST,
                    &format!("/conversations/{id}/activities"),
                    &request,
                )
                .await?;
            bus.send(Update::Event(ServerEvent::Activity { activity }));
        }
        Action::UpdateActivity(id, request) => {
            let activity = api
                .json(Method::PATCH, &format!("/activities/{id}"), &request)
                .await?;
            bus.send(Update::Event(ServerEvent::Activity { activity }));
        }
        _ => return Err("Operação interna enviada ao manipulador incorreto.".into()),
    }
    Ok(())
}
