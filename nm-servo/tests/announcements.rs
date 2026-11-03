mod common;
use common::*;
use nm_protocol::*;
use reqwest::Method;
use uuid::Uuid;

#[actix_web::test]
async fn announcements_require_admin_and_publish_edit_pause_and_delete_across_websocket() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let admin = server.register("Editor de anúncios").await;
    let viewer = server.register("Pessoa sem privilégios").await;
    sqlx::query("INSERT INTO announcement_admins(email) VALUES($1)")
        .bind(&admin.user.email)
        .execute(&server.state.db)
        .await
        .unwrap();
    let get_feed = |auth: &AuthResponse| server.request(Method::GET, "/announcements", auth);
    let feed: AnnouncementFeed = json(get_feed(&viewer).send().await.unwrap()).await;
    assert!(!feed.can_manage);
    let feed: AnnouncementFeed = json(get_feed(&admin).send().await.unwrap()).await;
    assert!(
        !feed.can_manage,
        "unverified email must not grant administration"
    );
    sqlx::query("INSERT INTO app_admins(user_id) VALUES($1)")
        .bind(admin.user.id)
        .execute(&server.state.db)
        .await
        .unwrap();
    let feed: AnnouncementFeed = json(get_feed(&admin).send().await.unwrap()).await;
    assert!(feed.can_manage);
    error(
        server
            .client
            .get(format!("{}/v1/announcements", server.url))
            .send()
            .await
            .unwrap(),
        401,
    )
    .await;
    let image: FileInfo = json(
        server
            .upload(
                &admin,
                None,
                "banner.png",
                include_bytes!("../../nmessenger-client/assets/nmessenger/avatars/daisy.png").to_vec(),
                "image/png",
            )
            .await,
    )
    .await;
    let mut draft = SaveAnnouncementRequest {
        title: "Novidades do NMessenger".into(),
        body: "Imagem, texto e link publicados pelo administrador.".into(),
        link_url: Some("https://example.com/news".into()),
        image_file_id: Some(image.id),
        full_banner: false,
        enabled: false,
    };
    error(
        server
            .request(Method::POST, "/announcements", &viewer)
            .json(&draft)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let item: Announcement = json(
        server
            .request(Method::POST, "/announcements", &admin)
            .json(&draft)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let route = format!("/announcements/{}", item.id);
    let image_route = format!("/files/{}", image.id);
    let feed: AnnouncementFeed = json(get_feed(&viewer).send().await.unwrap()).await;
    assert!(!feed.items.iter().any(|a| a.id == item.id));
    error(
        server
            .request(Method::GET, &image_route, &viewer)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    for method in [Method::PUT, Method::DELETE] {
        error(
            server
                .request(method, &route, &viewer)
                .json(&draft)
                .send()
                .await
                .unwrap(),
            403,
        )
        .await;
    }
    let mut socket = server.socket(&viewer).await;
    draft.enabled = true;
    draft.title = "Título alterado e publicado".into();
    let updated: Announcement = json(
        server
            .request(Method::PUT, &route, &admin)
            .json(&draft)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(updated.id, item.id);
    assert_eq!(updated.title, draft.title);
    event_matching(&mut socket, |event| {
        matches!(event, ServerEvent::AnnouncementsChanged)
    })
    .await;
    let feed: AnnouncementFeed = json(get_feed(&viewer).send().await.unwrap()).await;
    assert!(feed.items.iter().any(|a| a.id == item.id && a.enabled));
    assert_eq!(
        server
            .request(Method::GET, &image_route, &viewer)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let mut invalid = draft.clone();
    invalid.link_url = Some("javascript:alert(1)".into());
    error(
        server
            .request(Method::PUT, &route, &admin)
            .json(&invalid)
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;
    invalid.link_url = Some("https://user:password@example.com".into());
    error(
        server
            .request(Method::PUT, &route, &admin)
            .json(&invalid)
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;
    let private: FileInfo = json(
        server
            .upload(
                &viewer,
                None,
                "private.png",
                include_bytes!("../../nmessenger-client/assets/nmessenger/avatars/daisy.png").to_vec(),
                "image/png",
            )
            .await,
    )
    .await;
    invalid = draft.clone();
    invalid.image_file_id = Some(private.id);
    error(
        server
            .request(Method::PUT, &route, &admin)
            .json(&invalid)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    // A public banner cannot later be converted into a private chat attachment.
    server.befriend(&admin, &viewer).await;
    let conversation = server.direct(&admin, &viewer).await;
    let mut attachment = send_request("banner");
    attachment.kind = MessageKind::Photo;
    attachment.attachment_id = Some(image.id);
    error(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/messages", conversation.id),
                &admin,
            )
            .json(&attachment)
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;
    draft.enabled = false;
    let _: Announcement = json(
        server
            .request(Method::PUT, &route, &admin)
            .json(&draft)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let feed: AnnouncementFeed = json(get_feed(&viewer).send().await.unwrap()).await;
    assert!(!feed.items.iter().any(|a| a.id == item.id));
    error(
        server
            .request(Method::GET, &image_route, &viewer)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    assert_eq!(
        server
            .request(Method::DELETE, &route, &admin)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let feed: AnnouncementFeed = json(get_feed(&admin).send().await.unwrap()).await;
    assert!(!feed.items.iter().any(|a| a.id == item.id));
    error(
        server
            .request(
                Method::PUT,
                &format!("/announcements/{}", Uuid::new_v4()),
                &admin,
            )
            .json(&SaveAnnouncementRequest {
                title: "Inexistente".into(),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
        404,
    )
    .await;
    sqlx::query("DELETE FROM announcement_admins WHERE email=$1")
        .bind(&admin.user.email)
        .execute(&server.state.db)
        .await
        .unwrap();
    drop(socket);
    server.shutdown().await;
}
