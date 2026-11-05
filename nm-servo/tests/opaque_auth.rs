mod common;
use common::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::{AuthResponse, secure_auth::{CipherSuite, opaque_ke}};
use opaque_ke::{ClientLogin, ClientLoginFinishParameters, ClientRegistration, ClientRegistrationFinishParameters, RegistrationResponse, CredentialResponse};
use reqwest::Method;

#[actix_web::test]
async fn opaque_login_authenticates_without_sending_the_password_and_rejects_replay() {
    let Some(server) = TestServer::start().await else { return; };
    let alice = server.register("Alice").await;
    let mut rng = opaque_ke::rand::rngs::OsRng;
    let registration = ClientRegistration::<CipherSuite>::start(&mut rng, b"test password 123").unwrap();
    let response = server.request(Method::POST, "/auth/opaque/register/start", &alice)
        .json(&serde_json::json!({"request":STANDARD.encode(registration.message.serialize())})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    let response = RegistrationResponse::<CipherSuite>::deserialize(&STANDARD.decode(body["response"].as_str().unwrap()).unwrap()).unwrap();
    let registration = registration.state.finish(&mut rng, b"test password 123", response, ClientRegistrationFinishParameters::default()).unwrap();
    let reply = server.request(Method::POST, "/auth/opaque/register/finish", &alice)
        .json(&serde_json::json!({"record":STANDARD.encode(registration.message.serialize())})).send().await.unwrap();
    assert_eq!(reply.status(), 204);
    let legacy = server.client.post(format!("{}/v1/auth/login", server.url))
        .json(&serde_json::json!({"email":alice.user.email,"password":"test password 123","device_name":"old client"}))
        .send().await.unwrap();
    assert_eq!(legacy.status(), 409);
    let login = ClientLogin::<CipherSuite>::start(&mut rng, b"test password 123").unwrap();
    let response = server.client.post(format!("{}/v1/auth/opaque/login/start", server.url))
        .json(&serde_json::json!({"email":alice.user.email,"device_name":"new device","request":STANDARD.encode(login.message.serialize())})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let response: serde_json::Value = response.json().await.unwrap();
    let challenge = response["challenge"].as_str().unwrap().to_owned();
    let message = CredentialResponse::<CipherSuite>::deserialize(&STANDARD.decode(response["response"].as_str().unwrap()).unwrap()).unwrap();
    let finish = login.state.finish(b"test password 123", message, ClientLoginFinishParameters::default()).unwrap();
    assert_eq!(registration.export_key, finish.export_key);
    let payload = serde_json::json!({"challenge":challenge,"finalization":STANDARD.encode(finish.message.serialize())});
    let reply = server.client.post(format!("{}/v1/auth/opaque/login/finish", server.url)).json(&payload).send().await.unwrap();
    assert_eq!(reply.status(), 200);
    let logged: AuthResponse = reply.json().await.unwrap();
    assert_eq!(logged.user.id, alice.user.id);
    let replay = server.client.post(format!("{}/v1/auth/opaque/login/finish", server.url)).json(&payload).send().await.unwrap();
    assert_eq!(replay.status(), 401);
}
