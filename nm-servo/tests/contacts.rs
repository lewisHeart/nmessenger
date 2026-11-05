mod common;

use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn consent_categories_private_details_and_import_export() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let a = server.register("Alice").await;
    let b = server.register("Bob").await;
    let c = server.register("Carol").await;
    error(
        server
            .request(Method::POST, "/conversations", &a)
            .json(&CreateConversationRequest {
                member_ids: vec![b.user.id],
                title: None,
                members_can_invite: true,
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let outgoing: Contact = json(
        server
            .request(Method::POST, "/contacts", &a)
            .json(&AddContactRequest {
                email: b.user.email.to_ascii_uppercase(),
                nickname: Some("Bobby".into()),
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(outgoing.state, ContactState::PendingOutgoing);
    error(
        server
            .request(Method::PATCH, &format!("/contacts/{}", b.user.id), &a)
            .json(&UpdateContactRequest {
                accepted: Some(true),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
        409,
    )
    .await;
    let incoming: Vec<Contact> = json(
        server
            .request(Method::GET, "/contacts", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(incoming[0].state, ContactState::PendingIncoming);
    let _: Contact = json(
        server
            .request(Method::PATCH, &format!("/contacts/{}", a.user.id), &b)
            .json(&UpdateContactRequest {
                accepted: Some(true),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let _ = server.direct(&a, &b).await;

    let category: Category = json(
        server
            .request(Method::POST, "/categories", &a)
            .json(&CategoryRequest {
                name: "Amigos".into(),
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let foreign: Category = json(
        server
            .request(Method::POST, "/categories", &b)
            .json(&CategoryRequest {
                name: "Privado".into(),
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    error(
        server
            .request(Method::PATCH, &format!("/contacts/{}", b.user.id), &a)
            .json(&UpdateContactRequest {
                category_ids: Some(vec![foreign.id]),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let details = ContactDetails {
        notes: "Notas privadas da Alice".into(),
        company: "NM".into(),
        ..Default::default()
    };
    let updated: Contact = json(
        server
            .request(Method::PATCH, &format!("/contacts/{}", b.user.id), &a)
            .json(&UpdateContactRequest {
                favorite: Some(true),
                category_ids: Some(vec![category.id]),
                details: Some(details),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert!(updated.favorite);
    assert_eq!(updated.nickname.as_deref(), Some("Bobby"));
    assert_eq!(updated.category_ids, vec![category.id]);
    let other_side: Vec<Contact> = json(
        server
            .request(Method::GET, "/contacts", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert!(other_side[0].details.notes.is_empty());
    assert!(other_side[0].category_ids.is_empty());
    assert_eq!(
        server
            .request(Method::DELETE, &format!("/categories/{}", category.id), &a)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let remaining: Vec<Contact> = json(
        server
            .request(Method::GET, "/contacts/export", &a)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert!(remaining[0].category_ids.is_empty());
    assert_eq!(remaining[0].details.notes, "Notas privadas da Alice");

    let imported: Vec<Contact> = json(
        server
            .request(Method::POST, "/contacts/import", &a)
            .json(&ImportContactsRequest {
                emails: vec![
                    c.user.email.clone(),
                    c.user.email.clone(),
                    "missing@nm.test".into(),
                ],
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].user.id, c.user.id);
    let _: Contact = json(
        server
            .request(Method::PATCH, &format!("/contacts/{}", b.user.id), &a)
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
            .request(Method::POST, "/conversations", &b)
            .json(&CreateConversationRequest {
                member_ids: vec![a.user.id],
                title: None,
                members_can_invite: true,
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    server.shutdown().await;
}
