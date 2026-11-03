mod common;

use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use nm_protocol::*;
use reqwest::Method;

#[actix_web::test]
async fn redis_cross_instance_events_activity_consent_media_blocking_and_ws_revocation() {
    let Some(first) = TestServer::start().await else {
        return;
    };
    let second = TestServer::start().await.unwrap();
    let a = first.register("Alice").await;
    let b = first.register("Bob").await;
    let c = first.register("Outsider").await;
    first.befriend(&a, &b).await;
    let conversation = first.direct(&a, &b).await;
    let mut alice = first.socket(&a).await;
    let mut bob = second.socket(&b).await;
    let mut outsider = second.socket(&c).await;

    let _: Profile = json(
        first
            .request(Method::PATCH, "/me", &a)
            .json(&UpdateProfileRequest {
                presence: Some(Presence::Invisible),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let event = event_matching(
        &mut bob,
        |e| matches!(e,ServerEvent::ProfileUpdated {profile} if profile.id==a.user.id),
    )
    .await;
    assert!(
        matches!(event,ServerEvent::ProfileUpdated {profile} if profile.presence==Presence::Offline)
    );
    let contacts: Vec<Contact> = json(
        second
            .request(Method::GET, "/contacts", &b)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(contacts[0].user.presence, Presence::Offline);

    let sent = first
        .message(&a, conversation.id, "Across real Redis pub/sub")
        .await;
    let event = event_matching(
        &mut bob,
        |e| matches!(e,ServerEvent::Message {message} if message.id==sent.id),
    )
    .await;
    assert!(matches!(event,ServerEvent::Message {message} if message.sender_id==a.user.id));
    send_event(
        &mut outsider,
        &ClientEvent::Typing {
            conversation_id: conversation.id,
            is_typing: true,
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut outsider,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="forbidden")
    );
    send_event(
        &mut alice,
        &ClientEvent::Typing {
            conversation_id: conversation.id,
            is_typing: true,
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut bob,|e| matches!(e,ServerEvent::Typing {..})).await,
        ServerEvent::Typing {user_id,..} if user_id==a.user.id)
    );

    let game: Activity = json(
        first
            .request(
                Method::POST,
                &format!("/conversations/{}/activities", conversation.id),
                &a,
            )
            .json(&ActivityRequest {
                kind: ActivityKind::Game,
                payload: serde_json::json!({"game":"tic_tac_toe"}),
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    send_event(
        &mut alice,
        &ClientEvent::Signal {
            activity_id: game.id,
            data: serde_json::json!({"cell":0}),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut alice,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="forbidden")
    );
    error(
        first
            .request(Method::PATCH, &format!("/activities/{}", game.id), &a)
            .json(&UpdateActivityRequest {
                status: ActivityStatus::Accepted,
                payload: None,
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    error(
        first
            .request(Method::PATCH, &format!("/activities/{}", game.id), &c)
            .json(&UpdateActivityRequest {
                status: ActivityStatus::Accepted,
                payload: None,
            })
            .send()
            .await
            .unwrap(),
        403,
    )
    .await;
    let accepted: Activity = json(
        second
            .request(Method::PATCH, &format!("/activities/{}", game.id), &b)
            .json(&UpdateActivityRequest {
                status: ActivityStatus::Accepted,
                payload: None,
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(accepted.status, ActivityStatus::Accepted);
    send_event(
        &mut alice,
        &ClientEvent::Signal {
            activity_id: game.id,
            data: serde_json::json!({"cell":4,"turn":1}),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut bob,|e| matches!(e,ServerEvent::Signal {..})).await,
        ServerEvent::Signal {activity_id,from_user_id,data} if activity_id==game.id
            && from_user_id==a.user.id && data["cell"]==4)
    );
    send_event(
        &mut alice,
        &ClientEvent::Media {
            activity_id: game.id,
            stream: MediaStream::Audio,
            sequence: 0,
            data: STANDARD.encode([0u8; 640]),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut alice,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="forbidden")
    );

    let call: Activity = json(
        first
            .request(
                Method::POST,
                &format!("/conversations/{}/activities", conversation.id),
                &a,
            )
            .json(&ActivityRequest {
                kind: ActivityKind::VideoCall,
                payload: serde_json::json!({}),
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    let _: Activity = json(
        second
            .request(Method::PATCH, &format!("/activities/{}", call.id), &b)
            .json(&UpdateActivityRequest {
                status: ActivityStatus::Accepted,
                payload: None,
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    send_event(
        &mut alice,
        &ClientEvent::Media {
            activity_id: call.id,
            stream: MediaStream::Camera,
            sequence: 0,
            data: STANDARD.encode(b"not jpeg"),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut alice,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="invalid_request")
    );
    send_event(
        &mut alice,
        &ClientEvent::Media {
            activity_id: call.id,
            stream: MediaStream::Audio,
            sequence: 0,
            data: STANDARD.encode([0u8; 640]),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut bob,|e| matches!(e,ServerEvent::Media {..})).await,
        ServerEvent::Media {from_user_id,stream:MediaStream::Audio,..} if from_user_id==a.user.id)
    );
    for stream in [MediaStream::Camera, MediaStream::Screen] {
        send_event(&mut alice, &ClientEvent::Media {
            activity_id: call.id, stream, sequence: 1,
            data: STANDARD.encode([0xff, 0xd8, 0xff, 0xd9]),
        }).await;
        let event = event_matching(&mut bob, |event| matches!(event, ServerEvent::Media { stream: received, .. } if *received == stream)).await;
        assert!(matches!(event, ServerEvent::Media { stream: received, .. } if received == stream));
    }

    let newer = first.login(&a, "Second device").await;
    assert_eq!(
        second
            .request(
                Method::DELETE,
                &format!("/sessions/{}", a.session_id),
                &newer
            )
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert!(
        matches!(event_matching(&mut alice,|e| matches!(e,ServerEvent::SessionRevoked {..})).await,
        ServerEvent::SessionRevoked {session_id} if session_id==a.session_id)
    );
    error(
        first
            .request(Method::GET, "/bootstrap", &a)
            .send()
            .await
            .unwrap(),
        401,
    )
    .await;
    let _: Bootstrap = json(
        second
            .request(Method::GET, "/bootstrap", &newer)
            .send()
            .await
            .unwrap(),
    )
    .await;

    let _: Contact = json(
        first
            .request(Method::PATCH, &format!("/contacts/{}", b.user.id), &newer)
            .json(&UpdateContactRequest {
                blocked: Some(true),
                ..Default::default()
            })
            .send()
            .await
            .unwrap(),
    )
    .await;
    send_event(
        &mut bob,
        &ClientEvent::Typing {
            conversation_id: conversation.id,
            is_typing: true,
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut bob,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="forbidden")
    );
    send_event(
        &mut bob,
        &ClientEvent::Signal {
            activity_id: game.id,
            data: serde_json::json!({"cell":0}),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut bob,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="forbidden")
    );
    send_event(
        &mut bob,
        &ClientEvent::Media {
            activity_id: call.id,
            stream: MediaStream::Audio,
            sequence: 0,
            data: STANDARD.encode([0u8; 640]),
        },
    )
    .await;
    assert!(
        matches!(event_matching(&mut bob,|e| matches!(e,ServerEvent::Error {..})).await,
        ServerEvent::Error {code,..} if code=="forbidden")
    );
    let _ = bob.close(None).await;
    let _ = outsider.close(None).await;
    second.shutdown().await;
    first.shutdown().await;
}

#[actix_web::test]
async fn websocket_requires_authentication_in_first_text_frame() {
    let Some(server) = TestServer::start().await else {
        return;
    };
    let url = format!("{}/v1/events", server.url.replacen("http://", "ws://", 1));
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send_event(&mut socket, &ClientEvent::Ping).await;
    assert!(
        matches!(next_event(&mut socket).await,ServerEvent::Error {code,..} if code=="unauthorized")
    );
    let _ = socket.close(None).await;
    server.shutdown().await;
}
