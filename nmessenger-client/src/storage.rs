use nm_protocol::{Contact, Message, Presence};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub server: String,
    pub custom_server: bool,
    pub email: String,
    pub device: String,
    pub presence: Presence,
    pub remember: bool,
    pub auto_login: bool,
    pub content_directory: String,
    pub camera_device: String,
    pub audio_input: String,
    pub audio_output: String,
    pub system_titlebars: bool,
    pub language: crate::i18n::LanguagePreference,
    pub ui_scale: f32,
    pub section_sizes: std::collections::HashMap<String, String>,
    pub window_sizes: std::collections::HashMap<String, [f32; 2]>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            server: crate::env::default_server_url(),
            custom_server: false,
            email: String::new(),
            device: "Meu computador".into(),
            presence: Presence::Available,
            remember: true,
            auto_login: false,
            content_directory: String::new(),
            camera_device: "/dev/video0".into(),
            audio_input: String::new(),
            audio_output: String::new(),
            system_titlebars: false,
            language: crate::i18n::LanguagePreference::System,
            ui_scale: 1.25,
            section_sizes: std::collections::HashMap::new(),
            window_sizes: std::collections::HashMap::new(),
        }
    }
}

#[cfg(not(test))]
pub fn directory() -> PathBuf {
    std::env::var_os("NM_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(crate::platform::data_directory)
        .unwrap_or_else(|| PathBuf::from(".nmessenger"))
}

#[cfg(test)]
pub fn directory() -> PathBuf {
    static DIRECTORY: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-config")
            .join(uuid::Uuid::new_v4().to_string())
    });
    DIRECTORY.clone()
}

pub async fn load_preferences() -> Result<Preferences, String> {
    match tokio::fs::read(directory().join("preferences.json")).await {
        Ok(bytes) => {
            let mut prefs: Preferences =
                serde_json::from_slice(&bytes).map_err(|e| format!("Preferências: {e}"))?;
            if !prefs.custom_server {
                prefs.server = crate::env::default_server_url();
            }
            Ok(prefs)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(e) => Err(format!("Não foi possível ler as preferências: {e}")),
    }
}

pub async fn save_preferences(prefs: &Preferences) -> Result<(), String> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _lock = LOCK.lock().await;
    let dir = directory();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    let mut safe = prefs.clone();
    if !safe.remember {
        safe.email.clear();
        safe.auto_login = false;
    }
    let bytes = serde_json::to_vec_pretty(&safe).map_err(|e| e.to_string())?;
    crate::secure::write_private(&dir.join("preferences.json"), &bytes).await
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub server: String,
    pub email: String,
    #[serde(default, skip_serializing)]
    pub password: String,
}

pub async fn save_saved_session(server: &str, email: &str, password: &str) -> Result<(), String> {
    let dir = directory();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;
    let data = SavedSession {
        server: server.to_string(),
        email: email.to_string(),
        password: String::new(),
    };
    crate::secure::credential(
        crate::secure::account(server, email),
        Some(password.into()),
        false,
    )
    .await?;
    let bytes = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
    crate::secure::write_private(&dir.join("saved_session.json"), &bytes).await
}

pub async fn load_saved_session() -> Result<Option<SavedSession>, String> {
    let dir = directory();
    match tokio::fs::read(dir.join("saved_session.json")).await {
        Ok(bytes) => {
            let bytes = zeroize::Zeroizing::new(bytes);
            let mut saved: SavedSession =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if !saved.password.is_empty() {
                // Remove legacy plaintext immediately, even if the OS vault is unavailable.
                let legacy = zeroize::Zeroizing::new(std::mem::take(&mut saved.password));
                tokio::fs::remove_file(dir.join("saved_session.json"))
                    .await
                    .map_err(|e| e.to_string())?;
                save_saved_session(&saved.server, &saved.email, &legacy).await?;
            }
            saved.password = crate::secure::credential(
                crate::secure::account(&saved.server, &saved.email),
                None,
                false,
            )
            .await?
            .unwrap_or_default();
            Ok((!saved.password.is_empty()).then_some(saved))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn clear_saved_session() -> Result<(), String> {
    let dir = directory();
    let path = dir.join("saved_session.json");
    let mut result = Ok(());
    if let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(saved) = serde_json::from_slice::<SavedSession>(&bytes)
    {
        result = crate::secure::credential(
            crate::secure::account(&saved.server, &saved.email),
            None,
            true,
        )
        .await
        .map(|_| ());
    }
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .map_err(|error| error.to_string())?;
    }
    result
}

pub fn cached_avatar_path(server: &str, email: &str) -> PathBuf {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(server.as_bytes());
    hash.update([0]);
    hash.update(email.trim().to_ascii_lowercase().as_bytes());
    directory().join("avatars").join(format!("avatar_{}.png", URL_SAFE_NO_PAD.encode(hash.finalize())))
}

pub async fn save_cached_avatar(server: &str, email: &str, bytes: &[u8]) -> Result<(), String> {
    let path = cached_avatar_path(server, email);
    crate::secure::write_private(&path, bytes).await
}

pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn contact_emails(bytes: &[u8]) -> Result<Vec<String>, String> {
    if let Ok(contacts) = serde_json::from_slice::<Vec<Contact>>(bytes) {
        return Ok(contacts.into_iter().map(|c| c.user.email).collect());
    }
    if let Ok(emails) = serde_json::from_slice::<Vec<String>>(bytes) {
        return Ok(emails);
    }
    let utf16;
    let bytes = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if !bytes.len().is_multiple_of(2) {
            return Err("CTT UTF-16 incompleto.".into());
        }
        let units: Vec<_> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| {
                if little {
                    u16::from_le_bytes([v[0], v[1]])
                } else {
                    u16::from_be_bytes([v[0], v[1]])
                }
            })
            .collect();
        utf16 = String::from_utf16(&units).map_err(|e| e.to_string())?;
        let text = utf16.trim_start_matches('\u{feff}');
        if text.starts_with("<?xml") {
            text.split_once("?>")
                .map_or(text, |(_, rest)| rest)
                .as_bytes()
        } else {
            text.as_bytes()
        }
    } else {
        bytes
    };
    let mut reader = quick_xml::Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut emails = Vec::new();
    loop {
        use quick_xml::events::Event;
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                if tag.local_name().as_ref().eq_ignore_ascii_case("contact") {
                    let decoded = reader.read_text(tag.name()).map_err(|e| e.to_string())?;
                    let email = quick_xml::escape::unescape(&decoded).map_err(|e| e.to_string())?;
                    if email.contains('@') {
                        emails.push(email.trim().to_owned());
                    }
                }
            }
            Ok(Event::DocType(_)) => return Err("CTT não deve conter DTD.".into()),
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("CTT/XML inválido: {e}")),
            _ => {}
        }
    }
    emails.sort();
    emails.dedup();
    if emails.is_empty() {
        Err("Nenhum endereço encontrado no arquivo CTT ou JSON.".into())
    } else {
        Ok(emails)
    }
}

pub fn contacts_xml(contacts: &[Contact]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<messenger><service name=\".NET Messenger Service\"><contactlist>\n",
    );
    for contact in contacts {
        xml.push_str(&format!(
            "<contact type=\"1\">{}</contact>\n",
            escape(&contact.user.email)
        ));
    }
    xml.push_str("</contactlist></service></messenger>\n");
    xml
}

pub fn history_export(
    messages: &[Message],
    title: &str,
    format: &str,
    names: &[(uuid::Uuid, String)],
) -> String {
    let name = |id| {
        names
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| format!("{id}"))
    };
    match format {
        "xml" => {
            let mut out = format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Log LogonName=\"{}\">\n",
                escape(title)
            );
            for m in messages {
                out.push_str(&format!("<Message DateTime=\"{}\" SessionID=\"{}\" Kind=\"{:?}\"><From><User FriendlyName=\"{}\"/></From><Text Style=\"{}\">{}</Text></Message>\n",
                    m.created_at.to_rfc3339(), m.conversation_id, m.kind, escape(&name(m.sender_id)),
                    escape(&css(m)), escape(&m.body)));
            }
            out.push_str("</Log>\n");
            out
        }
        "html" => {
            let mut out = format!(
                "<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>{}</title><style>body{{font:15px system-ui;max-width:850px;margin:40px auto;background:#f2f8fb;color:#243d50}}article{{padding:12px;background:white;margin:8px 0;border-radius:10px}}p{{white-space:pre-wrap}}small{{color:#607c90}}</style><h1>{}</h1>",
                escape(title),
                escape(title)
            );
            for m in messages {
                out.push_str(&format!(
                    "<article><small>{} · {} · {:?}</small><p style=\"{}\">{}</p></article>",
                    escape(&name(m.sender_id)),
                    m.created_at.to_rfc3339(),
                    m.kind,
                    css(m),
                    escape(&m.body)
                ));
            }
            out.push_str("</html>");
            out
        }
        _ => {
            let mut out = format!("NMessenger — {title}\n\n");
            for m in messages {
                out.push_str(&format!(
                    "[{}] {} ({:?}): {}\n",
                    m.created_at.to_rfc3339(),
                    name(m.sender_id),
                    m.kind,
                    m.body
                ));
            }
            out
        }
    }
}

fn css(m: &Message) -> String {
    let s = &m.style;
    format!(
        "color:#{:02x}{:02x}{:02x};font-size:{}px;font-weight:{};font-style:{};text-decoration:{};direction:{}",
        s.color[0],
        s.color[1],
        s.color[2],
        s.font_size.clamp(8.0, 48.0),
        if s.bold { "bold" } else { "normal" },
        if s.italic { "italic" } else { "normal" },
        if s.underline { "underline" } else { "none" },
        if s.rtl { "rtl" } else { "ltr" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctt_import_handles_entities_namespaces_and_utf16_without_expanding_dtds() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><messenger xmlns=\"urn:ctt\"><contactlist><contact type=\"1\">a&amp;b@example.test</contact><contact>zoe@example.test</contact><contact>zoe@example.test</contact></contactlist></messenger>";
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(
            contact_emails(&bytes).unwrap(),
            vec!["a&b@example.test", "zoe@example.test"]
        );
        assert!(contact_emails(b"<!DOCTYPE messenger [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><messenger><contact>&x;</contact></messenger>").is_err());
        assert!(contact_emails(b"<messenger><contact>unclosed@example.test").is_err());
    }

    #[test]
    fn history_exports_escape_names_and_message_content_and_preserve_unicode() {
        let message = Message {
            id: uuid::Uuid::new_v4(),
            conversation_id: uuid::Uuid::new_v4(),
            sender_id: uuid::Uuid::new_v4(),
            client_id: uuid::Uuid::new_v4(),
            kind: nm_protocol::MessageKind::Text,
            body: "Olá <script>alert('x')</script> & você".into(),
            style: nm_protocol::MessageStyle::default(),
            attachment_id: None,
            created_at: chrono::Utc::now(),
        };
        let names = vec![(message.sender_id, "Ana & João".into())];
        let messages = vec![message];
        for format in ["xml", "html"] {
            let text = history_export(&messages, "Amigos <3", format, &names);
            assert!(text.contains("Ana &amp; João") && text.contains("Olá &lt;script&gt;"));
            assert!(!text.contains("<script>"));
        }
        let txt = history_export(&messages, "Amigos", "txt", &names);
        assert!(txt.contains("Ana & João") && txt.contains("você"));
    }

    #[test]
    fn preferences_have_no_credential_fields() {
        let value = serde_json::to_value(Preferences::default()).unwrap();
        assert!(value.get("password").is_none() && value.get("token").is_none());
    }

    #[test]
    fn language_preference_roundtrips_and_old_preferences_follow_system() {
        use crate::i18n::LanguagePreference;
        let old: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(old.language, LanguagePreference::System);
        for language in [LanguagePreference::System, LanguagePreference::English, LanguagePreference::Portuguese] {
            let prefs = Preferences { language, ..Preferences::default() };
            let restored: Preferences = serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
            assert_eq!(restored.language, language);
        }
    }

    #[test]
    fn cached_avatar_paths_are_isolated_by_server_and_email() {
        let server = "https://example.test/v1";
        let path = cached_avatar_path(server, "user+a@example.test");
        assert_eq!(path, cached_avatar_path(server, " USER+A@example.test "));
        assert_ne!(path, cached_avatar_path(server, "user_a@example.test"));
        assert_ne!(path, cached_avatar_path("https://other.test/v1", "user+a@example.test"));
        assert!(!path.to_string_lossy().contains("example.test"));
    }

    #[tokio::test]
    async fn saved_session_roundtrips_and_can_be_cleared() {
        let server = "http://localhost:8080";
        let email = "test_roundtrip@example.com";
        let pass = "secret123";
        save_saved_session(server, email, pass).await.unwrap();
        let file = directory().join("saved_session.json");
        let encoded = tokio::fs::read(&file).await.unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains(pass));
        assert!(
            serde_json::from_slice::<serde_json::Value>(&encoded)
                .unwrap()
                .get("password")
                .is_none()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                tokio::fs::metadata(&file)
                    .await
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let loaded = load_saved_session()
            .await
            .unwrap()
            .expect("session should exist");
        assert_eq!(loaded.server, server);
        assert_eq!(loaded.email, email);
        assert_eq!(loaded.password, pass);

        clear_saved_session().await.unwrap();
        let after_clear = load_saved_session().await.unwrap();
        assert!(after_clear.is_none());
        crate::secure::write_private(
            &file,
            &serde_json::to_vec(
                &serde_json::json!({"server":server,"email":email,"password":pass}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(load_saved_session().await.unwrap().unwrap().password, pass);
        let migrated = tokio::fs::read(&file).await.unwrap();
        assert!(!String::from_utf8_lossy(&migrated).contains(pass));
        clear_saved_session().await.unwrap();
    }
}
