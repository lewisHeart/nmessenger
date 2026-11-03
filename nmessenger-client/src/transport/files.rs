use super::{Api, Bus, Update};
use crate::{media::images::ImageSource, storage};
use nm_protocol::*;
use reqwest::Method;
use std::path::PathBuf;
use uuid::Uuid;

async fn pick(filter: &'static str) -> Result<Option<PathBuf>, String> {
    crate::platform::pick_file(filter).await
}

pub async fn pick_avatar(bus: &Bus) -> Result<(), String> {
    if let Some(path) = pick("image").await? {
        bus.send(Update::AvatarFilePicked(path));
    }
    Ok(())
}

pub async fn pick_receive_directory(bus: &Bus, current: Option<String>) -> Result<(), String> {
    if let Some(path) = crate::platform::pick_folder(current).await? {
        bus.send(Update::ReceiveDirectoryPicked(path));
    }
    Ok(())
}

pub async fn upload_announcement_image(api: &Api, bus: &Bus) -> Result<(), String> {
    let Some(path) = pick("image").await? else {
        bus.send(Update::AnnouncementImage(None));
        return Ok(());
    };
    let bytes = read(&path).await?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let info = api.upload(None, filename, mime(&path), bytes).await?;
    bus.send(Update::AnnouncementImage(Some(info)));
    Ok(())
}

pub async fn upload_bio_image(api: &Api, bus: &Bus) -> Result<(), String> {
    let Some(path) = pick("image").await? else {
        bus.send(Update::BioImage(None));
        return Ok(());
    };
    let bytes = read(&path).await?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let info = api.upload(None, filename.clone(), mime(&path), bytes).await?;
    let _ = api
        .json::<UserAsset, _>(
            reqwest::Method::POST,
            "/assets",
            &CreateAssetRequest {
                kind: AssetKind::Background,
                name: filename.clone(),
                shortcut: None,
                file_id: info.id,
            },
        )
        .await;
    bus.send(Update::BioImage(Some(info)));
    Ok(())
}

async fn destination(name: String, directory: Option<String>) -> Result<Option<PathBuf>, String> {
    // The server filename is a suggestion, never a filesystem path.
    let name = PathBuf::from(name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    crate::platform::save_file(name, directory).await
}

async fn read(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let metadata = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
    if metadata.len() > MAX_UPLOAD_BYTES as u64 {
        return Err("O limite por arquivo é 25 MiB.".into());
    }
    tokio::fs::read(path)
        .await
        .map_err(|e| format!("Leitura do arquivo: {e}"))
}

pub async fn export_identity_backup(api: &Api, bus: &Bus) -> Result<(), String> {
    let crypto = api.crypto.as_ref().ok_or("Criptografia não iniciada")?;
    let Some(path) = crate::platform::save_file("nmessenger-identity.nmkey".into(), None).await? else {
        bus.send(Update::HistoryBackupFinished { imported: false, result: Ok(false) });
        return Ok(());
    };
    let bytes = crypto.export_identity_backup().await?;
    tokio::fs::write(&path, bytes).await.map_err(|error| error.to_string())?;
    crate::platform::finish_save(&path).await?;
    bus.send(Update::HistoryBackupFinished { imported: false, result: Ok(true) });
    Ok(())
}

pub async fn import_identity_backup(api: &Api, bus: &Bus, password: String) -> Result<(), String> {
    let password = zeroize::Zeroizing::new(password);
    let Some(path) = pick("all").await? else {
        bus.send(Update::HistoryBackupFinished { imported: true, result: Ok(false) });
        return Ok(());
    };
    let bytes = read(&path).await?;
    if bytes.len() > 32768 { return Err("Arquivo de identidade inválido".into()); }
    api.crypto.as_ref().ok_or("Criptografia não iniciada")?
        .import_identity_backup(bytes, password.to_string()).await?;
    bus.send(Update::HistoryBackupFinished { imported: true, result: Ok(true) });
    Ok(())
}

fn mime(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "wav" => "audio/wav",
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

pub async fn upload_message(
    api: &Api,
    bus: &Bus,
    conversation: Uuid,
    kind: MessageKind,
) -> Result<(), String> {
    let filter = match kind {
        MessageKind::Photo => "image",
        MessageKind::Voice => "audio",
        _ => "all",
    };
    let Some(path) = pick(filter).await? else {
        return Ok(());
    };
    let bytes = read(&path).await?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    bus.send(Update::Notice(format!("Enviando {filename}…")));
    let info = api
        .upload(Some(conversation), filename.clone(), mime(&path), bytes)
        .await?;
    bus.send(Update::FileInfo(info.clone()));
    let request = SendMessageRequest {
        client_id: Uuid::new_v4(),
        kind,
        body: filename,
        style: MessageStyle::default(),
        attachment_id: Some(info.id),
    };
    bus.send(Update::PreparedMessage {
        conversation,
        request,
    });
    bus.send(Update::Notice(
        "Arquivo carregado; enviando mensagem…".into(),
    ));
    Ok(())
}

pub async fn upload_asset(
    api: &Api,
    bus: &Bus,
    kind: AssetKind,
    name: String,
    shortcut: Option<String>,
    local: Option<PathBuf>,
) -> Result<(), String> {
    let path = match local {
        Some(path) => path,
        None => {
            let Some(path) = pick(if kind == AssetKind::Sound {
                "audio"
            } else {
                "image"
            })
            .await?
            else {
                if kind == AssetKind::Emoticon {
                    bus.send(Update::EmoticonUploadFinished(Ok(false)));
                }
                return Ok(());
            };
            path
        }
    };
    let bytes = read(&path).await?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let info = api
        .upload(None, filename.clone(), mime(&path), bytes)
        .await?;
    let asset: UserAsset = api
        .json(
            Method::POST,
            "/assets",
            &CreateAssetRequest {
                kind,
                name: if name.trim().is_empty() {
                    filename
                } else {
                    name
                },
                shortcut,
                file_id: info.id,
            },
        )
        .await?;
    bus.send(Update::Asset(asset.clone()));
    apply_asset(api, bus, &asset).await?;
    if kind == AssetKind::Emoticon {
        bus.send(Update::EmoticonUploadFinished(Ok(true)));
    } else {
        bus.send(Update::Notice("Personalização salva no servidor.".into()));
    }
    Ok(())
}

pub async fn apply_asset(api: &Api, bus: &Bus, asset: &UserAsset) -> Result<(), String> {
    if !matches!(asset.kind, AssetKind::Avatar | AssetKind::Background) {
        return Ok(());
    }
    let bootstrap: Bootstrap = api.get("/bootstrap").await?;
    let mut scene = crate::media::scene::Scene::parse(&bootstrap.me.scene);
    let mut request = UpdateProfileRequest::default();
    if asset.kind == AssetKind::Avatar {
        request.avatar_id = Some(asset.id);
        scene.avatar_file = Some(asset.file_id);
    } else {
        scene.background_file = Some(asset.file_id);
    }
    request.scene = Some(scene.encode());
    bus.send(Update::Profile(
        api.json(Method::PATCH, "/me", &request).await?,
    ));
    Ok(())
}

pub async fn send_emoticon(
    api: &Api,
    bus: &Bus,
    conversation: Uuid,
    asset: UserAsset,
) -> Result<(), String> {
    let metadata: FileInfo = api.get(&format!("/files/{}/info", asset.file_id)).await?;
    let bytes = api.bytes(asset.file_id).await?;
    // Public assets cannot be reclassified as private conversation attachments.
    let file = api
        .upload(Some(conversation), metadata.filename, &metadata.mime, bytes)
        .await?;
    bus.send(Update::PreparedMessage {
        conversation,
        request: SendMessageRequest {
            client_id: Uuid::new_v4(),
            kind: MessageKind::Text,
            body: asset.shortcut.unwrap_or(asset.name),
            style: MessageStyle::default(),
            attachment_id: Some(file.id),
        },
    });
    bus.send(Update::FileInfo(file));
    Ok(())
}

pub async fn forward_attachment(api: &Api, bus: &Bus, file: Uuid, conversation: Uuid, kind: MessageKind) -> Result<(), String> {
    if !matches!(kind, MessageKind::Photo | MessageKind::Voice | MessageKind::File) {
        return Err("Tipo de anexo não suportado".into());
    }
    let metadata: FileInfo = api.get(&format!("/files/{file}/info")).await?;
    let bytes = api.bytes(file).await?;
    let uploaded = api.upload(Some(conversation), metadata.filename.clone(), &metadata.mime, bytes).await?;
    bus.send(Update::FileInfo(uploaded.clone()));
    bus.send(Update::PreparedMessage {
        conversation,
        request: SendMessageRequest {
            client_id: Uuid::new_v4(), kind, body: metadata.filename,
            style: MessageStyle::default(), attachment_id: Some(uploaded.id),
        },
    });
    Ok(())
}

pub async fn load_image(api: &Api, bus: &Bus, source: ImageSource) -> Result<(), String> {
    static IMAGE_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
    let _permit = IMAGE_JOBS
        .acquire()
        .await
        .map_err(|_| "Decodificação de imagens indisponível")?;
    let bytes = match &source {
        ImageSource::File(id) => api.bytes(*id).await?,
        ImageSource::Local(path) => read(path).await?,
        ImageSource::Bundled(name) => crate::resources::bytes(name)
            .ok_or("Imagem incorporada não encontrada")?
            .to_vec(),
    };
    let maximum = matches!(source, ImageSource::Bundled(_)).then_some(48);
    let frames =
        tokio::task::spawn_blocking(move || crate::media::images::decode_sized(&bytes, maximum))
            .await
            .map_err(|e| e.to_string())??;
    bus.send(Update::Image { source, frames });
    Ok(())
}

pub async fn download(api: &Api, bus: &Bus, id: Uuid) -> Result<(), String> {
    let info: FileInfo = api.get(&format!("/files/{id}/info")).await?;
    let directory = match api.get::<UserSettings>("/settings").await {
        Ok(settings) => Some(settings.receive_directory),
        Err(e) => {
            bus.send(Update::Error(format!("Preferência de pasta: {e}")));
            None
        }
    };
    let Some(path) = destination(info.filename.clone(), directory).await? else {
        return Ok(());
    };
    let bytes = api.bytes(id).await?;
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|e| format!("Não foi possível salvar: {e}"))?;
    crate::platform::finish_save(&path).await?;
    bus.send(Update::FileInfo(info));
    bus.send(Update::Notice(format!("Salvo em {}", path.display())));
    Ok(())
}

pub async fn import_contacts(api: &Api, bus: &Bus) -> Result<(), String> {
    let Some(path) = pick("contact").await? else {
        return Ok(());
    };
    let bytes = read(&path).await?;
    let emails = storage::contact_emails(&bytes)?;
    let contacts: Vec<Contact> = api
        .json(
            Method::POST,
            "/contacts/import",
            &ImportContactsRequest { emails },
        )
        .await?;
    bus.send(Update::Notice(format!(
        "{} contato(s) importado(s).",
        contacts.len()
    )));
    bus.send(Update::Contacts(api.get("/contacts").await?));
    Ok(())
}

pub async fn export_contacts(api: &Api, bus: &Bus, format: &str) -> Result<(), String> {
    let contacts: Vec<Contact> = api.get("/contacts/export").await?;
    let Some(path) = destination(format!("contatos.{format}"), None).await? else {
        return Ok(());
    };
    let bytes = if format == "json" {
        serde_json::to_vec_pretty(&contacts).map_err(|e| e.to_string())?
    } else {
        storage::contacts_xml(&contacts).into_bytes()
    };
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|e| e.to_string())?;
    crate::platform::finish_save(&path).await?;
    bus.send(Update::Notice("Lista de contatos exportada.".into()));
    Ok(())
}

pub async fn export_history(
    api: &Api,
    bus: &Bus,
    id: Uuid,
    title: String,
    format: String,
    names: Vec<(Uuid, String)>,
) -> Result<(), String> {
    let Some(path) = destination(format!("conversa.{format}"), None).await? else {
        return Ok(());
    };
    bus.send(Update::Notice(
        "Buscando todas as páginas do histórico…".into(),
    ));
    let mut messages = Vec::new();
    let mut before = None;
    loop {
        let page = api.history(id, before, "").await?;
        let next = page.first().map(|m| m.id);
        let done = page.len() < 50 || next == before;
        messages.extend(page);
        if done {
            break;
        }
        before = next;
    }
    messages.sort_by_key(|m| (m.created_at, m.id));
    messages.dedup_by_key(|m| m.id);
    let output = storage::history_export(&messages, &title, &format, &names);
    tokio::fs::write(&path, output)
        .await
        .map_err(|e| e.to_string())?;
    crate::platform::finish_save(&path).await?;
    bus.send(Update::Notice(format!(
        "Histórico exportado: {} mensagens.",
        messages.len()
    )));
    Ok(())
}
