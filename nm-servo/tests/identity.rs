mod common;

use common::*;
use nm_protocol::*;
use reqwest::Method;
use uuid::Uuid;

#[actix_web::test]
async fn auth_sessions_profile_bootstrap_and_consistent_errors() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let readiness: serde_json::Value = json(
        server
            .client
            .get(format!("{}/v1/health", server.url))
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(readiness["ready"], true);
    let a = server.register("Alice").await;
    let b = server.register("Bob").await;
    error(
        server
            .client
            .get(format!("{}/v1/bootstrap", server.url))
            .send()
            .await
            .unwrap(),
        401,
    )
    .await;
    error(
        server
            .client
            .post(format!("{}/v1/auth/login", server.url))
            .json(&LoginRequest {
                email: a.user.email.clone(),
                password: "wrong password".into(),
                device_name: "Wrong".into(),
            })
            .send()
            .await
            .unwrap(),
        401,
    )
    .await;

    let hash: (String, String) = sqlx::query_as(
        "SELECT u.password_hash,s.token_hash FROM users u
        JOIN auth_sessions s ON s.user_id=u.id WHERE s.id=$1",
    )
    .bind(a.session_id)
    .fetch_one(&server.state.db)
    .await
    .unwrap();
    assert!(hash.0.starts_with("$argon2id$"));
    assert_ne!(hash.1, a.token);
    assert_eq!(hash.1, nm_servo::auth::digest(&a.token));
    let ttl: i64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM (expires_at-created_at))::bigint
        FROM auth_sessions WHERE id=$1",
    )
    .bind(a.session_id)
    .fetch_one(&server.state.db)
    .await
    .unwrap();
    assert!((3595..=3605).contains(&ttl));

    let second = server.login(&a, "Notebook").await;
    assert_ne!(second.session_id, a.session_id);
    let sessions: Vec<SessionInfo> = json(
        server
            .request(Method::GET, "/sessions", &a)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions.iter().filter(|s| s.current).count(), 1);
    assert!(sessions.iter().any(|s| s.id == a.session_id && s.current));
    error(
        server
            .request(Method::DELETE, &format!("/sessions/{}", a.session_id), &b)
            .send()
            .await
            .unwrap(),
        404,
    )
    .await;

    let _: Profile = json(
        server
            .request(Method::PATCH, "/me", &a)
            .json(&UpdateProfileRequest {
                display_name: Some("Alice Nova".into()),
                personal_message: Some("Teste".into()),
                presence: Some(Presence::Invisible),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let settings = UserSettings {
        theme: "verde".into(),
        allow_strangers: true,
        ..Default::default()
    };
    let saved: UserSettings = json(
        server
            .request(Method::PUT, "/settings", &a)
            .json(&settings)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(saved.theme, "verde");
    let bootstrap: Bootstrap = json(
        server
            .request(Method::GET, "/bootstrap", &a)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(bootstrap.me.display_name, "Alice Nova");
    assert_eq!(bootstrap.me.presence, Presence::Invisible);
    assert_eq!(bootstrap.settings.theme, "verde");
    assert_eq!(bootstrap.sessions.len(), 2);

    assert_eq!(
        server
            .request(
                Method::DELETE,
                &format!("/sessions/{}", second.session_id),
                &a
            )
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    error(
        server
            .request(Method::GET, "/bootstrap", &second)
            .send()
            .await
            .unwrap(),
        401,
    )
    .await;
    let revoked: bool = redis::cmd("EXISTS")
        .arg(format!("nm:revoked:{}", second.session_id))
        .query_async(&mut server.state.redis.clone())
        .await
        .unwrap();
    assert!(revoked);
    error(
        server
            .request(Method::PATCH, "/contacts/not-a-uuid", &a)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;
    error(
        server
            .request(Method::POST, "/settings", &a)
            .send()
            .await
            .unwrap(),
        405,
    )
    .await;
    error(
        server
            .request(Method::GET, &format!("/files/{}", Uuid::new_v4()), &a)
            .send()
            .await
            .unwrap(),
        404,
    )
    .await;
    error(
        server
            .request(Method::PATCH, "/me", &a)
            .header("Content-Type", "application/json")
            .body("{broken")
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;
    assert_eq!(
        server
            .request(Method::POST, "/auth/logout", &a)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    error(
        server
            .request(Method::GET, "/sessions", &a)
            .send()
            .await
            .unwrap(),
        401,
    )
    .await;
    server.shutdown().await;
}
