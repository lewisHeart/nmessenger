mod common;
use common::TestServer;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

#[actix_web::test]
async fn signed_release_is_persisted_and_replays_are_idempotent() {
    // This integration-test binary has one test and its own process environment.
    unsafe { std::env::set_var("GITHUB_WEBHOOK_SECRET", "0123456789abcdef0123456789abcdef"); }
    let Some(server) = TestServer::start().await else { return; };
    let account = server.register("Release test").await;
    sqlx::query("DELETE FROM github_releases").execute(&server.state.db).await.unwrap();
    let tag = format!("v0.1.{}", uuid::Uuid::new_v4().as_u128());
    let event = serde_json::json!({
        "action":"published", "repository":{"full_name":"lewisHeart/nmessenger"},
        "release":{"tag_name":tag,"html_url":format!("https://github.com/lewisHeart/nmessenger/releases/tag/{tag}"),
            "published_at":"2026-01-01T00:00:00Z","draft":false,"prerelease":false,"assets":[]}
    }).to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(b"0123456789abcdef0123456789abcdef").unwrap();
    mac.update(event.as_bytes());
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    let url = format!("{}/v1/webhooks/github", server.url);
    let mut events = server.state.events.subscribe();
    let rejected = server.client.post(&url).header("X-GitHub-Event", "release")
        .header("X-Hub-Signature-256", "sha256=00").body(event.clone()).send().await.unwrap();
    assert_eq!(rejected.status(), 403);
    for _ in 0..2 {
        let response = server.client.post(&url).header("X-GitHub-Event", "release")
            .header("X-Hub-Signature-256", &signature).body(event.clone()).send().await.unwrap();
        assert_eq!(response.status(), 204);
    }
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(event.event, nm_protocol::ServerEvent::ReleaseChanged) { break event; }
        }
    }).await.expect("release event was not broadcast");
    assert!(event.recipients.is_empty());
    let latest = server.request(reqwest::Method::GET, "/releases/latest", &account).send().await.unwrap();
    assert_eq!(latest.status(), 200);
    assert_eq!(latest.json::<serde_json::Value>().await.unwrap()["tag_name"], tag);
}
