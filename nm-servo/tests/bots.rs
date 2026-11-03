mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;
use uuid::Uuid;

#[actix_web::test]
async fn bot_owner_can_create_and_manage_isolated_bot_account() {
    let Some(server) = TestServer::start().await else { return; };
    let owner = server.register("Bot owner").await;
    let other = server.register("Other user").await;
    let username = format!("bot_{}", &Uuid::new_v4().simple().to_string()[..16]);
    let response = server.request(Method::POST, "/bots", &owner)
        .json(&serde_json::json!({"username":username,"name":"Teste"})).send().await.unwrap();
    let created: serde_json::Value = json(response).await;
    let bot_id = Uuid::parse_str(created["bot"]["id"].as_str().unwrap()).unwrap();
    let token = created["token"].as_str().unwrap();
    let address = created["bot"]["address"].as_str().unwrap();
    let response = server.client.get(format!("{}/v1/bootstrap", server.url)).bearer_auth(token).send().await.unwrap();
    assert!(response.status().is_success());
    let rejected = server.request(Method::PATCH, &format!("/bots/{bot_id}"), &other)
        .json(&serde_json::json!({"name":"Invasão"})).send().await.unwrap();
    assert!(!rejected.status().is_success());
    let contact: Contact = json(server.request(Method::POST, "/contacts", &owner)
        .json(&AddContactRequest { email: address.into(), nickname: None }).send().await.unwrap()).await;
    assert_eq!(contact.state, ContactState::Accepted);
    let conversation: Conversation = json(server.request(Method::POST, "/conversations", &owner)
        .json(&CreateConversationRequest { member_ids: vec![bot_id], title: None }).send().await.unwrap()).await;
    let plaintext = server.client.post(format!("{}/v1/conversations/{}/messages", server.url, conversation.id))
        .bearer_auth(token).json(&send_request("sem criptografia")).send().await.unwrap();
    assert_eq!(plaintext.status(), 400);
    let owner_plaintext = server.request(Method::POST, &format!("/conversations/{}/messages", conversation.id), &owner)
        .json(&send_request("mensagem antiga sem cifragem")).send().await.unwrap();
    assert_eq!(owner_plaintext.status(), 400);
    let upload = server.upload(&owner, Some(conversation.id), "test.txt", b"conteudo".to_vec(), "text/plain").await;
    assert_eq!(upload.status(), 400);
    let bots: Vec<serde_json::Value> = json(server.request(Method::GET, "/bots", &owner).send().await.unwrap()).await;
    assert_eq!(bots.len(), 1);
    assert!(bots[0].get("token").is_none());
    let forbidden = server.request(Method::POST, &format!("/bots/{bot_id}/token"), &other).send().await.unwrap();
    assert_eq!(forbidden.status(), 404);
    let rotated: serde_json::Value = json(server.request(Method::POST, &format!("/bots/{bot_id}/token"), &owner).send().await.unwrap()).await;
    assert_ne!(rotated["token"].as_str().unwrap(), token);
    let revoked = server.client.get(format!("{}/v1/bootstrap", server.url)).bearer_auth(token).send().await.unwrap();
    assert_eq!(revoked.status(), 401);
    sqlx::query("DELETE FROM users WHERE id=$1").bind(bot_id).execute(&server.state.db).await.unwrap();
    server.shutdown().await;
}
