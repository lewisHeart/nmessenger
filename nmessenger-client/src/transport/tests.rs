use super::*;
use chrono::{TimeZone, Utc};
use futures_util::{SinkExt, StreamExt};
use reqwest::Method;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_tungstenite::tungstenite::Message as Frame;
use zeroize::Zeroizing;

fn profile(id: u128) -> Profile {
    Profile {
        id: Uuid::from_u128(id),
        email: format!("p{id}@example.test"),
        display_name: format!("Pessoa {id}"),
        personal_message: String::new(),
        presence: Presence::Available,
        avatar_id: None,
        scene: String::new(),
        now_playing: None,
        created_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        bio: String::new(),
        badges: Vec::new(),
    }
}

fn message(index: u128) -> Message {
    Message {
        id: Uuid::from_u128(index + 100),
        conversation_id: Uuid::from_u128(10),
        sender_id: profile(2).id,
        client_id: Uuid::from_u128(index + 1000),
        kind: MessageKind::Text,
        body: format!("Olá {index}"),
        style: MessageStyle::default(),
        attachment_id: None,
        created_at: Utc.timestamp_opt(1_700_000_000 + index as i64, 0).unwrap(),
    }
}

fn fixture_bootstrap() -> Bootstrap {
    Bootstrap {
        me: profile(1),
        contacts: vec![],
        categories: vec![],
        settings: UserSettings::default(),
        sessions: vec![],
        assets: vec![],
        activities: vec![],
        conversations: vec![Conversation {
            id: Uuid::from_u128(10),
            title: None,
            kind: ConversationKind::Direct,
            members: vec![profile(1), profile(2)],
            updated_at: message(71).created_at,
            unread: 71,
            last_message: Some(message(71)),
        }],
    }
}

fn fixture_api(address: std::net::SocketAddr) -> Api {
    Api {
        crypto: None,
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
        base: format!("http://{address}/v1"),
        token: Arc::new(Zeroizing::new("contract-token".into())),
        user_id: profile(1).id,
        session_id: Uuid::from_u128(20),
    }
}

async fn request(stream: &mut TcpStream) -> (String, Vec<u8>) {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let end = loop {
        let count = stream.read(&mut buffer).await.unwrap();
        assert!(count > 0 && bytes.len() < 512 * 1024);
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while bytes.len() < end + length {
        let count = stream.read(&mut buffer).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
    }
    (headers, bytes[end..end + length].to_vec())
}

async fn reply(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
}

#[tokio::test]
async fn http_uses_direct_arrays_bearer_encoded_queries_multipart_and_exact_dtos() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = fixture_api(listener.local_addr().unwrap());
    let contact = Contact {
        user: profile(2),
        nickname: None,
        category_ids: vec![],
        favorite: true,
        blocked: false,
        state: ContactState::Accepted,
        details: ContactDetails::default(),
    };
    let file = FileInfo {
        id: Uuid::from_u128(30),
        owner_id: profile(1).id,
        conversation_id: None,
        filename: "foto.png".into(),
        mime: "image/png".into(),
        size: 4,
        sha256: "hash".into(),
        created_at: message(0).created_at,
    };
    let server_file = file.clone();
    let server = tokio::spawn(async move {
        for step in 0..6 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (headers, body) = request(&mut stream).await;
            assert!(
                headers
                    .to_lowercase()
                    .contains("authorization: bearer contract-token")
            );
            let target = headers
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap();
            assert!(!target.contains("contract-token"));
            match step {
                0 => {
                    assert_eq!(target, "/v1/contacts");
                    reply(
                        &mut stream,
                        "200 OK",
                        &serde_json::to_string(&vec![contact.clone()]).unwrap(),
                    )
                    .await;
                }
                1 => {
                    let url = reqwest::Url::parse(&format!("http://local{target}")).unwrap();
                    let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
                    assert_eq!(params["q"], "Olá & álbum");
                    assert_eq!(params["before"], message(4).id.to_string());
                    reply(
                        &mut stream,
                        "200 OK",
                        &serde_json::to_string(&vec![message(1), message(2)]).unwrap(),
                    )
                    .await;
                }
                2 => {
                    assert!(headers.starts_with("POST "));
                    let request: SendMessageRequest = serde_json::from_slice(&body).unwrap();
                    assert_eq!(request.client_id, Uuid::from_u128(999));
                    assert!(request.style.bold);
                    assert_eq!(request.body, "Olá & álbum");
                    reply(
                        &mut stream,
                        "200 OK",
                        &serde_json::to_string(&message(5)).unwrap(),
                    )
                    .await;
                }
                3 => {
                    assert!(target.ends_with("/read"));
                    let request: ReadRequest = serde_json::from_slice(&body).unwrap();
                    assert_eq!(request.message_id, Some(message(5).id));
                    reply(&mut stream, "204 No Content", "").await;
                }
                4 => {
                    assert_eq!(target, "/v1/files");
                    assert!(
                        headers
                            .to_lowercase()
                            .contains("multipart/form-data; boundary=")
                    );
                    let text = String::from_utf8_lossy(&body);
                    assert!(text.contains("name=\"file\""));
                    assert!(text.contains("filename=\"foto.png\""));
                    reply(
                        &mut stream,
                        "200 OK",
                        &serde_json::to_string(&server_file).unwrap(),
                    )
                    .await;
                }
                _ => {
                    reply(
                        &mut stream,
                        "403 Forbidden",
                        r#"{"code":"forbidden","message":"Permissão negada"}"#,
                    )
                    .await
                }
            }
        }
    });
    let contacts: Vec<Contact> = api.get("/contacts").await.unwrap();
    assert_eq!(contacts.len(), 1);
    let history = api
        .history(Uuid::from_u128(10), Some(message(4).id), "Olá & álbum")
        .await
        .unwrap();
    assert_eq!(history[0].id, message(1).id);
    let outgoing = SendMessageRequest {
        client_id: Uuid::from_u128(999),
        kind: MessageKind::Text,
        body: "Olá & álbum".into(),
        style: MessageStyle {
            bold: true,
            ..Default::default()
        },
        attachment_id: None,
    };
    let _: Message = api
        .json(
            Method::POST,
            "/conversations/00000000-0000-0000-0000-00000000000a/messages",
            &outgoing,
        )
        .await
        .unwrap();
    api.read(Uuid::from_u128(10), Some(message(5).id))
        .await
        .unwrap();
    assert_eq!(
        api.upload(None, "foto.png".into(), "image/png", vec![1, 2, 3, 4])
            .await
            .unwrap()
            .id,
        file.id
    );
    let error = api.get::<UserSettings>("/settings").await.unwrap_err();
    assert!(error.contains("Permissão negada") && error.contains("403"));
    server.await.unwrap();
}

#[tokio::test]
#[allow(clippy::result_large_err)] // The tungstenite callback fixes this error type.
async fn websocket_authenticates_in_first_frame_and_resyncs_multiple_pages_after_reconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = fixture_api(listener.local_addr().unwrap());
    let connects = Arc::new(AtomicUsize::new(0));
    let drop_first = Arc::new(tokio::sync::Notify::new());
    let count = connects.clone();
    let disconnect = drop_first.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let count = count.clone();
            let disconnect = disconnect.clone();
            tokio::spawn(async move {
                let mut peek = [0u8; 1024];
                let n = stream.peek(&mut peek).await.unwrap();
                if String::from_utf8_lossy(&peek[..n]).contains("/v1/events") {
                    let mut socket = tokio_tungstenite::accept_hdr_async(
                        stream,
                        |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                         response| {
                            assert_eq!(request.uri().path(), "/v1/events");
                            assert!(request.uri().query().is_none());
                            Ok(response)
                        },
                    )
                    .await
                    .unwrap();
                    let frame = socket.next().await.unwrap().unwrap();
                    let event: ClientEvent =
                        serde_json::from_str(frame.to_text().unwrap()).unwrap();
                    assert!(
                        matches!(event,ClientEvent::Authenticate { token } if token == "contract-token")
                    );
                    let number = count.fetch_add(1, Ordering::SeqCst);
                    socket
                        .send(Frame::Text(
                            serde_json::to_string(&ServerEvent::Authenticated {
                                user_id: profile(1).id,
                            })
                            .unwrap()
                            .into(),
                        ))
                        .await
                        .unwrap();
                    if number == 0 {
                        disconnect.notified().await;
                        let _ = socket.close(None).await;
                    } else {
                        while socket.next().await.is_some() {}
                    }
                } else {
                    let (headers, _) = request(&mut stream).await;
                    let target = headers
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let body = if target == "/v1/bootstrap" {
                        serde_json::to_string(&fixture_bootstrap()).unwrap()
                    } else {
                        let url = reqwest::Url::parse(&format!("http://local{target}")).unwrap();
                        let before = url
                            .query_pairs()
                            .find(|(k, _)| k == "before")
                            .map(|(_, v)| Uuid::parse_str(&v).unwrap());
                        let page: Vec<_> = if before.is_none() {
                            (22..72).map(message).collect()
                        } else {
                            assert_eq!(before, Some(message(22).id));
                            (0..22).map(message).collect()
                        };
                        serde_json::to_string(&page).unwrap()
                    };
                    reply(&mut stream, "200 OK", &body).await;
                }
            });
        }
    });
    let (sender, receiver) = mpsc::channel();
    let bus = Bus {
        generation: 7,
        sender,
        context: egui::Context::default(),
    };
    let (_outgoing, rx) = async_mpsc::channel(128);
    let watches = Arc::new(Mutex::new(HashMap::from([(
        Uuid::from_u128(10),
        Watch {
            latest: Some(message(0).id),
            revision: 3,
        },
    )])));
    let connection = tokio::spawn(websocket::run(
        api,
        rx,
        bus,
        watches,
        Arc::new(MediaController::default()),
    ));
    let mut bootstraps = 0;
    let mut pages = 0;
    let mut sizes = Vec::new();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            while let Ok(packet) = receiver.try_recv() {
                assert_eq!(packet.generation, 7);
                match packet.update {
                    Update::Bootstrap(_) => bootstraps += 1,
                    Update::History {
                        messages, revision, ..
                    } => {
                        assert_eq!(revision, 3);
                        pages += 1;
                        sizes.push(messages.len());
                    }
                    Update::Error(e) | Update::AuthenticationFailed(e) => panic!("{e}"),
                    _ => {}
                }
            }
            if pages == 2 {
                drop_first.notify_one();
            }
            if pages >= 4 && bootstraps >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("A reconexão deve recuperar todas as páginas perdidas");
    assert_eq!(sizes, vec![50, 22, 50, 22]);
    assert!(connects.load(Ordering::SeqCst) >= 2);
    connection.abort();
    server.abort();
}

#[test]
fn server_addresses_never_accept_url_credentials_or_query_tokens() {
    for invalid in [
        "file:///tmp/data",
        "https://me:secret@localhost",
        "http://localhost?token=secret",
        "http://localhost#token",
    ] {
        assert!(api::base(invalid).is_err());
    }
    assert_eq!(
        api::base("https://example.test/v1/").unwrap(),
        "https://example.test/v1"
    );
    assert_eq!(
        api::base("http://192.168.1.50:8000").unwrap(),
        "http://192.168.1.50:8000/v1"
    );
    assert_eq!(
        api::base("http://servidor-remoto.local:3000/v1").unwrap(),
        "http://servidor-remoto.local:3000/v1"
    );
}

#[tokio::test]
async fn avatar_updates_use_asset_ids_and_publish_file_references_preserving_background() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = fixture_api(listener.local_addr().unwrap());
    let asset = UserAsset {
        id: Uuid::from_u128(400),
        file_id: Uuid::from_u128(500),
        kind: AssetKind::Avatar,
        name: "Meu avatar".into(),
        shortcut: None,
    };
    let background = Uuid::from_u128(600);
    let expected = asset.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (headers, _) = request(&mut stream).await;
        assert!(headers.starts_with("GET /v1/bootstrap "));
        let mut bootstrap = fixture_bootstrap();
        bootstrap.me.scene = crate::media::scene::Scene {
            avatar_file: None,
            background_file: Some(background),
        }
        .encode();
        reply(
            &mut stream,
            "200 OK",
            &serde_json::to_string(&bootstrap).unwrap(),
        )
        .await;
        let (mut stream, _) = listener.accept().await.unwrap();
        let (headers, body) = request(&mut stream).await;
        assert!(headers.starts_with("PATCH /v1/me "));
        let request: UpdateProfileRequest = serde_json::from_slice(&body).unwrap();
        assert_eq!(request.avatar_id, Some(expected.id));
        let scene = crate::media::scene::Scene::parse(request.scene.as_deref().unwrap());
        assert_eq!(scene.avatar_file, Some(expected.file_id));
        assert_eq!(scene.background_file, Some(background));
        bootstrap.me.avatar_id = request.avatar_id;
        bootstrap.me.scene = request.scene.unwrap();
        reply(
            &mut stream,
            "200 OK",
            &serde_json::to_string(&bootstrap.me).unwrap(),
        )
        .await;
    });
    let (sender, receiver) = mpsc::channel();
    let bus = Bus {
        generation: 1,
        sender,
        context: egui::Context::default(),
    };
    files::apply_asset(&api, &bus, &asset).await.unwrap();
    assert!(
        matches!(receiver.try_recv().unwrap().update,Update::Profile(p) if p.avatar_id == Some(asset.id))
    );
    server.await.unwrap();
}
