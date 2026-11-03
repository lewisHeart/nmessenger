use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    models::FileRow,
    state::AppState,
};
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::{
    HttpRequest, HttpResponse,
    http::header::{ContentDisposition, DispositionParam, DispositionType},
    web,
};
use futures_util::StreamExt;
use nm_protocol::{FileInfo, MAX_UPLOAD_BYTES};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub async fn authorize(
    _state: &AppState,
    db: &mut PgConnection,
    user: Uuid,
    id: Uuid,
) -> Result<FileRow> {
    let row: FileRow = sqlx::query_as("SELECT * FROM files WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *db)
        .await?
        .ok_or_else(Error::missing)?;
    if let Some(conversation) = row.conversation_id {
        authz::member(db, user, conversation).await?;
        let inaccessible_to_bot: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots b JOIN conversation_members m ON m.user_id=b.user_id WHERE b.user_id=$1 AND m.conversation_id=$2 AND ($3::timestamptz<m.joined_at OR $4<>'application/vnd.nmessenger.encrypted'))")
            .bind(user).bind(conversation).bind(row.created_at).bind(&row.mime).fetch_one(&mut *db).await?;
        if inaccessible_to_bot { return Err(Error::forbidden()); }
        if authz::blocked(db, user, row.owner_id).await? {
            return Err(Error::forbidden());
        }
    } else if row.owner_id != user {
        let announcement: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM announcements a WHERE a.image_file_id=$1 AND (a.enabled OR EXISTS(SELECT 1 FROM app_admins WHERE user_id=$2)))")
            .bind(id).bind(user).fetch_one(&mut *db).await?;
        if announcement {
            return Ok(row);
        }
        let public: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_assets WHERE file_id=$1)")
                .bind(id)
                .fetch_one(&mut *db)
                .await?;
        if !public || authz::blocked(db, user, row.owner_id).await? {
            return Err(Error::forbidden());
        }
    }
    Ok(row)
}

/// An unattached private upload can be attached once, by its owner, atomically.
pub async fn attachment(
    state: &AppState,
    db: &mut PgConnection,
    user: Uuid,
    id: Uuid,
    conversation: Uuid,
) -> Result<FileRow> {
    sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *db)
        .await?;
    let mut row = authorize(state, db, user, id).await?;
    if row.owner_id != user {
        return Err(Error::forbidden());
    }
    if row.mime != "application/vnd.nmessenger.encrypted" {
        let has_bot: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM conversation_members m JOIN bots b ON b.user_id=m.user_id WHERE m.conversation_id=$1)")
            .bind(conversation).fetch_one(&mut *db).await?;
        if has_bot { return Err(Error::bad("Anexos para bots exigem criptografia ponta a ponta")); }
    }
    if row.conversation_id.is_none() {
        let public: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_assets WHERE file_id=$1) OR EXISTS(SELECT 1 FROM announcements WHERE image_file_id=$1)")
                .bind(id)
                .fetch_one(&mut *db)
                .await?;
        if public {
            return Err(Error::bad("Asset público não pode ser convertido em anexo"));
        }
        sqlx::query("UPDATE files SET conversation_id=$2 WHERE id=$1")
            .bind(id)
            .bind(conversation)
            .execute(&mut *db)
            .await?;
        row.conversation_id = Some(conversation);
    }
    if row.conversation_id != Some(conversation) {
        return Err(Error::forbidden());
    }
    Ok(row)
}

fn filename(value: Option<&str>) -> String {
    let leaf = value
        .unwrap_or("arquivo")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("arquivo");
    let clean: String = leaf.chars().filter(|c| !c.is_control()).take(180).collect();
    if clean.trim().is_empty() || clean == "." || clean == ".." {
        "arquivo".into()
    } else {
        clean
    }
}

fn content_type(prefix: &[u8], supplied: &str) -> &'static str {
    if prefix.starts_with(b"NME2EE1\0") {
        "application/vnd.nmessenger.encrypted"
    } else if prefix.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if prefix.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if prefix.starts_with(b"GIF87a") || prefix.starts_with(b"GIF89a") {
        "image/gif"
    } else if prefix.starts_with(b"RIFF") && prefix.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if prefix.starts_with(b"RIFF") && prefix.get(8..12) == Some(b"WAVE") {
        "audio/wav"
    } else if prefix.starts_with(b"OggS") {
        "audio/ogg"
    } else if prefix.starts_with(b"fLaC") {
        "audio/flac"
    } else if prefix.starts_with(b"ID3") || prefix.starts_with(b"\xff\xfb") {
        "audio/mpeg"
    } else if prefix.starts_with(b"%PDF-") {
        "application/pdf"
    } else if prefix.starts_with(b"PK\x03\x04") {
        "application/zip"
    } else if supplied.eq_ignore_ascii_case("audio/l16") {
        "audio/l16"
    } else if supplied.eq_ignore_ascii_case("audio/pcm") {
        "audio/pcm"
    } else if supplied == "text/plain" && std::str::from_utf8(prefix).is_ok() {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

struct UploadGuard(Option<PathBuf>);
impl Drop for UploadGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = tokio::fs::remove_file(path).await;
            });
        }
    }
}

async fn store(
    state: &AppState,
    session: &Session,
    conversation: Option<Uuid>,
    mut multipart: Multipart,
) -> Result<FileInfo> {
    state
        .rate(&format!("upload:{}", session.user_id), 20, 60)
        .await?;
    if let Some(id) = conversation {
        let mut db = state.db.acquire().await?;
        let members = authz::member(&mut db, session.user_id, id).await?;
        authz::peers(
            &mut db,
            session.user_id,
            &members
                .into_iter()
                .filter(|u| *u != session.user_id)
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    let id = Uuid::new_v4();
    let temporary = state.upload_dir.join(format!("{id}.part"));
    let final_path = state.upload_dir.join(id.to_string());
    let mut guard = UploadGuard(Some(temporary.clone()));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut output = options.open(&temporary).await?;
    let mut seen = false;
    let mut name = String::new();
    let mut supplied = String::new();
    let mut size = 0usize;
    let mut hash = Sha256::new();
    let mut prefix = Vec::new();
    while let Some(field) = multipart.next().await {
        let mut field = field.map_err(|_| Error::bad("Multipart inválido"))?;
        if seen || field.name() != Some("file") {
            return Err(Error::bad("Envie somente um campo file"));
        }
        seen = true;
        name = filename(field.content_disposition().and_then(|d| d.get_filename()));
        supplied = field
            .content_type()
            .map(ToString::to_string)
            .unwrap_or_default();
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|_| Error::bad("Upload interrompido"))?;
            size = size.checked_add(chunk.len()).ok_or_else(Error::too_large)?;
            if size > MAX_UPLOAD_BYTES {
                return Err(Error::too_large());
            }
            let needed = 512usize.saturating_sub(prefix.len()).min(chunk.len());
            prefix.extend_from_slice(&chunk[..needed]);
            hash.update(&chunk);
            output.write_all(&chunk).await?;
        }
    }
    if !seen {
        return Err(Error::bad("Campo file obrigatório"));
    }
    output.flush().await?;
    drop(output);
    let mime = content_type(&prefix, &supplied);
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    crate::auth::valid_on(state, &mut *tx, session).await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    let used: i64 =
        sqlx::query_scalar("SELECT COALESCE(sum(size),0)::bigint FROM files WHERE owner_id=$1")
            .bind(session.user_id)
            .fetch_one(&mut *tx)
            .await?;
    if used.saturating_add(size as i64) > state.user_upload_quota {
        return Err(Error::bad("Cota de armazenamento de anexos atingida"));
    }
    if let Some(conversation) = conversation.filter(|_| mime != "application/vnd.nmessenger.encrypted") {
        let enrolled: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys WHERE user_id=$1) OR EXISTS(SELECT 1 FROM conversation_members m JOIN bots b ON b.user_id=m.user_id WHERE m.conversation_id=$2)")
                .bind(session.user_id)
                .bind(conversation)
                .fetch_one(&mut *tx)
                .await?;
        if enrolled {
            return Err(Error::bad(
                "Anexos desta conta exigem criptografia ponta a ponta",
            ));
        }
    }
    if let Some(conversation) = conversation {
        let members = authz::member(&mut tx, session.user_id, conversation).await?;
        authz::peers(
            &mut tx,
            session.user_id,
            &members
                .into_iter()
                .filter(|u| *u != session.user_id)
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    let row: FileRow = sqlx::query_as(
        "INSERT INTO files(id,owner_id,conversation_id,filename,mime,size,sha256)
        VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING *",
    )
    .bind(id)
    .bind(session.user_id)
    .bind(conversation)
    .bind(name)
    .bind(mime)
    .bind(size as i64)
    .bind(hex::encode(hash.finalize()))
    .fetch_one(&mut *tx)
    .await?;
    tokio::fs::rename(&temporary, &final_path).await?;
    // Disarm before COMMIT: if the connection drops after a successful commit,
    // removing the blob would destroy a durable file. A failed commit can leave
    // an unreferenced UUID blob, but never a committed row pointing at a deleted one.
    guard.0 = None;
    tx.commit().await?;
    Ok(row.model())
}

async fn upload(
    state: web::Data<AppState>,
    session: Session,
    multipart: Multipart,
) -> Result<web::Json<FileInfo>> {
    Ok(web::Json(store(&state, &session, None, multipart).await?))
}
async fn upload_conversation(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    multipart: Multipart,
) -> Result<web::Json<FileInfo>> {
    Ok(web::Json(
        store(&state, &session, Some(*id), multipart).await?,
    ))
}
async fn info(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<web::Json<FileInfo>> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let info = authorize(&state, &mut tx, session.user_id, *id)
        .await?
        .model();
    tx.commit().await?;
    Ok(web::Json(info))
}
async fn download(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let info = authorize(&state, &mut tx, session.user_id, *id).await?;
    let path = state.upload_dir.join(info.id.to_string());
    let file = tokio::task::spawn_blocking(move || NamedFile::open(path))
        .await
        .map_err(|_| Error::internal())?
        .map_err(|_| Error::missing())?;
    tx.commit().await?;
    let mut response = file
        .set_content_type(info.mime.parse().map_err(|_| Error::internal())?)
        .set_content_disposition(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(info.filename)],
        })
        .into_response(&req);
    response.headers_mut().insert(
        actix_web::http::header::CACHE_CONTROL,
        actix_web::http::header::HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        actix_web::http::header::X_CONTENT_TYPE_OPTIONS,
        actix_web::http::header::HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/files", web::post().to(upload))
        .route(
            "/conversations/{id}/files",
            web::post().to(upload_conversation),
        )
        .route("/files/{id}/info", web::get().to(info))
        .route("/files/{id}", web::get().to(download));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_metadata_and_active_content_is_not_sniffed_as_image() {
        assert_eq!(filename(Some("../../etc/passwd")), "passwd");
        assert_eq!(filename(Some("C:\\fakepath\\x.png")), "x.png");
        assert_eq!(filename(Some("\r\n")), "arquivo");
        assert_eq!(
            content_type(b"<svg onload='evil()'>", "image/svg+xml"),
            "application/octet-stream"
        );
    }
}
