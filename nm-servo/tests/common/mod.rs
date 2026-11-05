#![allow(dead_code)]

use actix_web::{App, HttpServer, dev::ServerHandle, web};
use futures_util::{SinkExt, StreamExt};
use nm_protocol::*;
use nm_servo::{
    configure,
    state::{AppState, Config},
    workers::{self, Workers},
};
use reqwest::{Client, Method, Response};
use serde::de::DeserializeOwned;
use std::{net::TcpListener, path::PathBuf, sync::Mutex, time::Duration};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message as WsMessage};
use uuid::Uuid;

pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct TestServer {
    pub url: String,
    pub client: Client,
    pub state: AppState,
    handle: ServerHandle,
    workers: Option<Workers>,
    directory: PathBuf,
    users: Mutex<Vec<Uuid>>,
}

impl TestServer {
    pub async fn start() -> Option<Self> {
        let database_url = std::env::var("NM_TEST_DATABASE_URL").ok();
        let redis_url = std::env::var("NM_TEST_REDIS_URL").ok();
        let (database_url, redis_url) = match (database_url, redis_url) {
            (Some(pg), Some(redis)) => (pg, redis),
            (None, None) => {
                eprintln!(
                    "Integration test skipped: set NM_TEST_DATABASE_URL and NM_TEST_REDIS_URL"
                );
                return None;
            }
            _ => panic!("Set BOTH NM_TEST_DATABASE_URL and NM_TEST_REDIS_URL"),
        };
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-uploads")
            .join(Uuid::new_v4().to_string());
        let config = Config {
            database_url,
            redis_url,
            bind_addr: "127.0.0.1:0".into(),
            upload_dir: directory.clone(),
            session_ttl_secs: 3600,
            user_upload_quota: 512 * 1024 * 1024,
        };
        let state = AppState::connect(&config)
            .await
            .expect("connect test PostgreSQL/Redis and migrate");
        // Most integration tests manage their own administrator fixtures.
        sqlx::query("UPDATE first_account_bootstrap SET claimed=true WHERE singleton")
            .execute(&state.db).await.expect("initialize test account policy");
        let workers = workers::start(&state).await.expect("subscribe Redis");
        let data = web::Data::new(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server =
            HttpServer::new(move || App::new().app_data(data.clone()).configure(configure))
                .workers(1)
                .listen(listener)
                .unwrap()
                .run();
        let handle = server.handle();
        actix_web::rt::spawn(server);
        let client = Client::builder().timeout(Duration::from_secs(20));
        // Separate loopback addresses prevent independent test servers from sharing
        // the production authentication rate-limit bucket for 127.0.0.1.
        #[cfg(target_os = "linux")]
        let client = {
            let bytes = Uuid::new_v4().into_bytes();
            client.local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, bytes[0], bytes[1], bytes[2].max(1))))
        };
        Some(Self {
            url,
            client: client.build().unwrap(),
            state,
            handle,
            workers: Some(workers),
            directory,
            users: Mutex::new(Vec::new()),
        })
    }

    pub fn request(
        &self,
        method: Method,
        route: &str,
        auth: &AuthResponse,
    ) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}/v1{route}", self.url))
            .bearer_auth(&auth.token)
    }

    pub async fn register(&self, name: &str) -> AuthResponse {
        let body = RegisterRequest {
            email: format!("{}@nm.test", Uuid::new_v4()),
            password: "test password 123".into(),
            display_name: name.into(),
            device_name: "Integration desktop".into(),
        };
        let auth: AuthResponse = json(
            self.client
                .post(format!("{}/v1/auth/register", self.url))
                .json(&body)
                .send()
                .await
                .unwrap(),
        )
        .await;
        self.users.lock().unwrap().push(auth.user.id);
        auth
    }

    pub async fn login(&self, account: &AuthResponse, device: &str) -> AuthResponse {
        json(
            self.client
                .post(format!("{}/v1/auth/login", self.url))
                .json(&LoginRequest {
                    email: account.user.email.clone(),
                    password: "test password 123".into(),
                    device_name: device.into(),
                })
                .send()
                .await
                .unwrap(),
        )
        .await
    }

    pub async fn befriend(&self, a: &AuthResponse, b: &AuthResponse) {
        let c: Contact = json(
            self.request(Method::POST, "/contacts", a)
                .json(&AddContactRequest {
                    email: b.user.email.clone(),
                    nickname: None,
                })
                .send()
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(c.state, ContactState::PendingOutgoing);
        let c: Contact = json(
            self.request(Method::PATCH, &format!("/contacts/{}", a.user.id), b)
                .json(&UpdateContactRequest {
                    accepted: Some(true),
                    ..Default::default()
                })
                .send()
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(c.state, ContactState::Accepted);
    }

    pub async fn direct(&self, a: &AuthResponse, b: &AuthResponse) -> Conversation {
        json(
            self.request(Method::POST, "/conversations", a)
                .json(&CreateConversationRequest {
                    member_ids: vec![b.user.id],
                    title: None,
                    members_can_invite: true,
                })
                .send()
                .await
                .unwrap(),
        )
        .await
    }

    pub async fn group(&self, owner: &AuthResponse, others: &[&AuthResponse]) -> Conversation {
        json(
            self.request(Method::POST, "/conversations", owner)
                .json(&CreateConversationRequest {
                    member_ids: others.iter().map(|u| u.user.id).collect(),
                    title: Some("Test group".into()),
                    members_can_invite: true,
                })
                .send()
                .await
                .unwrap(),
        )
        .await
    }

    pub async fn message(&self, auth: &AuthResponse, conversation: Uuid, text: &str) -> Message {
        json(
            self.request(
                Method::POST,
                &format!("/conversations/{conversation}/messages"),
                auth,
            )
            .json(&send_request(text))
            .send()
            .await
            .unwrap(),
        )
        .await
    }

    pub async fn upload(
        &self,
        auth: &AuthResponse,
        conversation: Option<Uuid>,
        name: &str,
        bytes: Vec<u8>,
        mime: &str,
    ) -> Response {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(name.to_owned())
            .mime_str(mime)
            .unwrap();
        let route = conversation
            .map(|id| format!("/conversations/{id}/files"))
            .unwrap_or_else(|| "/files".into());
        self.request(Method::POST, &route, auth)
            .multipart(reqwest::multipart::Form::new().part("file", part))
            .send()
            .await
            .unwrap()
    }

    pub async fn socket(&self, auth: &AuthResponse) -> Socket {
        let url = format!("{}/v1/events", self.url.replacen("http://", "ws://", 1));
        let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        send_event(
            &mut socket,
            &ClientEvent::Authenticate {
                token: auth.token.clone(),
            },
        )
        .await;
        assert!(
            matches!(next_event(&mut socket).await,ServerEvent::Authenticated {user_id} if user_id==auth.user.id)
        );
        socket
    }

    pub async fn shutdown(mut self) {
        // Clean up persisted data before stopping the HTTP server: actix-web 4.18
        // shuts its runtime down on stop, and sqlx needs a live runtime for the pool.
        self.workers.take();
        let ids = self.users.lock().unwrap().clone();
        sqlx::query("DELETE FROM conversations WHERE owner_id=ANY($1) OR id IN(SELECT conversation_id FROM conversation_members WHERE user_id=ANY($1))")
            .bind(&ids).execute(&self.state.db).await.unwrap();
        sqlx::query("DELETE FROM users WHERE id=ANY($1)")
            .bind(&ids)
            .execute(&self.state.db)
            .await
            .unwrap();
        self.handle.stop(false).await;
        let _ = tokio::fs::remove_dir_all(&self.directory).await;
    }

    pub fn pause_workers(&mut self) {
        self.workers.take();
    }

    pub async fn resume_workers(&mut self) {
        self.workers = Some(
            workers::start(&self.state)
                .await
                .expect("restart delivery workers"),
        );
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let handle = self.handle.clone();
        actix_web::rt::spawn(async move {
            handle.stop(false).await;
        });
    }
}

pub fn send_request(text: &str) -> SendMessageRequest {
    SendMessageRequest {
        client_id: Uuid::new_v4(),
        kind: MessageKind::Text,
        body: text.into(),
        style: MessageStyle::default(),
        attachment_id: None,
    }
}

pub async fn json<T: DeserializeOwned>(response: Response) -> T {
    assert!(
        response.status().is_success(),
        "HTTP {}: {}",
        response.status(),
        response.text().await.unwrap()
    );
    response
        .json()
        .await
        .expect("exact shared-protocol JSON response")
}

pub async fn error(response: Response, status: u16) -> ApiError {
    assert_eq!(response.status().as_u16(), status);
    let error: ApiError = response.json().await.expect("consistent ApiError");
    assert!(!error.code.is_empty());
    error
}

pub async fn send_event(socket: &mut Socket, event: &ClientEvent) {
    socket
        .send(WsMessage::Text(
            serde_json::to_string(event).unwrap().into(),
        ))
        .await
        .unwrap();
}

pub async fn next_event(socket: &mut Socket) -> ServerEvent {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            match socket.next().await.expect("WS closed").expect("WS frame") {
                WsMessage::Text(text) => return serde_json::from_str(&text).expect("ServerEvent"),
                WsMessage::Ping(bytes) => socket.send(WsMessage::Pong(bytes)).await.unwrap(),
                WsMessage::Close(_) => panic!("WS closed before event"),
                _ => {}
            }
        }
    })
    .await
    .expect("event timeout")
}

pub async fn event_matching(
    socket: &mut Socket,
    predicate: impl Fn(&ServerEvent) -> bool,
) -> ServerEvent {
    for _ in 0..100 {
        let event = next_event(socket).await;
        if predicate(&event) {
            return event;
        }
    }
    panic!("matching event not found")
}
