use nm_protocol::*;
use reqwest::{Client, Method, Response, Url};
use serde::{Serialize, de::DeserializeOwned};
use std::{sync::Arc, time::Duration};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone)]
pub struct Api {
    pub client: Client,
    pub base: String,
    pub token: Arc<Zeroizing<String>>,
    pub user_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub crypto: Option<Arc<crate::encryption::Crypto>>,
}

pub fn base(server: &str) -> Result<String, String> {
    let url =
        Url::parse(server.trim()).map_err(|e| format!("Endereço do servidor inválido: {e}"))?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use um endereço HTTP(S), sem credenciais, parâmetros ou fragmentos.".into());
    }
    let base = url.as_str().trim_end_matches('/');
    Ok(if base.ends_with("/v1") {
        base.into()
    } else {
        format!("{base}/v1")
    })
}

pub(crate) fn client() -> Result<Client, String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent("NMessenger/0.1")
        .redirect(reqwest::redirect::Policy::none())
        .tls_backend_preconfigured(tls_config)
        .build()
        .map_err(|e| e.to_string())
}

pub async fn checked(mut response: Response) -> Result<Response, String> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| format!("HTTP {status}"))?
    {
        if bytes.len() + chunk.len() > 8192 {
            return Err(format!("HTTP {status}: resposta de erro excede o limite"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&bytes);
    if let Ok(error) = serde_json::from_str::<ApiError>(&body) {
        Err(format!(
            "{} · {} (HTTP {status})",
            error.code, error.message
        ))
    } else {
        Err(format!("{status}: {body}"))
    }
}

pub(crate) async fn bounded_json<T: DeserializeOwned>(mut response: Response) -> Result<T, String> {
    const LIMIT: usize = 16 * 1024 * 1024;
    if response.content_length().is_some_and(|size| size > LIMIT as u64) {
        return Err("Resposta do servidor excede o limite".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if chunk.len() > LIMIT.saturating_sub(bytes.len()) {
            return Err("Resposta do servidor excede o limite".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

impl Api {
    pub fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(self.token.as_str())
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let response = checked(
            self.request(Method::GET, path)
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        self.decode_response(response).await
    }

    pub async fn json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let mut body = serde_json::to_value(body).map_err(|error| error.to_string())?;
        if self.crypto.is_some() && ((method == Method::POST && path.ends_with("/messages"))
            || (method == Method::PATCH && path.contains("/messages/"))) {
            let id = path
                .strip_prefix("/conversations/")
                .and_then(|path| path.split_once("/messages").map(|(id, _)| id))
                .ok_or("Conversa inválida")?
                .parse()
                .map_err(|_| "Conversa inválida")?;
            let mut request: SendMessageRequest =
                serde_json::from_value(body).map_err(|error| error.to_string())?;
            self.seal_message(id, &mut request).await?;
            body = serde_json::to_value(request).map_err(|error| error.to_string())?;
        }
        let response = checked(
            self.request(method, path)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        self.decode_response(response).await
    }

    pub async fn empty(&self, method: Method, path: &str) -> Result<(), String> {
        checked(
            self.request(method, path)
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        Ok(())
    }

    pub async fn read(&self, id: uuid::Uuid, message_id: Option<uuid::Uuid>) -> Result<(), String> {
        checked(
            self.request(Method::POST, &format!("/conversations/{id}/read"))
                .json(&ReadRequest { message_id })
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        Ok(())
    }

    pub async fn history(
        &self,
        id: uuid::Uuid,
        before: Option<uuid::Uuid>,
        query: &str,
    ) -> Result<Vec<Message>, String> {
        if self.crypto.is_none() || query.is_empty() {
            return self.history_page(id, before, query).await;
        }
        let mut cursor = before;
        let mut found = Vec::new();
        let query = query.to_lowercase();
        for _ in 0..200 {
            // Search only authenticated plaintext in the client; never send search terms to the server.
            let page = self.history_page(id, cursor, "").await?;
            let next = page.first().map(|message| message.id);
            let done = page.len() < 50 || next == cursor;
            found.extend(
                page.into_iter()
                    .filter(|message| message.body.to_lowercase().contains(&query)),
            );
            if found.len() >= 50 || done {
                found.sort_by_key(|message| (message.created_at, message.id));
                if found.len() > 50 {
                    found.drain(..found.len() - 50);
                }
                return Ok(found);
            }
            cursor = next;
        }
        Err("A busca atingiu 10 mil mensagens. Refine a busca ou consulte um período menor.".into())
    }

    async fn history_page(
        &self,
        id: uuid::Uuid,
        before: Option<uuid::Uuid>,
        query: &str,
    ) -> Result<Vec<Message>, String> {
        let mut params = vec![("limit", "50".to_owned())];
        if let Some(before) = before {
            params.push(("before", before.to_string()));
        }
        if !query.is_empty() {
            params.push(("q", query.to_owned()));
        }
        let response = checked(
            self.request(Method::GET, &format!("/conversations/{id}/messages"))
                .query(&params)
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        self.decode_response(response).await
    }

    pub async fn bytes(&self, id: uuid::Uuid) -> Result<Vec<u8>, String> {
        let mut response = checked(
            self.request(Method::GET, &format!("/files/{id}"))
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        if response.content_length().unwrap_or(0) > MAX_UPLOAD_BYTES as u64 {
            return Err("Arquivo excede 25 MiB.".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if bytes.len() + chunk.len() > MAX_UPLOAD_BYTES {
                return Err("Arquivo excede 25 MiB.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let protected = self.crypto.as_ref().is_some_and(|crypto| {
            crypto
                .files
                .lock()
                .is_ok_and(|files| files.contains_key(&id))
        });
        if protected || bytes.starts_with(super::crypto_files::MAGIC) {
            return self.open_file(id, &bytes).await;
        }
        Ok(bytes)
    }

    pub async fn upload(
        &self,
        conversation: Option<uuid::Uuid>,
        name: String,
        mime: &str,
        bytes: Vec<u8>,
    ) -> Result<FileInfo, String> {
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err("O limite de envio é 25 MiB.".into());
        }
        let mut name = name;
        let mut mime = mime.to_owned();
        let mut bytes = bytes;
        let protected = if let Some(conversation) = conversation.filter(|_| self.crypto.is_some()) {
            let original = (name.clone(), mime.clone(), bytes.len());
            let encrypted = self.seal_file(conversation, &name, &mime, &bytes).await?;
            bytes.zeroize();
            name = "anexo.nme".into();
            mime = "application/vnd.nmessenger.encrypted".into();
            bytes = encrypted;
            Some((conversation, original, super::crypto_files::digest(&bytes)))
        } else {
            None
        };
        let path = conversation
            .map(|id| format!("/conversations/{id}/files"))
            .unwrap_or_else(|| "/files".into());
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(name)
            .mime_str(&mime)
            .map_err(|e| e.to_string())?;
        let form = reqwest::multipart::Form::new().part("file", part);
        let response = checked(
            self.request(Method::POST, &path)
                .multipart(form)
                .send()
                .await
                .map_err(|e| e.to_string())?,
        )
        .await?;
        let mut info: FileInfo = bounded_json(response).await?;
        if let Some((conversation, (name, mime, size), hash)) = protected {
            self.remember_upload(conversation, &mut info, hash, name, mime, size)
                .await?;
        }
        Ok(info)
    }
}

pub async fn authenticate(
    server: String,
    email: String,
    password: String,
    device_name: String,
    name: Option<String>,
    presence: Presence,
    identity_password: Option<String>,
) -> Result<(Api, Bootstrap), String> {
    let password = Zeroizing::new(password);
    let identity_password = identity_password.map(Zeroizing::new);
    let client = client()?;
    let base = base(&server)?;
    let secure = if name.is_none() {
        super::opaque::login(&client, &base, &email, &password, &device_name).await?
    } else { None };
    let (auth, mut export_key) = if let Some((auth, key)) = secure {
        (auth, Some(key))
    } else {
    let response = if let Some(display_name) = name {
        let mut request = RegisterRequest {
            email,
            password: password.to_string(),
            display_name,
            device_name,
        };
        let result = client
            .post(format!("{base}/auth/register"))
            .json(&request)
            .send()
            .await;
        request.password.zeroize();
        result
    } else {
        let mut request = LoginRequest {
            email,
            password: password.to_string(),
            device_name,
        };
        let result = client
            .post(format!("{base}/auth/login"))
            .json(&request)
            .send()
            .await;
        request.password.zeroize();
        result
    }
    .map_err(|e| {
        if e.is_connect() {
            "Não foi possível conectar ao servidor. Verifique o endereço e sua conexão.".to_string()
        } else if e.is_timeout() {
            "O servidor demorou muito para responder. Tente novamente.".to_string()
        } else {
            "Falha de conexão com o servidor. Verifique sua internet.".to_string()
        }
    })?;
    let response = checked(response)
        .await
        .map_err(|e| {
            if e.contains("unauthorized")
                || e.contains("401")
                || e.contains("inválida")
                || e.contains("expirada")
            {
                "E-mail ou senha incorretos. Verifique suas credenciais.".to_string()
            } else if let Some((_, rest)) = e.split_once(" · ") {
                if let Some((msg, _)) = rest.split_once(" (HTTP") {
                    msg.to_string()
                } else {
                    rest.to_string()
                }
            } else {
                e
            }
        })?;
    let auth: AuthResponse = bounded_json(response).await?;
    (auth, None)
    };
    let api = Api {
        client,
        base,
        token: Arc::new(Zeroizing::new(auth.token)),
        user_id: auth.user.id,
        session_id: auth.session_id,
        crypto: None,
    };
    let mut api = api;
    let result = async {
        let crypto = match crate::encryption::Crypto::load(&api.base, api.user_id, &password).await {
            Ok(crypto) => crypto,
            Err(error) => if let Some(old_password) = identity_password.as_deref() {
                let account = crate::secure::account(&api.base, &api.user_id.to_string());
                crate::identity_vault::rewrap(&account, old_password, &password).await?;
                crate::encryption::Crypto::load(&api.base, api.user_id, &password).await?
            } else { return Err(error); },
        };
        let crypto = Arc::new(crypto);
        api.crypto = Some(crypto.clone());
        let previous_key = if let Some(old_password) = identity_password.as_deref() {
            super::opaque::recover(&api, old_password).await?
        } else { None };
        if export_key.is_none() { export_key = super::opaque::enroll(&api, &password).await?; }
        api.json::<EncryptionKey, _>(Method::POST, "/encryption/keys", &crypto.registration()).await?;
        if let Some(key) = &export_key {
            api.sync_cloud_history_with_previous(key, previous_key.as_deref().map(|value| value.as_slice())).await?;
            crypto.set_history_export_key(key)?;
            if previous_key.is_some() { api.empty(Method::POST, "/auth/opaque/recovery/complete").await?; }
        }
        api.json::<Profile, _>(Method::PATCH, "/me", &UpdateProfileRequest {
            presence: Some(presence),
            ..Default::default()
        }).await?;
        api.get("/bootstrap").await
    }.await;
    let bootstrap = match result {
        Ok(bootstrap) => bootstrap,
        Err(error) => {
            let _ = api.empty(Method::POST, "/auth/logout").await;
            return Err(error);
        }
    };
    Ok((api, bootstrap))
}

pub async fn health(server: &str) -> Result<String, String> {
    let response = checked(
        client()?
            .get(format!("{}/health", base(server)?))
            .send()
            .await
            .map_err(|e| e.to_string())?,
    )
    .await?;
    let value: serde_json::Value = bounded_json(response).await?;
    Ok(format!("Diagnóstico do servidor: {value}"))
}
