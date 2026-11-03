mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;
use uuid::Uuid;

#[actix_web::test]
async fn invalid_login_attempts_are_counted_against_the_ip_limit() {
    let Some(server) = TestServer::start().await else { return; };
    for attempt in 0..31 {
        let response = server.client.post(format!("{}/v1/auth/login", server.url))
            .json(&LoginRequest {
                email: "invalid".into(),
                password: "invalid".into(),
                device_name: "test".into(),
            })
            .send().await.unwrap();
        assert_eq!(response.status().as_u16(), if attempt < 30 { 400 } else { 429 });
    }
}

#[actix_web::test]
async fn moderation_blocks_registration_and_anonymizes_without_erasing_messages() {
    let Some(server) = TestServer::start().await else { return; };
    let admin = server.register("Administrator").await;
    let user = server.register("User").await;
    sqlx::query("INSERT INTO app_admins(user_id) VALUES($1)").bind(admin.user.id).execute(&server.state.db).await.unwrap();
    let conversation = Uuid::new_v4();
    let message = Uuid::new_v4();
    sqlx::query("INSERT INTO conversations(id,kind,owner_id,direct_key) VALUES($1,'direct',$2,$3)")
        .bind(conversation).bind(user.user.id).bind(Uuid::new_v4().to_string()).execute(&server.state.db).await.unwrap();
    sqlx::query("INSERT INTO messages(id,conversation_id,sender_id,client_id,kind,body,style) VALUES($1,$2,$3,$4,'text','preservada','{}')")
        .bind(message).bind(conversation).bind(user.user.id).bind(Uuid::new_v4()).execute(&server.state.db).await.unwrap();
    let route = format!("/admin/users/{}/moderate", user.user.id);
    for banned in [Some(true), Some(false), None] {
        let response = server.request(Method::POST, &route, &admin)
            .json(&serde_json::json!({"banned":banned,"confirmation_password":"test password 123"}))
            .send().await.unwrap();
        assert!(response.status().is_success(), "{}", response.text().await.unwrap());
        if banned == Some(true) {
            let response = server.client.post(format!("{}/v1/auth/register", server.url))
                .json(&RegisterRequest { email:user.user.email.clone(), password:"test password 123".into(), display_name:"Outra conta".into(), device_name:"test".into() })
                .send().await.unwrap();
            assert_eq!(response.status(), 403);
            assert!(!server.request(Method::GET, "/bootstrap", &user).send().await.unwrap().status().is_success());
        }
    }
    let row: (String, String, bool) = sqlx::query_as("SELECT email,display_name,deleted_at IS NOT NULL FROM users WHERE id=$1")
        .bind(user.user.id).fetch_one(&server.state.db).await.unwrap();
    assert_ne!(row.0, user.user.email);
    assert_eq!(row.1, "Usuário excluído");
    assert!(row.2);
    let body: String = sqlx::query_scalar("SELECT body FROM messages WHERE id=$1").bind(message).fetch_one(&server.state.db).await.unwrap();
    assert_eq!(body, "preservada");
    server.shutdown().await;
}

#[actix_web::test]
async fn administrator_password_reset_requires_confirmation_and_revokes_sessions() {
    let Some(server) = TestServer::start().await else { return; };
    let admin = server.register("Administrator").await;
    let user = server.register("User").await;
    sqlx::query("INSERT INTO app_admins(user_id) VALUES($1)")
        .bind(admin.user.id).execute(&server.state.db).await.unwrap();
    let route = format!("/admin/users/{}/password", user.user.id);
    let request = |confirmation: &str| serde_json::json!({"password":"replacement password", "confirmation_password": confirmation});
    let denied = server.request(Method::POST, &route, &admin).json(&request("wrong password"))
        .send().await.unwrap();
    assert!(!denied.status().is_success());
    let unauthorized = server.request(Method::POST, &route, &user).json(&request("test password 123"))
        .send().await.unwrap();
    assert!(!unauthorized.status().is_success());
    let success = server.request(Method::POST, &route, &admin).json(&request("test password 123"))
        .send().await.unwrap();
    assert!(success.status().is_success());
    assert!(!server.request(Method::GET, "/bootstrap", &user).send().await.unwrap().status().is_success());
    for (password, expected) in [("test password 123", false), ("replacement password", true)] {
        let response = server.client.post(format!("{}/v1/auth/login", server.url)).json(&LoginRequest {
            email: user.user.email.clone(), password: password.into(), device_name: "test".into(),
        }).send().await.unwrap();
        assert_eq!(response.status().is_success(), expected);
    }
    server.shutdown().await;
}

#[actix_web::test]
async fn security_headers_storage_quota_and_websocket_connection_limit_are_enforced() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let user = server.register("Security limits").await;
    let response = server
        .request(Method::GET, "/bootstrap", &user)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["x-frame-options"], "DENY");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    let mut remaining = server.state.user_upload_quota;
    while remaining > 0 {
        let size = remaining.min(MAX_UPLOAD_BYTES as i64);
        sqlx::query("INSERT INTO files(id,owner_id,filename,mime,size,sha256) VALUES($1,$2,'quota-fixture','application/octet-stream',$3,$4)")
            .bind(Uuid::new_v4()).bind(user.user.id).bind(size).bind("0".repeat(64)).execute(&server.state.db).await.unwrap();
        remaining -= size;
    }
    let failure = error(
        server
            .upload(
                &user,
                None,
                "over-quota.txt",
                b"one byte too much".to_vec(),
                "text/plain",
            )
            .await,
        400,
    )
    .await;
    assert!(failure.message.contains("Cota"));
    let mut sockets = Vec::new();
    for _ in 0..8 {
        sockets.push(server.socket(&user).await);
    }
    let url = format!("{}/v1/events", server.url.replacen("http://", "ws://", 1));
    let (mut denied, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send_event(
        &mut denied,
        &ClientEvent::Authenticate {
            token: user.token.clone(),
        },
    )
    .await;
    assert!(
        matches!(next_event(&mut denied).await, ServerEvent::Error { code, .. } if code == "rate_limited")
    );
    drop(denied);
    drop(sockets);
    server.shutdown().await;
}
