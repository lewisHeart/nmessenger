mod common;

use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn membership_blocks_upload_authorization_and_public_assets() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let a = server.register("Alice").await;
    let b = server.register("Bob").await;
    let c = server.register("Carol").await;
    server.befriend(&a, &b).await;
    server.befriend(&a, &c).await;
    server.befriend(&b, &c).await;
    let group = server.group(&a, &[&b]).await;
    let history = format!("/conversations/{}/messages", group.id);
    let _ = server.message(&a, group.id, "Private group history").await;
    error(
        server
            .request(Method::GET, &history, &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .upload(
                &c,
                Some(group.id),
                "denied.txt",
                b"denied".to_vec(),
                "text/plain",
            )
            .await,
        403,
    )
    .await;
    error(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/members", group.id),
                &b,
            )
            .json(&MemberRequest { user_id: c.user.id })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;

    let file: FileInfo = json(
        server
            .upload(
                &a,
                Some(group.id),
                "../../secret.txt",
                b"private bytes".to_vec(),
                "text/plain",
            )
            .await,
    )
    .await;
    assert_eq!(file.filename, "secret.txt");
    assert_eq!(
        file.sha256,
        "1d4fec327a6cac6f1521d1b5321f55136d0472499d8482c4115eeacffaff140f"
    );
    // Compare actual bytes as well as metadata authorization.
    let downloaded = server
        .request(Method::GET, &format!("/files/{}", file.id), &b)
        .send()
        .await
        .unwrap();
    assert!(downloaded.status().is_success());
    assert_eq!(downloaded.bytes().await.unwrap().as_ref(), b"private bytes");
    error(
        server
            .request(Method::GET, &format!("/files/{}/info", file.id), &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let mut attachment = send_request("Arquivo");
    attachment.kind = MessageKind::File;
    attachment.attachment_id = Some(file.id);
    let _: Message = json(
        server
            .request(Method::POST, &history, &a)
            .json(&attachment)
            .send()
            .await
            .unwrap(),
    )
    .await;
    attachment.client_id = uuid::Uuid::new_v4();
    error(
        server
            .request(Method::POST, &history, &b)
            .json(&attachment)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .request(Method::POST, "/assets", &a)
            .json(&CreateAssetRequest {
                kind: AssetKind::Background,
                name: "Must stay private".into(),
                shortcut: None,
                file_id: file.id,
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;

    let _: Conversation = json(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/members", group.id),
                &a,
            )
            .json(&MemberRequest { user_id: c.user.id })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let _: Vec<Message> = json(
        server
            .request(Method::GET, &history, &c)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        server
            .request(
                Method::DELETE,
                &format!("/conversations/{}/members/{}", group.id, b.user.id),
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
            .request(Method::GET, &history, &b)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .request(Method::GET, &format!("/files/{}", file.id), &b)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .request(Method::POST, &history, &b)
            .json(&send_request("removed"))
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/activities", group.id),
                &b,
            )
            .json(&ActivityRequest {
                kind: ActivityKind::Game,
                payload: serde_json::json!({"game":"tic_tac_toe"}),
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;

    // Public assets can only be made from the owner's unscoped uploads.
    let image: FileInfo = json(
        server
            .upload(
                &a,
                None,
                "avatar.png",
                b"\x89PNG\r\n\x1a\nimage".to_vec(),
                "image/png",
            )
            .await,
    )
    .await;
    error(
        server
            .request(Method::GET, &format!("/files/{}", image.id), &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let asset: UserAsset = json(
        server
            .request(Method::POST, "/assets", &a)
            .json(&CreateAssetRequest {
                kind: AssetKind::Avatar,
                name: "Avatar".into(),
                shortcut: None,
                file_id: image.id,
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let _: FileInfo = json(
        server
            .request(Method::GET, &format!("/files/{}/info", image.id), &c)
            .send()
            .await
            .unwrap(),
    )
    .await;
    let _: Profile = json(
        server
            .request(Method::PATCH, "/me", &a)
            .json(&UpdateProfileRequest {
                avatar_id: Some(asset.id),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    error(
        server
            .upload(
                &a,
                None,
                "too-large.bin",
                vec![0; MAX_UPLOAD_BYTES + 1],
                "application/octet-stream",
            )
            .await,
        413,
    )
    .await;
    error(
        server
            .request(Method::PATCH, "/me", &c)
            .json(&UpdateProfileRequest {
                avatar_id: Some(asset.id),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;

    let _: Contact = json(
        server
            .request(Method::PATCH, &format!("/contacts/{}", c.user.id), &a)
            .json(&UpdateContactRequest {
                blocked: Some(true),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    error(
        server
            .request(Method::GET, &history, &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .request(Method::GET, &format!("/files/{}", file.id), &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .request(Method::GET, &format!("/files/{}", image.id), &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        server
            .upload(&c, Some(group.id), "blocked.txt", vec![1], "text/plain")
            .await,
        403,
    )
    .await;
    let convs: Vec<Conversation> = json(
        server
            .request(Method::GET, "/conversations", &c)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert!(convs.is_empty());
    // The blocked peer cannot erase Alice's block by deleting its own contact.
    assert_eq!(
        server
            .request(Method::DELETE, &format!("/contacts/{}", a.user.id), &c)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    error(
        server
            .request(Method::GET, &format!("/files/{}", image.id), &c)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let contacts: Vec<Contact> = json(
        server
            .request(Method::GET, "/contacts", &a)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert!(
        contacts
            .iter()
            .any(|contact| contact.user.id == c.user.id && contact.blocked)
    );
    assert_eq!(
        server
            .request(Method::DELETE, &format!("/assets/{}", asset.id), &a)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    error(
        server
            .request(Method::GET, &format!("/files/{}/info", image.id), &b)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    assert_eq!(
        server
            .request(
                Method::DELETE,
                &format!("/conversations/{}/members/{}", group.id, a.user.id),
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
            .request(Method::GET, &format!("/files/{}", file.id), &a)
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    server.shutdown().await;
}
