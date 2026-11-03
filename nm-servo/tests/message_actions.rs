mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn author_can_edit_and_delete_text_but_other_users_cannot() {
    let Some(mut server) = TestServer::start().await else { return; };
    let author = server.register("Author").await;
    let reader = server.register("Reader").await;
    server.befriend(&author, &reader).await;
    let conversation = server.direct(&author, &reader).await;
    let mut socket = server.socket(&reader).await;
    let original = server.message(&author, conversation.id, "original").await;
    let route = format!("/conversations/{}/messages/{}", conversation.id, original.id);
    let mut replacement = send_request("edited");
    replacement.client_id = original.client_id;
    let denied = server.request(Method::PATCH, &route, &reader).json(&replacement).send().await.unwrap();
    assert_eq!(denied.status(), 403);
    server.pause_workers();
    let edited: Message = json(server.request(Method::PATCH, &route, &author).json(&replacement).send().await.unwrap()).await;
    assert_eq!(edited.id, original.id);
    assert_eq!(edited.body, "edited");
    let queued_edit: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM message_outbox WHERE message_id=$1 AND delivered_at IS NULL AND event->>'type'='message_updated')")
        .bind(original.id).fetch_one(&server.state.db).await.unwrap();
    assert!(queued_edit);
    server.resume_workers().await;
    assert!(matches!(event_matching(&mut socket, |event| matches!(event, ServerEvent::MessageUpdated { message } if message.id == original.id)).await,
        ServerEvent::MessageUpdated { message } if message.body == "edited"));
    sqlx::query("UPDATE messages SET created_at=clock_timestamp()-interval '16 minutes' WHERE id=$1")
        .bind(original.id).execute(&server.state.db).await.unwrap();
    assert_eq!(server.request(Method::PATCH, &route, &author).json(&replacement).send().await.unwrap().status(), 409);
    assert_eq!(server.request(Method::DELETE, &route, &reader).send().await.unwrap().status(), 403);
    server.pause_workers();
    assert!(server.request(Method::DELETE, &route, &author).send().await.unwrap().status().is_success());
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM message_deletion_outbox WHERE event->>'message_id'=$1")
        .bind(original.id.to_string()).fetch_one(&server.state.db).await.unwrap();
    assert_eq!(queued, 1);
    server.resume_workers().await;
    event_matching(&mut socket, |event| matches!(event, ServerEvent::MessageDeleted { message_id, .. } if *message_id == original.id)).await;
    let history: Vec<Message> = json(server.request(Method::GET, &format!("/conversations/{}/messages", conversation.id), &reader).send().await.unwrap()).await;
    assert!(history.iter().all(|message| message.id != original.id));
    server.shutdown().await;
}
