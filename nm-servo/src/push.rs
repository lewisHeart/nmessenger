//! Authenticated, content-free unread message hints for the Android service.
use crate::{auth::{self, Session}, error::{Error, Result}, realtime, state::AppState};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt;
use nm_protocol::{ClientEvent, ServerEvent};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
struct Unread { conversation_id: Uuid, message_id: Uuid }

async fn unread(state: web::Data<AppState>, session: Session) -> Result<web::Json<Vec<Unread>>> {
    let rows = sqlx::query_as::<_, Unread>(
        "SELECT conversation_id,message_id FROM (
         SELECT DISTINCT ON (m.conversation_id) m.conversation_id,m.id AS message_id,m.created_at
         FROM conversation_members mine JOIN messages m ON m.conversation_id=mine.conversation_id
         JOIN users u ON u.id=mine.user_id
         WHERE mine.user_id=$1 AND m.sender_id<>$1 AND m.created_at>mine.hidden_at
         AND (m.created_at,m.id)>(mine.read_at,mine.read_id)
         AND COALESCE((u.settings->>'message_alerts')::boolean,true)
         AND NOT EXISTS(SELECT 1 FROM user_blocks b JOIN conversation_members x ON x.conversation_id=m.conversation_id AND x.user_id=b.owner_id
             JOIN conversation_members y ON y.conversation_id=m.conversation_id AND y.user_id=b.user_id)
         ORDER BY m.conversation_id,m.created_at DESC,m.id DESC
         ) latest ORDER BY created_at DESC,message_id DESC LIMIT 50")
        .bind(session.user_id).fetch_all(&state.db).await?;
    Ok(web::Json(rows))
}

/// A notification-only socket: it never changes the user's online presence and
/// sends only resync hints, not message content or media.
async fn events(req: HttpRequest, payload: web::Payload, state: web::Data<AppState>) -> Result<HttpResponse> {
    let peer = req.peer_addr().map(|p| p.ip().to_string()).unwrap_or_else(|| "unknown".into());
    state.rate(&format!("notification-ws-open:{peer}"), 60, 60).await?;
    let (response, mut socket, stream) = actix_ws::handle(&req, payload).map_err(|_| Error::bad("WebSocket inválido"))?;
    let mut stream = stream.max_frame_size(4096).aggregate_continuations().max_continuation_size(4096);
    let mut events = state.events.subscribe();
    actix_web::rt::spawn(async move {
        let first = tokio::time::timeout(std::time::Duration::from_secs(10), stream.next()).await;
        let session = match first {
            Ok(Some(Ok(AggregatedMessage::Text(text)))) if text.len() <= 256 => {
                match serde_json::from_str::<ClientEvent>(&text) {
                    Ok(ClientEvent::Authenticate { token }) => auth::authenticate(&state, &token).await.ok(),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(session) = session else { let _ = socket.close(None).await; return; };
        let welcome = ServerEvent::Authenticated { user_id: session.user_id };
        if socket.text(serde_json::to_string(&welcome).unwrap_or_default()).await.is_err() { return; }
        let mut check = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut last_input = std::time::Instant::now();
        loop {
            tokio::select! {
                incoming = stream.next() => match incoming {
                    Some(Ok(AggregatedMessage::Text(text))) if text.len() <= 256 => {
                        last_input = std::time::Instant::now();
                        if state.rate(&format!("notification-ws-frames:{}", session.user_id), 30, 10).await.is_err() { break; }
                        match serde_json::from_str::<ClientEvent>(&text) {
                            Ok(ClientEvent::Ping) => {
                                if socket.text("{\"type\":\"pong\"}").await.is_err() { break; }
                            }
                            _ => break,
                        }
                    }
                    Some(Ok(AggregatedMessage::Ping(bytes))) => {
                        last_input = std::time::Instant::now();
                        if state.rate(&format!("notification-ws-control:{}", session.user_id), 30, 10).await.is_err() || socket.pong(&bytes).await.is_err() { break; }
                    },
                    Some(Ok(AggregatedMessage::Pong(_))) => { last_input = std::time::Instant::now(); },
                    _ => break,
                },
                _ = check.tick() => if last_input.elapsed() > std::time::Duration::from_secs(90) || auth::valid(&state, &session).await.is_err() { break; },
                event = events.recv() => match event {
                    Ok(envelope) => {
                        let broadcast = envelope.recipients.is_empty() && matches!(envelope.event, ServerEvent::ResyncRequired);
                        if !broadcast && !envelope.recipients.contains(&session.user_id) { continue; }
                        if matches!(&envelope.event, ServerEvent::SessionRevoked { session_id } if *session_id == session.id) { break; }
                        let relevant = match &envelope.event {
                            ServerEvent::Message { message } if message.sender_id != session.user_id => {
                                matches!(realtime::can_deliver(&state, session.user_id, &envelope.event).await, Ok(true))
                            }
                            ServerEvent::ResyncRequired | ServerEvent::MessageDeleted { .. } | ServerEvent::Read { .. } => true,
                            _ => false,
                        };
                        if relevant && socket.text("{\"type\":\"resync_required\"}").await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if socket.text("{\"type\":\"resync_required\"}").await.is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
        }
        let _ = socket.close(None).await;
    });
    Ok(response)
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/notifications/unread", web::get().to(unread));
    cfg.route("/notifications/events", web::get().to(events));
}
