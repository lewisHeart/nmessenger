use super::{Api, Bus, Update, Watch};
use crate::media::calls::MediaController;
use futures_util::{SinkExt, StreamExt};
use nm_protocol::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::tungstenite::{Message as Frame, protocol::WebSocketConfig};
use uuid::Uuid;

type Watches = Arc<Mutex<HashMap<Uuid, Watch>>>;

pub async fn run(
    api: Api,
    mut outgoing: Receiver<ClientEvent>,
    bus: Bus,
    watches: Watches,
    media: Arc<MediaController>,
) {
    let mut url = match reqwest::Url::parse(&format!("{}/events", api.base)) {
        Ok(url) => url,
        Err(e) => {
            bus.send(Update::Error(e.to_string()));
            return;
        }
    };
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    if url.set_scheme(scheme).is_err() {
        return;
    }
    let mut delay = 1;
    loop {
        bus.send(Update::Connection("Conectando tempo real…".into()));
        // Typing, game signals and media are ephemeral: never replay a disconnected backlog.
        while outgoing.try_recv().is_ok() {}
        let config = WebSocketConfig::default()
            .max_message_size(Some(512 * 1024))
            .max_frame_size(Some(512 * 1024));
        let connection = tokio::time::timeout(
            Duration::from_secs(15),
            tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config), false),
        )
        .await;
        let mut socket = match connection {
            Ok(Ok((socket, _))) => socket,
            Ok(Err(e)) => {
                bus.send(Update::Connection(format!(
                    "Sem tempo real: {e}. Nova tentativa em {delay}s."
                )));
                if !backoff(&mut outgoing, delay).await {
                    return;
                }
                delay = (delay * 2).min(30);
                continue;
            }
            Err(_) => {
                bus.send(Update::Connection(format!(
                    "Conexão expirou. Nova tentativa em {delay}s."
                )));
                if !backoff(&mut outgoing, delay).await {
                    return;
                }
                delay = (delay * 2).min(30);
                continue;
            }
        };
        let authentication = ClientEvent::Authenticate {
            token: api.token.to_string(),
        };
        let Ok(text) = serde_json::to_string(&authentication) else {
            return;
        };
        if socket.send(Frame::Text(text.into())).await.is_err() {
            continue;
        }
        let mut authenticated = false;
        let deadline = tokio::time::sleep(Duration::from_secs(10));
        tokio::pin!(deadline);
        let mut heartbeat = tokio::time::interval(Duration::from_secs(25));
        let mut last_received = tokio::time::Instant::now();
        loop {
            tokio::select! {
                _ = &mut deadline, if !authenticated => {
                    bus.send(Update::Error("O servidor não confirmou a autenticação WebSocket.".into()));
                    break;
                }
                event = outgoing.recv() => {
                    let Some(event) = event else { return; };
                    if !authenticated { continue; }
                    let event = match api.seal_event(event).await {
                        Ok(event) => event,
                        Err(error) => { bus.send(Update::Error(error)); continue; }
                    };
                    if let Ok(json) = serde_json::to_string(&event)
                        && !matches!(tokio::time::timeout(Duration::from_secs(5), socket.send(Frame::Text(json.into()))).await, Ok(Ok(()))) {
                        break;
                    }
                }
                _ = heartbeat.tick(), if authenticated => {
                    if last_received.elapsed() > Duration::from_secs(75) { break; }
                    if socket.send(Frame::Text("{\"type\":\"ping\"}".into())).await.is_err() { break; }
                }
                frame = socket.next() => {
                    let Some(frame) = frame else { break; };
                    last_received = tokio::time::Instant::now();
                    match frame {
                        Ok(Frame::Text(text)) => match parse_event(&api, &text).await {
                            Ok(ServerEvent::Authenticated { user_id }) => {
                                if user_id != api.user_id { bus.send(Update::Error("Identidade WebSocket inesperada.".into())); return; }
                                authenticated = true;
                                delay = 1;
                                bus.send(Update::Connection("Conectado".into()));
                                tokio::spawn(resync(api.clone(), bus.clone(), watches.clone()));
                            }
                            Ok(ServerEvent::SessionRevoked { session_id }) if session_id == api.session_id => {
                                media.stop_all();
                                bus.send(Update::LoggedOut);
                                bus.send(Update::Error("Esta sessão foi encerrada em outro dispositivo.".into()));
                                return;
                            }
                            Ok(ServerEvent::ResyncRequired | ServerEvent::ContactsChanged | ServerEvent::ConversationsChanged) => {
                                tokio::spawn(resync(api.clone(), bus.clone(), watches.clone()));
                            }
                            Ok(ServerEvent::Media { activity_id, from_user_id, stream, sequence, data }) => {
                                if authenticated && from_user_id != api.user_id {
                                    media.receive(activity_id, from_user_id, stream, sequence, data, bus.clone());
                                }
                            }
                            Ok(ServerEvent::Pong) => {}
                            Ok(ServerEvent::ReleaseChanged) if authenticated => {
                                tokio::spawn(check_release(api.clone(), bus.clone()));
                            }
                            Ok(ServerEvent::Signal { activity_id, from_user_id, data }) if authenticated && data.get("remote_input").is_some() => {
                                media.remote_input(activity_id, from_user_id, data["remote_input"].clone(), bus.clone());
                            }
                            Ok(event) if authenticated => bus.send(Update::Event(event)),
                            Ok(ServerEvent::Error { code, message }) => {
                                bus.send(Update::AuthenticationFailed(format!("{code}: {message}")));
                                return;
                            }
                            Ok(_) => {}
                            Err(e) => bus.send(Update::Error(format!("Evento WebSocket inválido: {e}"))),
                        },
                        Ok(Frame::Ping(data)) => { if socket.send(Frame::Pong(data)).await.is_err() { break; } }
                        Ok(Frame::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
            }
        }
        media.stop_call();
        bus.send(Update::CallPaused);
        bus.send(Update::Connection(format!(
            "Conexão interrompida. Reconectando em {delay}s…"
        )));
        if !backoff(&mut outgoing, delay).await {
            return;
        }
        delay = (delay * 2).min(30);
    }
}

async fn parse_event(api: &Api, text: &str) -> Result<ServerEvent, String> {
    api.open_event(serde_json::from_str(text).map_err(|error| format!("Evento inválido: {error}"))?)
        .await
}

async fn backoff(outgoing: &mut Receiver<ClientEvent>, seconds: u64) -> bool {
    let sleep = tokio::time::sleep(Duration::from_secs(seconds));
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = &mut sleep => return true,
            event = outgoing.recv() => if event.is_none() { return false; },
        }
    }
}

async fn resync(api: Api, bus: Bus, watches: Watches) {
    tokio::spawn(check_release(api.clone(), bus.clone()));
    let bootstrap = match api.get::<Bootstrap>("/bootstrap").await {
        Ok(value) => value,
        Err(e) => {
            bus.send(Update::Error(format!("Ressincronização: {e}")));
            return;
        }
    };
    let allowed: Vec<_> = bootstrap.conversations.iter().map(|c| c.id).collect();
    bus.send(Update::Bootstrap(Box::new(bootstrap)));
    if let Ok(pending) = super::outbox::load(&api).await {
        for message in pending
            .into_iter()
            .filter(|message| allowed.contains(&message.conversation))
        {
            bus.send(Update::PreparedMessage {
                conversation: message.conversation,
                request: message.request,
            });
        }
    }
    let watched = watches.lock().unwrap().clone();
    for (id, watch) in watched {
        if !allowed.contains(&id) {
            continue;
        }
        let mut before = None;
        loop {
            match api.history(id, before, "").await {
                Ok(messages) => {
                    let next = messages.first().map(|m| m.id);
                    let done = messages.len() < 50
                        || watch.latest.is_none()
                        || messages.iter().any(|m| Some(m.id) == watch.latest)
                        || next == before;
                    bus.send(Update::History {
                        id,
                        before,
                        query: String::new(),
                        messages,
                        revision: watch.revision,
                    });
                    if done {
                        break;
                    }
                    before = next;
                }
                Err(e) => {
                    bus.send(Update::HistoryFailed(id, watch.revision, e));
                    break;
                }
            }
        }
    }
}

async fn check_release(api: Api, bus: Bus) {
    let Ok(response) = api.request(reqwest::Method::GET, "/releases/latest").send().await else { return; };
    if response.status() != reqwest::StatusCode::OK { return; }
    let Ok(release) = super::api::bounded_json::<ReleaseInfo>(response).await else { return; };
    if release.tag_name.is_empty() || release.tag_name.len() > 128 || !release.tag_name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')) { return; }
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"));
    let offered = semver::Version::parse(release.tag_name.trim_start_matches('v'));
    if matches!((current, offered), (Ok(current), Ok(offered)) if offered > current) {
        bus.send(Update::Release(release));
    }
}
