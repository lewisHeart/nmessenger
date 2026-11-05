mod common;
use common::*;
use reqwest::Method;
use nm_protocol::{ClientEvent, ServerEvent};

#[actix_web::test]
async fn unread_hints_are_authenticated_private_and_follow_read_receipts() {
    let Some(server) = TestServer::start().await else { return; };
    let alice = server.register("Alice").await;
    let bob = server.register("Bob").await;
    let outsider = server.register("Outsider").await;
    server.befriend(&alice, &bob).await;
    let conversation = server.direct(&alice, &bob).await;
    let message = server.message(&alice, conversation.id, "private message").await;
    let anonymous = server.client.get(format!("{}/v1/notifications/unread", server.url)).send().await.unwrap();
    assert_eq!(anonymous.status(), 401);
    let hints: serde_json::Value = server.request(Method::GET, "/notifications/unread", &bob).send().await.unwrap().json().await.unwrap();
    assert_eq!(hints.as_array().unwrap().len(), 1);
    assert_eq!(hints[0]["conversation_id"], conversation.id.to_string());
    assert_eq!(hints[0]["message_id"], message.id.to_string());
    assert!(!hints.to_string().contains("private message"));
    let others: serde_json::Value = server.request(Method::GET, "/notifications/unread", &outsider).send().await.unwrap().json().await.unwrap();
    assert!(others.as_array().unwrap().is_empty());
    let read = server.request(Method::POST, &format!("/conversations/{}/read", conversation.id), &bob)
        .json(&serde_json::json!({"message_id":message.id})).send().await.unwrap();
    assert!(read.status().is_success());
    let after: serde_json::Value = server.request(Method::GET, "/notifications/unread", &bob).send().await.unwrap().json().await.unwrap();
    assert!(after.as_array().unwrap().is_empty());
    server.shutdown().await;
}

#[actix_web::test]
async fn background_socket_sends_only_hints_without_marking_user_online() {
    let Some(server) = TestServer::start().await else { return; };
    let alice = server.register("Alice").await;
    let bob = server.register("Bob").await;
    server.befriend(&alice, &bob).await;
    let conversation = server.direct(&alice, &bob).await;
    let url = format!("{}/v1/notifications/events", server.url.replacen("http://", "ws://", 1));
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send_event(&mut socket, &ClientEvent::Authenticate { token: bob.token.clone() }).await;
    assert!(matches!(next_event(&mut socket).await, ServerEvent::Authenticated { user_id } if user_id == bob.user.id));
    let count: usize = redis::cmd("ZCARD").arg(format!("nm:presence:{}", bob.user.id))
        .query_async(&mut server.state.redis.clone()).await.unwrap();
    assert_eq!(count, 0, "background notification socket must not make user online");
    let _message = server.message(&alice, conversation.id, "secret content").await;
    assert!(matches!(event_matching(&mut socket, |event| matches!(event, ServerEvent::ResyncRequired)).await, ServerEvent::ResyncRequired));
    send_event(&mut socket, &ClientEvent::Ping).await;
    assert!(matches!(event_matching(&mut socket, |event| matches!(event, ServerEvent::Pong)).await, ServerEvent::Pong));
    drop(socket);
    let missed = server.message(&alice, conversation.id, "while disconnected").await;
    let url = format!("{}/v1/notifications/events", server.url.replacen("http://", "ws://", 1));
    let (mut reconnected, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send_event(&mut reconnected, &ClientEvent::Authenticate { token: bob.token.clone() }).await;
    assert!(matches!(next_event(&mut reconnected).await, ServerEvent::Authenticated { user_id } if user_id == bob.user.id));
    let hints: serde_json::Value = server.request(Method::GET, "/notifications/unread", &bob).send().await.unwrap().json().await.unwrap();
    assert_eq!(hints[0]["message_id"], missed.id.to_string(), "reconnect must recover messages missed while offline");
    drop(reconnected);
    server.shutdown().await;
}
