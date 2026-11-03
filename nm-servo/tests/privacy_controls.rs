mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn private_reads_clear_unread_without_broadcasting_and_typing_is_server_enforced() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let a = server.register("Remetente").await;
    let b = server.register("Destinatário").await;
    server.befriend(&a, &b).await;
    let conversation = server.direct(&a, &b).await;
    let mut settings = UserSettings {
        read_receipts: false,
        typing_indicators: false,
        ..Default::default()
    };
    let _: UserSettings = json(
        server
            .request(Method::PUT, "/settings", &b)
            .json(&settings)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let mut observer = server.socket(&a).await;
    let mut reader = server.socket(&b).await;
    let message = server
        .message(&a, conversation.id, "Mensagem privada")
        .await;
    let read_route = format!("/conversations/{}/read", conversation.id);
    assert_eq!(
        server
            .request(Method::POST, &read_route, &b)
            .json(&ReadRequest {
                message_id: Some(message.id)
            })
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    send_event(
        &mut reader,
        &ClientEvent::Typing {
            conversation_id: conversation.id,
            is_typing: true,
        },
    )
    .await;
    let conversations: Vec<Conversation> = json(
        server
            .request(Method::GET, "/conversations", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        conversations
            .iter()
            .find(|c| c.id == conversation.id)
            .unwrap()
            .unread,
        0
    );
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(600);
    while let Ok(event) = tokio::time::timeout_at(deadline, next_event(&mut observer)).await {
        assert!(
            !matches!(event, ServerEvent::Read { user_id, .. } | ServerEvent::Typing { user_id, .. } if user_id == b.user.id)
        );
    }
    settings.read_receipts = true;
    settings.typing_indicators = true;
    let _: UserSettings = json(
        server
            .request(Method::PUT, "/settings", &b)
            .json(&settings)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        server
            .request(Method::POST, &read_route, &b)
            .json(&ReadRequest {
                message_id: Some(message.id)
            })
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    event_matching(
        &mut observer,
        |event| matches!(event, ServerEvent::Read { user_id, .. } if *user_id == b.user.id),
    )
    .await;
    send_event(
        &mut reader,
        &ClientEvent::Typing {
            conversation_id: conversation.id,
            is_typing: true,
        },
    )
    .await;
    event_matching(&mut observer, |event| matches!(event, ServerEvent::Typing { user_id, is_typing: true, .. } if *user_id == b.user.id)).await;
    drop((reader, observer));
    server.shutdown().await;
}
