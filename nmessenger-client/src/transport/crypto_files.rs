use super::Api;
use crate::encryption::{Binding, ProtectedAttachment};
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MAGIC: &[u8; 8] = b"NME2EE1\0";

#[derive(Serialize, Deserialize)]
struct Header {
    binding: Binding,
    envelope: EncryptedMessage,
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    filename: String,
    mime: String,
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Api {
    pub(super) async fn seal_file(
        &self,
        conversation: Uuid,
        name: &str,
        mime: &str,
        clear: &[u8],
    ) -> Result<Vec<u8>, String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia indisponível")?;
        let keys = self.encryption_keys(conversation, true).await?;
        let metadata = serde_json::to_vec(&Metadata {
            filename: name.into(),
            mime: mime.into(),
        })
        .map_err(|error| error.to_string())?;
        if metadata.len() > 4096 || clear.len() > MAX_UPLOAD_BYTES - 64 * 1024 {
            return Err("O limite de anexos protegidos é 25 MiB menos 64 KiB de metadados.".into());
        }
        let content = zeroize::Zeroizing::new(
            [
                (metadata.len() as u32).to_be_bytes().as_slice(),
                &metadata,
                clear,
            ]
            .concat(),
        );
        let binding = Binding {
            purpose: "file".into(),
            conversation,
            sender: self.user_id,
            object: Uuid::new_v4(),
            kind: "attachment".into(),
            sequence: None,
            attachment: None,
        };
        let mut envelope = crypto.seal(&content, &binding, &keys)?;
        let ciphertext = STANDARD
            .decode(&envelope.ciphertext)
            .map_err(|_| "Anexo inválido")?;
        envelope.ciphertext.clear();
        let header =
            serde_json::to_vec(&Header { binding, envelope }).map_err(|error| error.to_string())?;
        if header.len() > 60 * 1024 {
            return Err("Número de dispositivos excede o limite do anexo".into());
        }
        Ok([
            MAGIC.as_slice(),
            (header.len() as u32).to_be_bytes().as_slice(),
            &header,
            &ciphertext,
        ]
        .concat())
    }

    pub(super) async fn remember_upload(
        &self,
        conversation: Uuid,
        info: &mut FileInfo,
        hash: String,
        filename: String,
        mime: String,
        size: usize,
    ) -> Result<(), String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia indisponível")?;
        let meta = ProtectedAttachment {
            file_id: info.id,
            conversation,
            owner: self.user_id,
            digest: hash,
            filename: filename.clone(),
            mime: mime.clone(),
            size: size as i64,
        };
        if meta.digest != info.sha256 {
            return Err("O servidor retornou um anexo diferente do enviado".into());
        }
        crypto
            .files
            .lock()
            .map_err(|_| "Estado de arquivos indisponível")?
            .insert(info.id, meta);
        crypto.save_files().await?;
        info.filename = filename;
        info.mime = mime;
        info.size = size as i64;
        Ok(())
    }

    pub(super) async fn open_file(&self, id: Uuid, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let crypto = self.crypto.as_ref().ok_or("Criptografia indisponível")?;
        let meta = crypto
            .files
            .lock()
            .map_err(|_| "Estado de arquivos indisponível")?
            .get(&id)
            .cloned()
            .ok_or("Carregue a mensagem original para verificar este anexo protegido")?;
        if digest(bytes) != meta.digest {
            return Err("O anexo foi substituído ou alterado".into());
        }
        if bytes.len() < 12 || !bytes.starts_with(MAGIC) {
            return Err("Anexo protegido inválido".into());
        }
        let length = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if length > 60 * 1024 || bytes.len() < 12 + length {
            return Err("Cabeçalho de anexo inválido".into());
        }
        let mut header: Header = serde_json::from_slice(&bytes[12..12 + length])
            .map_err(|_| "Cabeçalho de anexo inválido")?;
        if header.binding.purpose != "file"
            || header.binding.conversation != meta.conversation
            || header.binding.sender != meta.owner
        {
            return Err("Anexo pertence a outro remetente ou conversa".into());
        }
        header.envelope.ciphertext = STANDARD.encode(&bytes[12 + length..]);
        let keys = self.encryption_keys(meta.conversation, false).await?;
        let content = crypto.open(&header.envelope, &header.binding, &keys)?;
        if content.len() < 4 {
            return Err("Anexo protegido inválido".into());
        }
        let length = u32::from_be_bytes(content[..4].try_into().unwrap()) as usize;
        if length > 4096 || content.len() < 4 + length {
            return Err("Metadados de anexo inválidos".into());
        }
        let metadata: Metadata = serde_json::from_slice(&content[4..4 + length])
            .map_err(|_| "Metadados de anexo inválidos")?;
        if metadata.filename != meta.filename
            || metadata.mime != meta.mime
            || content.len() - 4 - length != meta.size as usize
        {
            return Err("Metadados de anexo foram alterados".into());
        }
        Ok(content[4 + length..].to_vec())
    }
}
