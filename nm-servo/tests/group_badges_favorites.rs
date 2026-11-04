mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn owner_transfer_is_authorized_and_favorites_are_limited() {
    let Some(server) = TestServer::start().await else { return; };
    let owner = server.register("Owner").await;
    let mut users = Vec::new();
    for _ in 0..8 { users.push(server.register("Member").await); }
    server.befriend(&owner, &users[0]).await;
    let group = server.group(&owner, &[&users[0]]).await;
    let route = format!("/conversations/{}/owner", group.id);
    let denied = server.request(Method::PUT, &route, &users[0]).json(&MemberRequest { user_id: users[0].user.id }).send().await.unwrap();
    assert_eq!(denied.status(), 403);
    let updated: Conversation = json(server.request(Method::PUT, &route, &owner).json(&MemberRequest { user_id: users[0].user.id }).send().await.unwrap()).await;
    assert_eq!(updated.owner_id, Some(users[0].user.id));
    let denied = server.request(Method::DELETE, &format!("/conversations/{}/members/{}", group.id, users[0].user.id), &owner).send().await.unwrap();
    assert_eq!(denied.status(), 403);
    for user in &users[1..] {
        sqlx::query("INSERT INTO contacts(owner_id,user_id,state) VALUES($1,$2,'accepted') ON CONFLICT DO NOTHING")
            .bind(owner.user.id).bind(user.user.id).execute(&server.state.db).await.unwrap();
    }
    for (index, user) in users.iter().enumerate() {
        let response = server.request(Method::PATCH, &format!("/contacts/{}", user.user.id), &owner)
            .json(&UpdateContactRequest { favorite: Some(true), ..Default::default() }).send().await.unwrap();
        assert_eq!(response.status().as_u16(), if index < 7 { 200 } else { 409 });
    }
}

#[actix_web::test]
async fn founder_badge_applies_only_to_november_2026() {
    let Some(server) = TestServer::start().await else { return; };
    let user = server.register("Founder").await;
    sqlx::query("UPDATE users SET created_at='2026-11-15 00:00:00+00' WHERE id=$1").bind(user.user.id).execute(&server.state.db).await.unwrap();
    let badges = nm_servo::badges::for_user(&server.state.db, user.user.id).await.unwrap();
    assert!(badges.iter().any(|badge| badge.slug == "fundadores"));
    sqlx::query("DELETE FROM user_badges WHERE user_id=$1").bind(user.user.id).execute(&server.state.db).await.unwrap();
    sqlx::query("UPDATE users SET created_at='2026-12-01 00:00:00+00' WHERE id=$1").bind(user.user.id).execute(&server.state.db).await.unwrap();
    let badges = nm_servo::badges::for_user(&server.state.db, user.user.id).await.unwrap();
    assert!(!badges.iter().any(|badge| badge.slug == "fundadores"));
}
