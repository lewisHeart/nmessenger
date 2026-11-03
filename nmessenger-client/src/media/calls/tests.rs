use super::*;
use actix_web::{App, HttpServer, web};
use futures_util::{FutureExt, SinkExt, StreamExt};
use nm_servo::{
    state::{AppState, Config},
    workers,
};
use std::{
    net::TcpListener,
    path::PathBuf,
    sync::atomic::{AtomicU64, AtomicUsize},
    time::Duration,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message as Frame};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Default)]
struct Stats {
    audio: AtomicUsize,
    video: AtomicUsize,
    nonzero: AtomicUsize,
    silent: AtomicUsize,
    sequence: AtomicU64,
}

async fn account(client: &reqwest::Client, base: &str, name: &str) -> AuthResponse {
    let response = client
        .post(format!("{base}/v1/auth/register"))
        .json(&RegisterRequest {
            email: format!("media-{}@nm.test", Uuid::new_v4()),
            password: "Media test password 123".into(),
            display_name: name.into(),
            device_name: "Synthetic media test".into(),
        })
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    response.json().await.unwrap()
}

async fn socket(base: &str, account: &AuthResponse) -> Socket {
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("{}/v1/events", base.replacen("http", "ws", 1)))
            .await
            .unwrap();
    socket
        .send(Frame::Text(
            serde_json::to_string(&ClientEvent::Authenticate {
                token: account.token.clone(),
            })
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    let frame = socket.next().await.unwrap().unwrap();
    assert!(matches!(
        serde_json::from_str::<ServerEvent>(frame.to_text().unwrap()).unwrap(),
        ServerEvent::Authenticated { .. }
    ));
    socket
}

fn drive(
    socket: Socket,
    mut outgoing: tokio::sync::mpsc::Receiver<ClientEvent>,
    controller: Arc<MediaController>,
    bus: Bus,
    remote: Uuid,
    stats: Arc<Stats>,
) -> Vec<tokio::task::JoinHandle<()>> {
    let (sender, mut incoming) = socket.split();
    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    let write = sender.clone();
    let writer = tokio::spawn(async move {
        while let Some(event) = outgoing.recv().await {
            if write
                .lock()
                .await
                .send(Frame::Text(serde_json::to_string(&event).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let reader = tokio::spawn(async move {
        while let Some(Ok(frame)) = incoming.next().await {
            match frame {
                Frame::Ping(bytes) => {
                    let _ = sender.lock().await.send(Frame::Pong(bytes)).await;
                }
                Frame::Text(text) => {
                    if let Ok(ServerEvent::Media {
                        activity_id,
                        from_user_id,
                        stream,
                        sequence,
                        data,
                    }) = serde_json::from_str(&text)
                        && from_user_id == remote
                    {
                        match stream {
                            MediaStream::Audio => {
                                stats.audio.fetch_add(1, Ordering::Relaxed);
                                let bytes = STANDARD.decode(&data).unwrap();
                                if bytes.iter().any(|byte| *byte != 0) {
                                    stats.nonzero.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    stats.silent.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            MediaStream::Camera => {
                                stats.video.fetch_add(1, Ordering::Relaxed);
                                stats.sequence.store(sequence, Ordering::Relaxed);
                            }
                            _ => {}
                        }
                        controller.receive(
                            activity_id,
                            from_user_id,
                            stream,
                            sequence,
                            data,
                            bus.clone(),
                        );
                    }
                }
                _ => {}
            }
        }
    });
    vec![writer, reader]
}

async fn until(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("synthetic call must deliver real audio/video through the server");
}

#[actix_web::test]
#[ignore = "requires PostgreSQL, Redis, ffmpeg and NM_TEST_MEDIA=1; never uses physical devices"]
async fn synthetic_audio_video_cross_the_real_backend_and_survive_restart() {
    assert_eq!(std::env::var("NM_TEST_MEDIA").as_deref(), Ok("1"));
    let config = Config {
        user_upload_quota: 512 * 1024 * 1024,
        database_url: std::env::var("NM_TEST_DATABASE_URL").expect("test database"),
        redis_url: std::env::var("NM_TEST_REDIS_URL").expect("test Redis"),
        bind_addr: "127.0.0.1:0".into(),
        upload_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/media-test")
            .join(Uuid::new_v4().to_string()),
        session_ttl_secs: 3600,
    };
    let state = AppState::connect(&config).await.unwrap();
    let worker = workers::start(&state).await.unwrap();
    let data = web::Data::new(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .configure(nm_servo::configure)
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);
    let client = reqwest::Client::new();
    let a = account(&client, &base, "Media A").await;
    let b = account(&client, &base, "Media B").await;
    let response = client
        .post(format!("{base}/v1/contacts"))
        .bearer_auth(&a.token)
        .json(&AddContactRequest {
            email: b.user.email.clone(),
            nickname: None,
        })
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let response = client
        .patch(format!("{base}/v1/contacts/{}", a.user.id))
        .bearer_auth(&b.token)
        .json(&UpdateContactRequest {
            accepted: Some(true),
            ..Default::default()
        })
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let conversation: Conversation = client
        .post(format!("{base}/v1/conversations"))
        .bearer_auth(&a.token)
        .json(&CreateConversationRequest {
            member_ids: vec![b.user.id],
            title: None,
        })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let offered: Activity = client
        .post(format!(
            "{base}/v1/conversations/{}/activities",
            conversation.id
        ))
        .bearer_auth(&a.token)
        .json(&ActivityRequest {
            kind: ActivityKind::VideoCall,
            payload: serde_json::json!({"test":"synthetic"}),
        })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let activity: Activity = client
        .patch(format!("{base}/v1/activities/{}", offered.id))
        .bearer_auth(&b.token)
        .json(&UpdateActivityRequest {
            status: ActivityStatus::Accepted,
            payload: None,
        })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(activity.status, ActivityStatus::Accepted);

    let ca = Arc::new(MediaController::default());
    let cb = Arc::new(MediaController::default());
    let (ba, events_a) = crate::transport::test_bus();
    let (bb, events_b) = crate::transport::test_bus();
    let (ta, ra) = tokio::sync::mpsc::channel(128);
    let (tb, rb) = tokio::sync::mpsc::channel(128);
    let sa = Arc::new(Stats::default());
    let sb = Arc::new(Stats::default());
    let mut drivers = drive(
        socket(&base, &a).await,
        ra,
        ca.clone(),
        ba.clone(),
        b.user.id,
        sa.clone(),
    );
    drivers.extend(drive(
        socket(&base, &b).await,
        rb,
        cb.clone(),
        bb.clone(),
        a.user.id,
        sb.clone(),
    ));
    let media = MediaConfig {
        input: "test:sine".into(),
        output: "test:null".into(),
        camera: "test:pattern".into(),
        muted: false,
        camera_enabled: true,
    };
    ca.start_call(
        activity.clone(),
        media.clone(),
        a.user.id,
        ta.clone(),
        ba.clone(),
        false,
    );
    cb.start_call(activity.clone(), media.clone(), b.user.id, tb, bb, false);
    let verification = std::panic::AssertUnwindSafe(async {
        until(|| {
            sa.video.load(Ordering::Relaxed) >= 2
                && sb.video.load(Ordering::Relaxed) >= 2
                && sa.nonzero.load(Ordering::Relaxed) >= 3
                && sb.nonzero.load(Ordering::Relaxed) >= 3
        })
        .await;
        let mut remote_frames = 0;
        until(|| {
            for packet in events_b.try_iter() {
                if let Update::LiveFrame { from, frame, .. } = packet.update
                    && from == a.user.id
                {
                    assert_eq!(frame.image.size, [640, 360]);
                    remote_frames += 1;
                }
            }
            remote_frames > 0
        })
        .await;
        ca.mute(true);
        until(|| sb.silent.load(Ordering::Relaxed) >= 2).await;
        ca.camera(false);
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        let stopped_video = sb.video.load(Ordering::Relaxed);
        let audio_before = sb.silent.load(Ordering::Relaxed);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        assert_eq!(sb.video.load(Ordering::Relaxed), stopped_video, "camera should stop transmitting");
        assert!(sb.silent.load(Ordering::Relaxed) > audio_before, "audio must continue while camera is off");
        ca.camera(true);
        until(|| sb.video.load(Ordering::Relaxed) >= stopped_video + 2).await;
        let old_sequence = sb.sequence.load(Ordering::Relaxed);
        let old_count = sb.video.load(Ordering::Relaxed);
        ca.start_call(activity, media, a.user.id, ta, ba, true);
        until(|| {
            sb.video.load(Ordering::Relaxed) >= old_count + 3
                && sb.sequence.load(Ordering::Relaxed) > old_sequence
        })
        .await;
        assert!(events_a.try_iter().any(|packet| matches!(
            packet.update,
            Update::CallMedia(CallEvent::Ready {
                component: Component::RemoteAudio,
                ..
            })
        )));
    })
    .catch_unwind()
    .await;
    ca.stop_all();
    cb.stop_all();
    for task in drivers {
        task.abort();
    }
    handle.stop(false).await;
    drop(worker);
    sqlx::query("DELETE FROM conversations WHERE id=$1")
        .bind(conversation.id)
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id=ANY($1)")
        .bind(vec![a.user.id, b.user.id])
        .execute(&state.db)
        .await
        .unwrap();
    let _ = tokio::fs::remove_dir_all(config.upload_dir).await;
    if let Err(panic) = verification {
        std::panic::resume_unwind(panic);
    }
}
