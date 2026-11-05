mod common;
use common::*;
use reqwest::Method;

#[actix_web::test]
async fn history_backup_is_account_scoped_and_rejects_stale_writes() {
    let Some(server) = TestServer::start().await else { return; };
    let alice = server.register("Alice").await;
    let bob = server.register("Bob").await;
    let route = "/encryption/history-backup";
    assert_eq!(server.request(Method::GET, route, &alice).send().await.unwrap().status(), 204);
    let response = server.request(Method::PUT, route, &alice)
        .json(&serde_json::json!({"revision":0,"ciphertext":"opaque-ciphertext"})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<serde_json::Value>().await.unwrap()["revision"], 1);
    assert_eq!(server.request(Method::GET, route, &bob).send().await.unwrap().status(), 204);
    let stale = server.request(Method::PUT, route, &alice)
        .json(&serde_json::json!({"revision":0,"ciphertext":"stale"})).send().await.unwrap();
    assert_eq!(stale.status(), 409);
    let latest = server.request(Method::GET, route, &alice).send().await.unwrap().json::<serde_json::Value>().await.unwrap();
    assert_eq!(latest["ciphertext"], "opaque-ciphertext");
}
