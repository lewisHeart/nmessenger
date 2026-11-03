use crate::{
    auth,
    error::{Error, Result},
    presence, realtime,
    state::AppState,
};
use actix_web::{HttpRequest, HttpResponse, web};
use actix_ws::{AggregatedMessage, CloseCode, CloseReason};
use futures_util::StreamExt;
use nm_protocol::{ClientEvent, ServerEvent};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

const MAX_FRAME: usize = 512 * 1024;

async fn send(socket: &mut actix_ws::Session, event: &ServerEvent) -> bool {
    let Ok(json) = serde_json::to_string(event) else {
        return false;
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(2), socket.text(json)).await,
        Ok(Ok(()))
    )
}
async fn send_error(socket: &mut actix_ws::Session, error: &Error) -> bool {
    send(
        socket,
        &ServerEvent::Error {
            code: error.code.into(),
            message: error.message.clone(),
        },
    )
    .await
}
async fn close(socket: actix_ws::Session, description: &'static str) {
    let _ = socket
        .close(Some(CloseReason {
            code: CloseCode::Policy,
            description: Some(description.into()),
        }))
        .await;
}

pub async fn upgrade(
    req: HttpRequest,
    payload: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let peer = req
        .peer_addr()
        .map(|p| p.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    state.rate(&format!("ws-open:{peer}"), 60, 60).await?;
    let (response, mut socket, stream) =
        actix_ws::handle(&req, payload).map_err(|_| Error::bad("Upgrade WebSocket inválido"))?;
    let mut stream = stream
        .max_frame_size(MAX_FRAME)
        .aggregate_continuations()
        .max_continuation_size(MAX_FRAME);
    let mut events = state.events.subscribe();
    actix_web::rt::spawn(async move {
        let first = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match stream.next().await {
                    Some(Ok(AggregatedMessage::Text(text))) => {
                        if text.len() > 256 {
                            return Err(Error::unauthorized());
                        }
                        let event = serde_json::from_str::<ClientEvent>(&text)
                            .map_err(|_| Error::bad("Evento inválido"))?;
                        return match event {
                            ClientEvent::Authenticate { token } => {
                                auth::authenticate(&state, &token).await
                            }
                            _ => Err(Error::unauthorized()),
                        };
                    }
                    Some(Ok(AggregatedMessage::Ping(bytes))) => {
                        state
                            .rate(&format!("ws-preauth-control:{peer}"), 20, 10)
                            .await?;
                        socket
                            .pong(&bytes)
                            .await
                            .map_err(|_| Error::unauthorized())?;
                    }
                    _ => return Err(Error::unauthorized()),
                }
            }
        })
        .await;
        let session = match first {
            Ok(Ok(session)) => session,
            Ok(Err(error)) => {
                send_error(&mut socket, &error).await;
                close(socket, "authentication failed").await;
                return;
            }
            Err(_) => {
                send_error(&mut socket, &Error::unauthorized()).await;
                close(socket, "authentication timeout").await;
                return;
            }
        };
        let connection = Uuid::new_v4();
        if let Err(error) = presence::refresh(&state, session.user_id, connection).await {
            send_error(&mut socket, &error).await;
            close(socket, "presence unavailable").await;
            return;
        }
        if !send(
            &mut socket,
            &ServerEvent::Authenticated {
                user_id: session.user_id,
            },
        )
        .await
        {
            let _ = presence::disconnect(&state, session.user_id, connection).await;
            return;
        }
        let _ = presence::announce(&state, session.user_id).await;
        let mut check = tokio::time::interval(Duration::from_secs(2));
        check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_input = Instant::now();
        let mut heartbeat = Instant::now();
        let mut invalid = 0u8;
        loop {
            tokio::select! {
                message=stream.next()=>{
                    last_input=Instant::now();
                    match message {
                        Some(Ok(AggregatedMessage::Text(text)))=>{
                            if state.rate(&format!("ws-frames:{}",session.user_id),200,1).await.is_err() {
                                send_error(&mut socket,&Error::limit()).await;
                                break;
                            }
                            let result=match serde_json::from_str::<ClientEvent>(&text) {
                                Ok(event)=>realtime::handle(&state,&session,connection,event).await,
                                Err(_)=>Err(Error::bad("Evento inválido")),
                            };
                            match result {
                                Ok(Some(event))=>{ if !send(&mut socket,&event).await { break; } },
                                Ok(None)=>{},
                                Err(error)=>{
                                    invalid=invalid.saturating_add(1);
                                    if !send_error(&mut socket,&error).await
                                        || error.status==actix_web::http::StatusCode::UNAUTHORIZED || invalid>=10 { break; }
                                },
                            }
                        },
                        Some(Ok(AggregatedMessage::Ping(bytes)))=>{
                            if state.rate(&format!("ws-control:{}",session.user_id),30,10).await.is_err()
                                || socket.pong(&bytes).await.is_err() { break; }
                        },
                        Some(Ok(AggregatedMessage::Pong(_)))=>{
                            if state.rate(&format!("ws-control:{}",session.user_id),30,10).await.is_err() { break; }
                        },
                        Some(Ok(AggregatedMessage::Binary(_)))=>{
                            send_error(&mut socket,&Error::bad("Use eventos JSON tipados")).await;
                            break;
                        },
                        _=>break,
                    }
                },
                event=events.recv()=>{
                    match event {
                        Ok(envelope)=>{
                            let all=envelope.recipients.is_empty() && matches!(envelope.event,ServerEvent::ResyncRequired | ServerEvent::ReleaseChanged);
                            if !all && !envelope.recipients.contains(&session.user_id) { continue; }
                            if let Err(error)=auth::valid(&state,&session).await {
                                if error.status==actix_web::http::StatusCode::UNAUTHORIZED {
                                    send(&mut socket,&ServerEvent::SessionRevoked {session_id:session.id}).await;
                                } else { send_error(&mut socket,&error).await; }
                                break;
                            }
                            match realtime::can_deliver(&state,session.user_id,&envelope.event).await {
                                Ok(true)=>{}, Ok(false)=>continue,
                                Err(_)=>{
                                    if !send(&mut socket,&ServerEvent::ResyncRequired).await { break; }
                                    continue;
                                },
                            }
                            let revoked=matches!(envelope.event,ServerEvent::SessionRevoked {session_id} if session_id==session.id);
                            if !send(&mut socket,&envelope.event).await || revoked { break; }
                        },
                        Err(broadcast::error::RecvError::Lagged(_))=>{
                            if !send(&mut socket,&ServerEvent::ResyncRequired).await { break; }
                        },
                        Err(broadcast::error::RecvError::Closed)=>break,
                    }
                },
                _=check.tick()=>{
                    if let Err(error)=auth::valid(&state,&session).await {
                        if error.status==actix_web::http::StatusCode::UNAUTHORIZED {
                            send(&mut socket,&ServerEvent::SessionRevoked {session_id:session.id}).await;
                        } else {
                            send_error(&mut socket,&error).await;
                        }
                        break;
                    }
                    if last_input.elapsed()>Duration::from_secs(60) { break; }
                    if heartbeat.elapsed()>Duration::from_secs(20) {
                        heartbeat=Instant::now();
                        if presence::refresh(&state,session.user_id,connection).await.is_err()
                            || socket.ping(b"nm").await.is_err() { break; }
                        let _=sqlx::query("UPDATE auth_sessions SET last_seen_at=clock_timestamp() WHERE id=$1")
                            .bind(session.id).execute(&state.db).await;
                    }
                },
            }
        }
        close(socket, "connection ended").await;
        let _ = presence::disconnect(&state, session.user_id, connection).await;
    });
    Ok(response)
}
