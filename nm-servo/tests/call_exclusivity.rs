mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn calls_are_exclusive_across_conversations_and_concurrent_accepts() {
    let Some(server) = TestServer::start().await else { return; };
    let alice = server.register("Caller A").await;
    let bob = server.register("Recipient B").await;
    let carol = server.register("Caller C").await;
    server.befriend(&alice, &bob).await;
    server.befriend(&bob, &carol).await;
    let ab = server.direct(&alice, &bob).await;
    let bc = server.direct(&bob, &carol).await;
    let offer = |auth: &AuthResponse, conversation, kind| server.request(Method::POST, &format!("/conversations/{conversation}/activities"), auth)
        .json(&ActivityRequest { kind, payload: serde_json::json!({}) });
    let first: Activity = json(offer(&alice, ab.id, ActivityKind::AudioCall).send().await.unwrap()).await;
    let duplicate = offer(&alice, ab.id, ActivityKind::VideoCall).send().await.unwrap();
    assert_eq!(duplicate.status(), 409);
    let second: Activity = json(offer(&carol, bc.id, ActivityKind::VideoCall).send().await.unwrap()).await;
    let accept = |id| server.request(Method::PATCH, &format!("/activities/{id}"), &bob)
        .json(&UpdateActivityRequest { status: ActivityStatus::Accepted, payload: None });
    let (a, b) = tokio::join!(accept(first.id).send(), accept(second.id).send());
    let (a, b) = (a.unwrap().status(), b.unwrap().status());
    assert!((a.is_success() && b.as_u16() == 409) || (b.is_success() && a.as_u16() == 409), "{a}, {b}");
    let (accepted, pending) = if a.is_success() { (first.id, second.id) } else { (second.id, first.id) };
    let ended = server.request(Method::PATCH, &format!("/activities/{accepted}"), &bob)
        .json(&UpdateActivityRequest { status: ActivityStatus::Ended, payload: None }).send().await.unwrap();
    assert!(ended.status().is_success());
    assert!(accept(pending).send().await.unwrap().status().is_success());
    server.shutdown().await;
}
