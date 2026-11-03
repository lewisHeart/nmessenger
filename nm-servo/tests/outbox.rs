mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn committed_messages_survive_a_delivery_worker_restart() {
    let Some(mut server) = TestServer::start().await else {
        return;
    };
    let a = server.register("Outbox sender").await;
    let b = server.register("Outbox receiver").await;
    server.befriend(&a, &b).await;
    let conversation = server.direct(&a, &b).await;
    let mut socket = server.socket(&b).await;
    server.pause_workers();
    let request = send_request("Persistida antes da publicação");
    let sent: Message = json(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/messages", conversation.id),
                &a,
            )
            .json(&request)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let queued: bool =
        sqlx::query_scalar("SELECT delivered_at IS NULL FROM message_outbox WHERE message_id=$1")
            .bind(sent.id)
            .fetch_one(&server.state.db)
            .await
            .unwrap();
    assert!(queued);
    server.resume_workers().await;
    let event = event_matching(
        &mut socket,
        |event| matches!(event, ServerEvent::Message { message } if message.id == sent.id),
    )
    .await;
    assert!(matches!(event, ServerEvent::Message { message } if message.body == request.body));
    let retried: Message = json(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/messages", conversation.id),
                &a,
            )
            .json(&request)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(retried.id, sent.id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM message_outbox WHERE message_id=$1")
        .bind(sent.id)
        .fetch_one(&server.state.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
    drop(socket);
    server.shutdown().await;
}
