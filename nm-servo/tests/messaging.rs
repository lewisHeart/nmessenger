mod common;

use common::*;
use nm_protocol::*;
use reqwest::Method;
use uuid::Uuid;

#[actix_web::test]
async fn durable_offline_messages_cursor_search_idempotence_unread_and_cooldown() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let a = server.register("Alice").await;
    let b = server.register("Bob offline").await;
    server.befriend(&a, &b).await;
    let first = server.direct(&a, &b).await;
    let reverse = server.direct(&b, &a).await;
    assert_eq!(first.id, reverse.id);
    let route = format!("/conversations/{}/messages", first.id);
    let request = send_request("Primeira mensagem offline");
    let (left, right) = tokio::join!(
        server
            .request(Method::POST, &route, &a)
            .json(&request)
            .send(),
        server
            .request(Method::POST, &route, &a)
            .json(&request)
            .send()
    );
    let left: Message = json(left.unwrap()).await;
    let right: Message = json(right.unwrap()).await;
    assert_eq!(left.id, right.id);
    let second = server.message(&a, first.id, "Agulha de busca").await;
    let third = server.message(&a, first.id, "Terceira mensagem").await;
    let bootstrap: Bootstrap = json(
        server
            .request(Method::GET, "/bootstrap", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(bootstrap.conversations[0].unread, 3);
    assert_eq!(
        bootstrap.conversations[0].last_message.as_ref().unwrap().id,
        third.id
    );
    let latest: Vec<Message> = json(
        server
            .request(Method::GET, &format!("{route}?limit=2"), &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        latest.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![second.id, third.id]
    );
    let older: Vec<Message> = json(
        server
            .request(
                Method::GET,
                &format!("{route}?before={}&limit=2", second.id),
                &b,
            )
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(older.len(), 1);
    assert_eq!(older[0].id, left.id);
    let search: Vec<Message> = json(
        server
            .request(Method::GET, &route, &b)
            .query(&[("q", "AGULHA")])
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(search.len(), 1);
    assert_eq!(search[0].id, second.id);
    error(
        server
            .request(
                Method::GET,
                &format!("{route}?before={}", Uuid::new_v4()),
                &b,
            )
            .send()
            .await
            .unwrap(),
        404,
    )
    .await;
    error(
        server
            .request(Method::GET, &format!("{route}?limit=0"), &b)
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;

    assert_eq!(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/read", first.id),
                &b
            )
            .json(&ReadRequest {
                message_id: Some(second.id)
            })
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let convs: Vec<Conversation> = json(
        server
            .request(Method::GET, "/conversations", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(convs[0].unread, 1);
    // An old read receipt cannot move the read marker backwards.
    assert_eq!(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/read", first.id),
                &b
            )
            .json(&ReadRequest {
                message_id: Some(left.id)
            })
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let convs: Vec<Conversation> = json(
        server
            .request(Method::GET, "/conversations", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(convs[0].unread, 1);

    for kind in [MessageKind::System, MessageKind::Activity] {
        let mut forged = send_request("forged");
        forged.kind = kind;
        error(
            server
                .request(Method::POST, &route, &a)
                .json(&forged)
                .send()
                .await
                .unwrap(),
            400,
        )
        .await;
    }
    let mut nudge = send_request("");
    nudge.kind = MessageKind::Nudge;
    let sent: Message = json(
        server
            .request(Method::POST, &route, &a)
            .json(&nudge)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let retry: Message = json(
        server
            .request(Method::POST, &route, &a)
            .json(&nudge)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(sent.id, retry.id);
    nudge.client_id = Uuid::new_v4();
    let limited = error(
        server
            .request(Method::POST, &route, &a)
            .json(&nudge)
            .send()
            .await
            .unwrap(),
        429,
    )
    .await;
    assert_eq!(limited.code, "rate_limited");

    let other = server.group(&a, &[&b]).await;
    error(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/messages", other.id),
                &a,
            )
            .json(&request)
            .send()
            .await
            .unwrap(),
        409,
    )
    .await;
    let foreign = server.message(&a, other.id, "Other conversation").await;
    error(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/read", first.id),
                &b,
            )
            .json(&ReadRequest {
                message_id: Some(foreign.id),
            })
            .send()
            .await
            .unwrap(),
        404,
    )
    .await;
    assert_eq!(
        server
            .request(
                Method::DELETE,
                &format!("/conversations/{}/history", first.id),
                &b
            )
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let hidden: Vec<Message> = json(
        server
            .request(Method::GET, &route, &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert!(hidden.is_empty());
    let retained: Vec<Message> = json(
        server
            .request(Method::GET, &route, &a)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(retained.len(), 4);
    let newest = server.message(&a, first.id, "Após limpar histórico").await;
    let visible: Vec<Message> = json(
        server
            .request(Method::GET, &route, &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, newest.id);
    server.shutdown().await;
}
