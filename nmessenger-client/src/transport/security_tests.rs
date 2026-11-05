#[path = "../../../nm-servo/tests/common/mod.rs"]
mod server;
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Method;

#[actix_web::test]
async fn group_member_changes_do_not_require_manual_verification() {
    let Some(server) = server::TestServer::start().await else { return; };
    let alice = server.register("Alice").await;
    let bob = server.register("Bob").await;
    let carol = server.register("Carol").await;
    server.befriend(&alice, &bob).await;
    server.befriend(&alice, &carol).await;
    let group = server.group(&alice, &[&bob]).await;
    let connect = |user: &AuthResponse| api::authenticate(server.url.clone(), user.user.email.clone(),
        "test password 123".into(), "Group test".into(), None, Presence::Available, None);
    let (a, _) = connect(&alice).await.unwrap();
    let (_b, _) = connect(&bob).await.unwrap();
    let (c, _) = connect(&carol).await.unwrap();
    let added: Conversation = a.json(Method::POST, &format!("/conversations/{}/members", group.id), &MemberRequest { user_id: carol.user.id }).await.unwrap();
    assert_eq!(added.members.len(), 3);
    let _: Vec<Conversation> = c.get("/conversations").await.unwrap();
    let message: Message = a.json(Method::POST, &format!("/conversations/{}/messages", group.id), &server::send_request("grupo atualizado")).await.unwrap();
    let history = c.history(group.id, None, "").await.unwrap();
    assert_eq!(history.iter().find(|entry| entry.id == message.id).unwrap().body, "grupo atualizado");
    a.empty(Method::DELETE, &format!("/conversations/{}/members/{}", group.id, carol.user.id)).await.unwrap();
    let _: Vec<Conversation> = a.get("/conversations").await.unwrap();
    let _: Message = a.json(Method::POST, &format!("/conversations/{}/messages", group.id), &server::send_request("grupo após remoção")).await.unwrap();
    server.shutdown().await;
}

#[actix_web::test]
async fn encrypted_group_history_can_be_restored_on_a_new_device() {
    let Some(server) = server::TestServer::start().await else { return; };
    let alice = server.register("Sender").await;
    let bob = server.register("Recipient").await;
    server.befriend(&alice, &bob).await;
    let group = server.group(&alice, &[&bob]).await;
    let connect = |account: &AuthResponse| api::authenticate(server.url.clone(), account.user.email.clone(),
        "test password 123".into(), "History test".into(), None, Presence::Available, None);
    let (sender, _) = connect(&alice).await.unwrap();
    let (old, _) = connect(&bob).await.unwrap();
    let sent: Message = sender.json(Method::POST, &format!("/conversations/{}/messages", group.id), &server::send_request("histórico privado")).await.unwrap();
    assert_eq!(old.history(group.id, None, "").await.unwrap().last().unwrap().body, "histórico privado");
    let (_, export) = super::opaque::login(&old.client, &old.base, &bob.user.email, "test password 123", "History test").await.unwrap().unwrap();
    old.sync_cloud_history(&export).await.unwrap();
    let old_device = old.crypto.as_ref().unwrap().device;
    let account = crate::secure::account(&old.base, &bob.user.id.to_string());
    let directory = crate::storage::directory().join("identities");
    for suffix in ["json", "pins.json", "files", "rosters", "history"] {
        let path = directory.join(format!("{account}.{suffix}"));
        if tokio::fs::try_exists(&path).await.unwrap() {
            tokio::fs::rename(&path, directory.join(format!("{account}.{suffix}.history-test"))).await.unwrap();
        }
    }
    let (new, _) = connect(&bob).await.unwrap();
    assert_ne!(new.crypto.as_ref().unwrap().device, old_device);
    let history = new.history(group.id, None, "").await.unwrap();
    assert_eq!(history.iter().find(|message| message.id == sent.id).unwrap().body, "histórico privado");
    server.shutdown().await;
}

#[actix_web::test]
async fn custom_emoticon_is_uploaded_and_sent_as_an_encrypted_attachment() {
    let Some(server) = server::TestServer::start().await else { return; };
    let sender = server.register("Emoticon sender").await;
    let receiver = server.register("Emoticon receiver").await;
    server.befriend(&sender, &receiver).await;
    let conversation = server.direct(&sender, &receiver).await;
    let connect = |account: &AuthResponse| api::authenticate(
        server.url.clone(), account.user.email.clone(), "test password 123".into(),
        "Emoticon test".into(), None, Presence::Available, None,
    );
    let (sender_api, _) = connect(&sender).await.unwrap();
    let (receiver_api, _) = connect(&receiver).await.unwrap();
    let (bus, events) = super::test_bus();
    let image = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/nmessenger/avatars/robot.png");
    let original = tokio::fs::read(&image).await.unwrap();
    super::files::upload_asset(&sender_api, &bus, AssetKind::Emoticon,
        "Robot".into(), Some(":robot:".into()), Some(image)).await.unwrap();
    let asset = match events.try_recv().unwrap().update {
        Update::Asset(asset) => asset,
        _ => panic!("expected created emoticon"),
    };
    assert!(matches!(events.try_recv().unwrap().update, Update::EmoticonUploadFinished(Ok(true))));
    super::files::send_emoticon(&sender_api, &bus, conversation.id, asset).await.unwrap();
    let request = match events.try_recv().unwrap().update {
        Update::PreparedMessage { request, .. } => request,
        _ => panic!("expected prepared emoticon message"),
    };
    let sent: Message = sender_api.json(Method::POST, &format!("/conversations/{}/messages", conversation.id), &request).await.unwrap();
    let messages = receiver_api.history(conversation.id, None, "").await.unwrap();
    let received = messages.iter().find(|message| message.id == sent.id).unwrap();
    assert_eq!(received.body, ":robot:");
    assert_eq!(receiver_api.bytes(received.attachment_id.unwrap()).await.unwrap(), original);
    let (forward_bus, forward_events) = super::test_bus();
    super::files::forward_attachment(&receiver_api, &forward_bus, received.attachment_id.unwrap(), conversation.id, MessageKind::Photo).await.unwrap();
    assert!(matches!(forward_events.try_recv().unwrap().update, Update::FileInfo(_)));
    let request = match forward_events.try_recv().unwrap().update {
        Update::PreparedMessage { request, .. } => request,
        _ => panic!("expected forwarded message"),
    };
    let forwarded: Message = receiver_api.json(Method::POST, &format!("/conversations/{}/messages", conversation.id), &request).await.unwrap();
    let history = sender_api.history(conversation.id, None, "").await.unwrap();
    let forwarded = history.iter().find(|message| message.id == forwarded.id).unwrap();
    assert_eq!(sender_api.bytes(forwarded.attachment_id.unwrap()).await.unwrap(), original);
    let audio = crate::media::audio::wav(&vec![1000; 3200]).unwrap();
    let voice_file = sender_api.upload(Some(conversation.id), "voice.wav".into(), "audio/wav", audio.clone()).await.unwrap();
    let voice_request = SendMessageRequest { kind: MessageKind::Voice, attachment_id: Some(voice_file.id), ..server::send_request("voice") };
    let voice: Message = sender_api.json(Method::POST, &format!("/conversations/{}/messages", conversation.id), &voice_request).await.unwrap();
    let history = receiver_api.history(conversation.id, None, "").await.unwrap();
    let received_voice = history.iter().find(|message| message.id == voice.id).unwrap();
    let (voice_bus, voice_events) = super::test_bus();
    super::files::forward_attachment(&receiver_api, &voice_bus, received_voice.attachment_id.unwrap(), conversation.id, MessageKind::Voice).await.unwrap();
    assert!(matches!(voice_events.try_recv().unwrap().update, Update::FileInfo(_)));
    let request = match voice_events.try_recv().unwrap().update {
        Update::PreparedMessage { request, .. } => request,
        _ => panic!("expected forwarded voice message"),
    };
    let voice: Message = receiver_api.json(Method::POST, &format!("/conversations/{}/messages", conversation.id), &request).await.unwrap();
    let history = sender_api.history(conversation.id, None, "").await.unwrap();
    let voice = history.iter().find(|message| message.id == voice.id).unwrap();
    assert_eq!(voice.kind, MessageKind::Voice);
    assert_eq!(sender_api.bytes(voice.attachment_id.unwrap()).await.unwrap(), audio);
    server.shutdown().await;
}

#[actix_web::test]
async fn bot_sdk_exchanges_encrypted_messages_and_files() {
    let Some(server) = server::TestServer::start().await else { return; };
    let owner = server.register("Bot SDK owner").await;
    let created: serde_json::Value = server::json(server.request(Method::POST, "/bots", &owner)
        .json(&serde_json::json!({"username": format!("bot_{}", &Uuid::new_v4().simple().to_string()[..16]), "name": "SDK bot"}))
        .send().await.unwrap()).await;
    let bot_id = Uuid::parse_str(created["bot"]["id"].as_str().unwrap()).unwrap();
    let bot = crate::bot_sdk::BotClient::connect(&server.url, bot_id, created["token"].as_str().unwrap().into(), "bot vault password").await.unwrap();
    let _: Contact = server::json(server.request(Method::POST, "/contacts", &owner)
        .json(&AddContactRequest { email: created["bot"]["address"].as_str().unwrap().into(), nickname: None })
        .send().await.unwrap()).await;
    let conversation: Conversation = server::json(server.request(Method::POST, "/conversations", &owner)
        .json(&CreateConversationRequest { member_ids: vec![bot_id], title: None, members_can_invite: true }).send().await.unwrap()).await;
    let (owner_api, _) = api::authenticate(server.url.clone(), owner.user.email.clone(), "test password 123".into(),
        "Bot SDK test".into(), None, Presence::Available, None).await.unwrap();
    bot.conversations().await.unwrap();
    let client_id = Uuid::new_v4();
    let sent = bot.send_text_with_id(conversation.id, client_id, "Olá do bot").await.unwrap();
    let retried = bot.send_text_with_id(conversation.id, client_id, "Olá do bot").await.unwrap();
    assert_eq!(sent.id, retried.id);
    assert_eq!(owner_api.history(conversation.id, None, "").await.unwrap().last().unwrap().body, "Olá do bot");
    let clear = b"arquivo do bot".to_vec();
    let file = bot.upload_file(conversation.id, "bot.txt".into(), "text/plain", clear.clone()).await.unwrap();
    bot.send_file(conversation.id, Uuid::new_v4(), file.id, "anexo").await.unwrap();
    owner_api.history(conversation.id, None, "").await.unwrap();
    assert_eq!(owner_api.bytes(file.id).await.unwrap(), clear);
    let message: Message = owner_api.json(Method::POST, &format!("/conversations/{}/messages", conversation.id),
        &server::send_request("Olá do proprietário")).await.unwrap();
    assert!(bot.history(conversation.id, None).await.unwrap().iter().any(|m| m.id == message.id && m.body == "Olá do proprietário"));
    bot.mark_read(conversation.id, Some(message.id)).await.unwrap();
    sqlx::query("DELETE FROM conversations WHERE id=$1").bind(conversation.id).execute(&server.state.db).await.unwrap();
    sqlx::query("DELETE FROM users WHERE id=$1").bind(bot_id).execute(&server.state.db).await.unwrap();
    server.shutdown().await;
}

#[actix_web::test]
async fn password_reset_keeps_the_existing_device_identity() {
    let Some(server) = server::TestServer::start().await else { return; };
    let admin = server.register("Administrator").await;
    let user = server.register("Password reset user").await;
    sqlx::query("INSERT INTO app_admins(user_id) VALUES($1)").bind(admin.user.id).execute(&server.state.db).await.unwrap();
    let login = |password: &str, old: Option<&str>| api::authenticate(
        server.url.clone(), user.user.email.clone(), password.to_owned(), "Reset test".into(), None,
        Presence::Available, old.map(str::to_owned),
    );
    let (before, _) = login("test password 123", None).await.unwrap();
    let old_device = before.crypto.as_ref().unwrap().device;
    let response = server.request(Method::POST, &format!("/admin/users/{}/password", user.user.id), &admin)
        .json(&serde_json::json!({"password":"new test password", "confirmation_password":"test password 123"}))
        .send().await.unwrap();
    assert!(response.status().is_success());
    let error = match login("new test password", None).await { Ok(_) => panic!("vault should require its original password"), Err(error) => error };
    assert!(error.contains("Não foi possível desbloquear a identidade local"));
    let (after, _) = login("new test password", Some("test password 123")).await.unwrap();
    assert_eq!(after.crypto.as_ref().unwrap().device, old_device);
    let (again, _) = login("new test password", None).await.unwrap();
    assert_eq!(again.crypto.as_ref().unwrap().device, old_device);
    server.shutdown().await;
}

#[actix_web::test]
async fn password_reset_history_recovery_works_on_a_fresh_device() {
    let Some(server) = server::TestServer::start().await else { return; };
    let admin = server.register("Administrator").await;
    let user = server.register("History user").await;
    sqlx::query("INSERT INTO app_admins(user_id) VALUES($1)").bind(admin.user.id).execute(&server.state.db).await.unwrap();
    let login = |password: &str, old: Option<&str>| api::authenticate(server.url.clone(), user.user.email.clone(),
        password.to_owned(), "History reset".into(), None, Presence::Available, old.map(str::to_owned));
    let (before, _) = login("test password 123", None).await.unwrap();
    let old_device = before.crypto.as_ref().unwrap().device;
    let response = server.request(Method::POST, &format!("/admin/users/{}/password", user.user.id), &admin)
        .json(&serde_json::json!({"password":"new test password", "confirmation_password":"test password 123"})).send().await.unwrap();
    assert!(response.status().is_success());
    let account = crate::secure::account(&before.base, &user.user.id.to_string());
    let directory = crate::storage::directory().join("identities");
    for suffix in ["json", "pins.json", "files", "rosters", "history"] {
        let path = directory.join(format!("{account}.{suffix}"));
        if tokio::fs::try_exists(&path).await.unwrap() {
            tokio::fs::rename(&path, directory.join(format!("{account}.{suffix}.reset-test"))).await.unwrap();
        }
    }
    let error = match login("new test password", None).await { Ok(_) => panic!("old backup must not be overwritten"), Err(error) => error };
    assert!(error.contains("Não foi possível desbloquear a identidade local"));
    let (after, _) = login("new test password", Some("test password 123")).await.unwrap();
    assert_ne!(after.crypto.as_ref().unwrap().device, old_device);
    let (again, _) = login("new test password", None).await.unwrap();
    assert_eq!(again.crypto.as_ref().unwrap().device, after.crypto.as_ref().unwrap().device);
    server.shutdown().await;
}

#[actix_web::test]
async fn e2ee_survives_real_http_database_uploads_websocket_media_and_session_revocation() {
    let Some(server) = server::TestServer::start().await else {
        return;
    };
    let original_a = server.register("E2EE Alice").await;
    let original_b = server.register("E2EE Bob").await;
    let outsider = server.register("E2EE outsider").await;
    server.befriend(&original_a, &original_b).await;
    let conversation = server.direct(&original_a, &original_b).await;
    let (a, boot_a) = api::authenticate(
        server.url.clone(),
        original_a.user.email.clone(),
        "test password 123".into(),
        "Alice device".into(),
        None,
        Presence::Available,
        None,
    )
    .await
    .unwrap();
    let (b, boot_b) = api::authenticate(
        server.url.clone(),
        original_b.user.email.clone(),
        "test password 123".into(),
        "Bob device".into(),
        None,
        Presence::Available,
        None,
    )
    .await
    .unwrap();
    let auth_a = AuthResponse {
        token: a.token.to_string(),
        session_id: a.session_id,
        user: boot_a.me,
    };
    let auth_b = AuthResponse {
        token: b.token.to_string(),
        session_id: b.session_id,
        user: boot_b.me,
    };
    let request = server::send_request("plaintext must never reach PostgreSQL 👋");
    let sent: Message = a
        .json(
            Method::POST,
            &format!("/conversations/{}/messages", conversation.id),
            &request,
        )
        .await
        .unwrap();
    assert_eq!(sent.body, request.body);
    let stored: String = sqlx::query_scalar("SELECT body FROM messages WHERE id=$1")
        .bind(sent.id)
        .fetch_one(&server.state.db)
        .await
        .unwrap();
    assert!(stored.starts_with(ENCRYPTED_PREFIX));
    assert!(!stored.contains("plaintext must never"));
    assert_eq!(
        b.history(conversation.id, None, "")
            .await
            .unwrap()
            .last()
            .unwrap()
            .body,
        request.body
    );
    let mut tampered = sent.clone();
    tampered.body = stored;
    tampered.client_id = Uuid::new_v4();
    assert!(b.open_message(&mut tampered).await.is_err());
    let mut edit_socket = server.socket(&auth_b).await;
    let mut edit_request = request.clone();
    edit_request.body = "edited confidential message".into();
    let edited: Message = a.json(Method::PATCH, &format!("/conversations/{}/messages/{}", conversation.id, sent.id), &edit_request).await.unwrap();
    assert_eq!(edited.body, edit_request.body);
    let stored_edit: String = sqlx::query_scalar("SELECT body FROM messages WHERE id=$1").bind(sent.id).fetch_one(&server.state.db).await.unwrap();
    assert!(stored_edit.starts_with(ENCRYPTED_PREFIX) && !stored_edit.contains(&edit_request.body));
    assert!(b.history(conversation.id, None, "").await.unwrap().iter().any(|message| message.id == sent.id && message.body == edit_request.body));
    let update = server::event_matching(&mut edit_socket, |event| matches!(event, ServerEvent::MessageUpdated { message } if message.id == sent.id)).await;
    assert!(matches!(b.open_event(update).await.unwrap(), ServerEvent::MessageUpdated { message } if message.body == edit_request.body));
    server::error(
        server
            .request(
                Method::GET,
                &format!("/conversations/{}/encryption/keys", conversation.id),
                &outsider,
            )
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    server::error(
        server
            .request(
                Method::POST,
                &format!("/conversations/{}/messages", conversation.id),
                &auth_a,
            )
            .json(&server::send_request("plaintext downgrade"))
            .send()
            .await
            .unwrap(),
        400,
    )
    .await;
    let clear = b"confidential file contents that the server must not see".to_vec();
    let file = a
        .upload(
            Some(conversation.id),
            "private.txt".into(),
            "text/plain",
            clear.clone(),
        )
        .await
        .unwrap();
    let attachment = SendMessageRequest {
        attachment_id: Some(file.id),
        kind: MessageKind::File,
        body: file.filename.clone(),
        ..server::send_request("")
    };
    let _: Message = a
        .json(
            Method::POST,
            &format!("/conversations/{}/messages", conversation.id),
            &attachment,
        )
        .await
        .unwrap();
    b.history(conversation.id, None, "").await.unwrap();
    assert_eq!(b.bytes(file.id).await.unwrap(), clear);
    let path = server.state.upload_dir.join(file.id.to_string());
    let mut blob = tokio::fs::read(&path).await.unwrap();
    assert!(blob.starts_with(super::crypto_files::MAGIC));
    assert!(!blob.windows(clear.len()).any(|bytes| bytes == clear));
    let last = blob.len() - 1;
    blob[last] ^= 1;
    tokio::fs::write(&path, blob).await.unwrap();
    assert!(b.bytes(file.id).await.is_err());
    let activity: Activity = a
        .json(
            Method::POST,
            &format!("/conversations/{}/activities", conversation.id),
            &ActivityRequest {
                kind: ActivityKind::AudioCall,
                payload: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    let activity: Activity = b
        .json(
            Method::PATCH,
            &format!("/activities/{}", activity.id),
            &UpdateActivityRequest {
                status: ActivityStatus::Accepted,
                payload: None,
            },
        )
        .await
        .unwrap();
    a.open_event(ServerEvent::Activity {
        activity: activity.clone(),
    })
    .await
    .unwrap();
    let mut sender = server.socket(&auth_a).await;
    let mut receiver = server.socket(&auth_b).await;
    let clear_pcm = STANDARD.encode([0u8; 640]);
    let encrypted = a
        .seal_event(ClientEvent::Media {
            activity_id: activity.id,
            stream: MediaStream::Audio,
            sequence: 1,
            data: clear_pcm.clone(),
        })
        .await
        .unwrap();
    assert!(
        matches!(&encrypted, ClientEvent::Media { data, .. } if data.starts_with(ENCRYPTED_PREFIX))
    );
    server::send_event(&mut sender, &encrypted).await;
    let incoming = server::event_matching(&mut receiver, |event| {
        matches!(event, ServerEvent::Media { .. })
    })
    .await;
    assert!(
        matches!(b.open_event(incoming).await.unwrap(), ServerEvent::Media { data, .. } if data == clear_pcm)
    );
    let signal = serde_json::json!({"test_signal":"private signal"});
    let encrypted = a
        .seal_event(ClientEvent::Signal {
            activity_id: activity.id,
            data: signal.clone(),
        })
        .await
        .unwrap();
    server::send_event(&mut sender, &encrypted).await;
    let incoming = server::event_matching(&mut receiver, |event| {
        matches!(event, ServerEvent::Signal { .. })
    })
    .await;
    assert!(
        matches!(b.open_event(incoming).await.unwrap(), ServerEvent::Signal { data, .. } if data == signal)
    );
    b.empty(Method::DELETE, &format!("/sessions/{}", b.session_id))
        .await
        .unwrap();
    a.crypto.as_ref().unwrap().key_cache.lock().await.clear();
    assert!(
        a.json::<Message, _>(
            Method::POST,
            &format!("/conversations/{}/messages", conversation.id),
            &server::send_request("must not downgrade after revocation")
        )
        .await
        .is_err()
    );
    drop(sender);
    drop(receiver);
    server.shutdown().await;
}
