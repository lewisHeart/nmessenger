//! Notification-only WebSocket owned by the Android foreground service.
use futures_util::{SinkExt, StreamExt};
use jni::{EnvUnowned, JavaVM, objects::{Global, JObject, JString}};
use nm_protocol::{ClientEvent, ServerEvent};
use std::{sync::Mutex, time::Duration};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};
use zeroize::Zeroizing;

static STOP: Mutex<Option<oneshot::Sender<()>>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MessageNotificationService_nativeStartSocket(
    mut env: EnvUnowned<'_>, service: JObject<'_>, base: JString<'_>, token: JString<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let base = base.try_to_string(env)?;
        let token = Zeroizing::new(token.try_to_string(env)?);
        let service = env.new_global_ref(&service)?;
        let vm = env.get_java_vm()?;
        let (sender, receiver) = oneshot::channel();
        if let Ok(mut current) = STOP.lock() {
            if let Some(previous) = current.replace(sender) { let _ = previous.send(()); }
        } else { return Ok(()); }
        std::thread::spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                runtime.block_on(run(base, token, vm, service, receiver));
            }
        });
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MessageNotificationService_nativeStopSocket(
    _env: *mut std::ffi::c_void, _service: *mut std::ffi::c_void,
) {
    if let Ok(mut current) = STOP.lock() { if let Some(stop) = current.take() { let _ = stop.send(()); } }
}

fn hint(vm: &JavaVM, service: &Global<JObject<'static>>) {
    let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        env.call_method(service, jni::jni_str!("onRealtimeHint"), jni::jni_sig!("()V"), &[])?;
        Ok(())
    });
}

async fn run(base: String, token: Zeroizing<String>, vm: JavaVM, service: Global<JObject<'static>>, mut stop: oneshot::Receiver<()>) {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let Ok(mut url) = reqwest::Url::parse(&format!("{base}/notifications/events")) else { return; };
    let scheme = match url.scheme() { "https" => "wss", "http" if matches!(url.host_str(), Some("localhost" | "127.0.0.1")) => "ws", _ => return };
    if url.set_scheme(scheme).is_err() { return; }
    let mut delay = 1u64;
    loop {
        let config = WebSocketConfig::default().max_message_size(Some(4096)).max_frame_size(Some(4096));
        let connection = tokio::select! {
            _ = &mut stop => return,
            result = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config), false)) => result,
        };
        if let Ok(Ok((mut socket, _))) = connection {
            let event = ClientEvent::Authenticate { token: token.to_string() };
            let Ok(json) = serde_json::to_string(&event) else { return; };
            if socket.send(Message::Text(json.into())).await.is_ok() {
                let mut heartbeat = tokio::time::interval(Duration::from_secs(25));
                let mut last_received = tokio::time::Instant::now();
                let mut authenticated = false;
                let deadline = tokio::time::sleep(Duration::from_secs(10));
                tokio::pin!(deadline);
                loop {
                    tokio::select! {
                        _ = &mut stop => return,
                        _ = &mut deadline, if !authenticated => break,
                        _ = heartbeat.tick(), if authenticated => {
                            if last_received.elapsed() > Duration::from_secs(75) || socket.send(Message::Text("{\"type\":\"ping\"}".into())).await.is_err() { break; }
                        }
                        frame = socket.next() => {
                            last_received = tokio::time::Instant::now();
                            match frame {
                                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ServerEvent>(&text) {
                                    Ok(ServerEvent::Authenticated { .. }) => { authenticated = true; delay = 1; hint(&vm, &service); }
                                    Ok(ServerEvent::ResyncRequired) if authenticated => hint(&vm, &service),
                                    Ok(ServerEvent::Pong) if authenticated => {},
                                    _ => break,
                                },
                                Some(Ok(Message::Ping(bytes))) => if socket.send(Message::Pong(bytes)).await.is_err() { break; },
                                Some(Ok(Message::Pong(_))) => {},
                                _ => break,
                            }
                        }
                    }
                }
            }
        }
        tokio::select! { _ = &mut stop => return, _ = tokio::time::sleep(Duration::from_secs(delay)) => {} }
        delay = (delay * 2).min(30);
    }
}
