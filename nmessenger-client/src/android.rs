use jni::{
    Env, EnvUnowned, JavaVM,
    objects::{JByteArray, JClass, JObject, JString, JValue},
};
use std::{path::PathBuf, sync::Mutex};
use tokio::sync::oneshot;
use winit::platform::android::activity::AndroidApp;

static APP: Mutex<Option<AndroidApp>> = Mutex::new(None);
type Selection = Result<Option<PathBuf>, String>;
static DOCUMENT: Mutex<Option<oneshot::Sender<Selection>>> = Mutex::new(None);

#[cfg(target_os = "android")]
pub fn log(level: i32, msg: &str) {
    use std::ffi::CString;
    unsafe extern "C" {
        fn __android_log_write(
            prio: std::ffi::c_int,
            tag: *const std::ffi::c_char,
            text: *const std::ffi::c_char,
        ) -> std::ffi::c_int;
    }
    let tag = c"NMessenger";
    if let Ok(cmsg) = CString::new(msg) {
        unsafe {
            __android_log_write(level, tag.as_ptr(), cmsg.as_ptr());
        }
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC in NMessenger: {info}");
        log(7, &msg); // ANDROID_LOG_FATAL
        eprintln!("{msg}");
    }));

    log(4, "NMessenger android_main starting...");
    *APP.lock().unwrap() = Some(app.clone());
    let options = eframe::NativeOptions {
        run_and_return: false,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("NMessenger")
            .with_decorations(false),
        android_app: Some(app),
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "NMessenger",
        options,
        Box::new(|cc| {
            log(4, "Creating MessengerApp...");
            Ok(Box::new(crate::app::MessengerApp::new(cc)))
        }),
    ) {
        let msg = format!("NMessenger run_native error: {error}");
        log(6, &msg); // ANDROID_LOG_ERROR
        eprintln!("{msg}");
    }
    *APP.lock().unwrap() = None;
    log(4, "NMessenger android_main finished.");
}

pub fn data_directory() -> Option<PathBuf> {
    APP.lock()
        .ok()?
        .as_ref()?
        .internal_data_path()
        .map(|directory| directory.join("nmessenger"))
}

fn with_activity<T>(
    call: impl FnOnce(&mut Env<'_>, &JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let app = APP
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("Atividade Android não inicializada")?;
    // AndroidApp owns these global JNI references; JObject does not delete them.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| -> jni::errors::Result<Result<T, String>> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        Ok(call(env, &activity))
    }).map_err(|e| e.to_string())?
}

pub fn system_locale() -> Option<String> {
    with_activity(|env, activity| {
        let result = env.call_method(
            activity,
            jni::jni_str!("systemLocale"),
            jni::jni_sig!("()Ljava/lang/String;"),
            &[],
        ).map_err(|error| error.to_string())?;
        let value = env.cast_local::<JString>(result.l().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        value.try_to_string(env).map_err(|error| error.to_string())
    }).ok()
}

pub fn notify_message(title: &str, body: &str, conversation: Option<uuid::Uuid>) -> Result<(), String> {
    with_activity(|env, activity| {
        let title = JObject::from(env.new_string(title).map_err(|error| error.to_string())?);
        let body = JObject::from(env.new_string(body).map_err(|error| error.to_string())?);
        let conversation = JObject::from(env.new_string(conversation.map_or_else(String::new, |id| id.to_string())).map_err(|error| error.to_string())?);
        env.call_method(activity, jni::jni_str!("notifyMessage"),
            jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&title), JValue::Object(&body), JValue::Object(&conversation)])
            .map(|_| ()).map_err(|error| error.to_string())
    })
}

pub fn take_notification_conversation() -> Option<uuid::Uuid> {
    with_activity(|env, activity| {
        let value = env.call_method(activity, jni::jni_str!("takeNotificationConversation"),
            jni::jni_sig!("()Ljava/lang/String;"), &[]).map_err(|error| error.to_string())?
            .l().map_err(|error| error.to_string())?;
        if value.is_null() { return Ok(None); }
        let value = env.cast_local::<JString>(value).map_err(|error| error.to_string())?;
        Ok(value.try_to_string(env).map_err(|error| error.to_string())?.parse().ok())
    }).ok().flatten()
}

static CAMERA_CAPTURE: Mutex<()> = Mutex::new(());
static CAMERA_FRAMES: Mutex<Option<std::sync::mpsc::SyncSender<Vec<u8>>>> = Mutex::new(None);
static SCREEN_CAPTURE: Mutex<()> = Mutex::new(());
static SCREEN_FRAMES: Mutex<Option<std::sync::mpsc::SyncSender<Vec<u8>>>> = Mutex::new(None);

#[unsafe(no_mangle)]
extern "system" fn Java_com_nmessenger_app_MainActivity_nativeScreenFrame(
    mut env: EnvUnowned<'_>, _class: JClass<'_>, data: JByteArray<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        if data.len(env)? <= nm_protocol::MAX_MEDIA_FRAME_BYTES {
            let bytes = env.convert_byte_array(&data)?;
            if bytes.starts_with(&[0xff, 0xd8]) && let Ok(sender) = SCREEN_FRAMES.lock()
                && let Some(sender) = sender.as_ref() {
                let _ = sender.try_send(bytes);
            }
        }
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

pub fn capture_screen(
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    mut frame: impl FnMut(&[u8]),
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let _capture = SCREEN_CAPTURE.lock().map_err(|_| "Screen capture unavailable")?;
    if stop.load(Ordering::Relaxed) { return Ok(()); }
    let (sender, receiver) = std::sync::mpsc::sync_channel(2);
    *SCREEN_FRAMES.lock().map_err(|_| "Screen capture unavailable")? = Some(sender);
    let started = with_activity(|env, activity| {
        env.call_method(activity, jni::jni_str!("startScreenCapture"), jni::jni_sig!("()Z"), &[])
            .and_then(|value| value.z()).map_err(|error| error.to_string())
    });
    let result = match started {
        Ok(true) => {
            let started = std::time::Instant::now();
            let mut active_since = None;
            let mut last_frame = None;
            loop {
                if stop.load(Ordering::Relaxed) { break Ok(()); }
                match receiver.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(bytes) => {
                        if !stop.load(Ordering::Relaxed) {
                            last_frame = Some(std::time::Instant::now());
                            frame(&bytes);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break Err("Screen capture stopped.".into()),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
                let status = with_activity(|env, activity| env.call_method(activity, jni::jni_str!("screenCaptureState"),
                    jni::jni_sig!("()I"), &[]).and_then(|value| value.i()).map_err(|error| error.to_string()));
                match status {
                    Ok(2) => { active_since.get_or_insert_with(std::time::Instant::now); }
                    Ok(3) => break Err("Screen sharing was denied or stopped.".into()),
                    Err(error) => break Err(error),
                    _ => {}
                }
                if started.elapsed() > std::time::Duration::from_secs(90) && active_since.is_none() {
                    break Err("Screen sharing permission timed out.".into());
                }
                if last_frame.or(active_since).is_some_and(|since| since.elapsed() > std::time::Duration::from_secs(12)) {
                    break Err("Screen capture did not produce frames.".into());
                }
            }
        }
        Ok(false) => Err("Screen capture is already active.".into()),
        Err(error) => Err(error),
    };
    let _ = with_activity(|env, activity| env.call_method(activity, jni::jni_str!("stopScreenCapture"),
        jni::jni_sig!("()V"), &[]).map(|_| ()).map_err(|error| error.to_string()));
    if let Ok(mut frames) = SCREEN_FRAMES.lock() { *frames = None; }
    result
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_nmessenger_app_MainActivity_nativeCameraFrame(
    mut env: EnvUnowned<'_>, _class: JClass<'_>, data: JByteArray<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        if data.len(env)? <= nm_protocol::MAX_MEDIA_FRAME_BYTES {
            let bytes = env.convert_byte_array(&data)?;
            if bytes.starts_with(&[0xff, 0xd8]) && let Ok(sender) = CAMERA_FRAMES.lock()
                && let Some(sender) = sender.as_ref() {
                let _ = sender.try_send(bytes);
            }
        }
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

pub fn capture_camera(
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    mut frame: impl FnMut(&[u8]),
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let _capture = CAMERA_CAPTURE.lock().map_err(|_| "Camera unavailable")?;
    if stop.load(Ordering::Relaxed) { return Ok(()); }
    let (sender, receiver) = std::sync::mpsc::sync_channel(2);
    *CAMERA_FRAMES.lock().map_err(|_| "Camera unavailable")? = Some(sender);
    let started = with_activity(|env, activity| {
        let allowed = env.call_method(activity, jni::jni_str!("ensureCamera"), jni::jni_sig!("()Z"), &[])
            .and_then(|value| value.z()).map_err(|error| error.to_string())?;
        if !allowed || stop.load(Ordering::Relaxed) { return Ok(false); }
        env.call_method(activity, jni::jni_str!("startCamera"), jni::jni_sig!("()Z"), &[])
            .and_then(|value| value.z()).map_err(|error| error.to_string())
    });
    let result = match started {
        Ok(true) => {
            let mut last_frame = std::time::Instant::now();
            loop {
                if stop.load(Ordering::Relaxed) { break Ok(()); }
                match receiver.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(bytes) => { last_frame = std::time::Instant::now(); frame(&bytes); }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) if last_frame.elapsed() < std::time::Duration::from_secs(12) => {}
                    Err(_) => break Err("Camera stopped producing frames.".into()),
                }
            }
        }
        Ok(false) if stop.load(Ordering::Relaxed) => Ok(()),
        Ok(false) => Err("Camera permission denied or camera unavailable.".into()),
        Err(error) => Err(error),
    };
    let _ = with_activity(|env, activity| {
        env.call_method(activity, jni::jni_str!("stopCamera"), jni::jni_sig!("()V"), &[])
            .map(|_| ()).map_err(|error| error.to_string())
    });
    if let Ok(mut frames) = CAMERA_FRAMES.lock() { *frames = None; }
    result
}

static STATUS_BAR_HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_nmessenger_app_MainActivity_nativeStatusBarHeight(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    height: i32,
) {
    let new_h = height.max(0) as u32;
    let old_h = STATUS_BAR_HEIGHT.swap(new_h, std::sync::atomic::Ordering::Relaxed);
    if old_h != new_h {
        if let Ok(guard) = APP.lock() {
            if let Some(app) = guard.as_ref() {
                app.create_waker().wake();
            }
        }
    }
}

pub fn status_bar_height() -> f32 {
    let cached = STATUS_BAR_HEIGHT.load(std::sync::atomic::Ordering::Relaxed);
    if cached > 0 {
        return cached as f32;
    }
    let h = with_activity(|env, activity| {
        let result = env.call_method(
            activity,
            jni::jni_str!("statusBarHeightDp"),
            jni::jni_sig!("()I"),
            &[],
        ).map_err(|error| error.to_string())?;
        result.i().map_err(|e| e.to_string())
    }).unwrap_or(38).max(28) as u32;
    STATUS_BAR_HEIGHT.store(h, std::sync::atomic::Ordering::Relaxed);
    h as f32
}

static KEYBOARD_HEIGHT_PX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static KEYBOARD_HEIGHT_DP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_nmessenger_app_MainActivity_nativeKeyboardHeight(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    height_px: i32,
    height_dp: i32,
) {
    let new_px = height_px.max(0) as u32;
    let new_dp = height_dp.max(0) as u32;
    let old_px = KEYBOARD_HEIGHT_PX.swap(new_px, std::sync::atomic::Ordering::Relaxed);
    KEYBOARD_HEIGHT_DP.store(new_dp, std::sync::atomic::Ordering::Relaxed);
    if old_px != new_px {
        if let Ok(guard) = APP.lock() {
            if let Some(app) = guard.as_ref() {
                app.create_waker().wake();
            }
        }
    }
}

pub fn safe_area_bottom() -> f32 {
    let h = with_activity(|env, activity| {
        let result = env.call_method(
            activity,
            jni::jni_str!("navigationBarHeightDp"),
            jni::jni_sig!("()I"),
            &[],
        ).map_err(|e| e.to_string())?;
        result.i().map_err(|e| e.to_string())
    }).unwrap_or(0).max(0) as f32;
    h
}

pub fn keyboard_height(ctx: &eframe::egui::Context) -> f32 {
    let mut px = KEYBOARD_HEIGHT_PX.load(std::sync::atomic::Ordering::Relaxed) as f32;
    if px <= 0.0 {
        let from_java = with_activity(|env, activity| {
            let result = env.call_method(
                activity,
                jni::jni_str!("keyboardHeightPx"),
                jni::jni_sig!("()I"),
                &[],
            ).map_err(|e| e.to_string())?;
            result.i().map_err(|e| e.to_string())
        }).unwrap_or(0).max(0) as u32;
        if from_java > 0 {
            KEYBOARD_HEIGHT_PX.store(from_java, std::sync::atomic::Ordering::Relaxed);
            px = from_java as f32;
        }
    }
    if px > 0.0 {
        let ppp = ctx.pixels_per_point();
        if ppp > 0.0 {
            return (px / ppp).round();
        }
    }
    let dp = KEYBOARD_HEIGHT_DP.load(std::sync::atomic::Ordering::Relaxed) as f32;
    if dp > 0.0 {
        let zoom = ctx.zoom_factor().max(0.1);
        return (dp / zoom).round();
    }
    if let Ok(guard) = APP.lock() {
        if let Some(app) = guard.as_ref() {
            let rect = app.content_rect();
            let ppp = ctx.pixels_per_point();
            if rect.bottom > 0 && ppp > 0.0 {
                let screen_h = ctx.content_rect().height() * ppp;
                let diff = screen_h - rect.bottom as f32;
                if diff > 80.0 * ppp {
                    return (diff / ppp).round();
                }
            }
        }
    }
    0.0
}

enum ImeEdit { Text(String), Compose(String), Paste(String), Delete { before: u32, after: u32 }, Action(i32) }
static PENDING_IME_EDITS: Mutex<Vec<ImeEdit>> = Mutex::new(Vec::new());
static PENDING_KEYBOARD_BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn queue_edit_action(action: i32) {
    if let Ok(mut edits) = PENDING_IME_EDITS.lock() { edits.push(ImeEdit::Action(action)); }
}

pub fn queue_paste(text: String) {
    if let Ok(mut edits) = PENDING_IME_EDITS.lock() { edits.push(ImeEdit::Paste(text)); }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MainActivity_nativeComposingText(
    mut env: EnvUnowned<'_>, _class: JClass<'_>, text: JString<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        if !text.is_null() {
            let text = text.try_to_string(env)?;
            if let Ok(mut edits) = PENDING_IME_EDITS.lock() { edits.push(ImeEdit::Compose(text)); }
            if let Ok(guard) = APP.lock() { if let Some(app) = guard.as_ref() { app.create_waker().wake(); } }
        }
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MainActivity_nativeEditAction(
    _env: *mut std::ffi::c_void, _class: *mut std::ffi::c_void, action: i32,
) {
    if matches!(action, 1 | 2 | 4) {
        if let Ok(mut edits) = PENDING_IME_EDITS.lock() { edits.push(ImeEdit::Action(action)); }
        if let Ok(guard) = APP.lock() { if let Some(app) = guard.as_ref() { app.create_waker().wake(); } }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MainActivity_nativeBackFromKeyboard(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
) {
    PENDING_KEYBOARD_BACK.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Ok(guard) = APP.lock() {
        if let Some(app) = guard.as_ref() {
            app.create_waker().wake();
        }
    }
}

pub fn take_keyboard_back() -> bool {
    PENDING_KEYBOARD_BACK.swap(false, std::sync::atomic::Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MainActivity_nativeCommitText(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    text: JString<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        if !text.is_null() {
            let s = text.try_to_string(env)?;
            if !s.is_empty() {
                if let Ok(mut lock) = PENDING_IME_EDITS.lock() {
                    lock.push(ImeEdit::Text(s));
                }
                if let Ok(guard) = APP.lock() {
                    if let Some(app) = guard.as_ref() {
                        app.create_waker().wake();
                    }
                }
            }
        }
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nmessenger_app_MainActivity_nativeDeleteSurroundingText(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    before: i32,
    after: i32,
) {
    if before > 0 || after > 0 {
        if let Ok(mut edits) = PENDING_IME_EDITS.lock() {
            edits.push(ImeEdit::Delete { before: before.clamp(0, 1000) as u32, after: after.clamp(0, 1000) as u32 });
        }
        if let Ok(guard) = APP.lock() {
            if let Some(app) = guard.as_ref() {
                app.create_waker().wake();
            }
        }
    }
}

pub fn poll_ime_events(ctx: &eframe::egui::Context) {
    let edits = if let Ok(mut lock) = PENDING_IME_EDITS.lock() {
        std::mem::take(&mut *lock)
    } else {
        Vec::new()
    };

    if !edits.is_empty() {
        ctx.input_mut(|i| {
            for edit in edits {
                let text = match edit {
                    ImeEdit::Text(text) => text,
                    ImeEdit::Compose(text) => {
                        i.events.push(eframe::egui::Event::Ime(eframe::egui::ImeEvent::Preedit { text, active_range_chars: None }));
                        continue;
                    }
                    ImeEdit::Paste(text) => { i.events.push(eframe::egui::Event::Paste(text)); continue; }
                    ImeEdit::Action(action) => {
                        match action {
                            1 => i.events.push(eframe::egui::Event::Copy),
                            2 => i.events.push(eframe::egui::Event::Cut),
                            4 => i.events.push(eframe::egui::Event::Key {
                                key: eframe::egui::Key::A, physical_key: Some(eframe::egui::Key::A),
                                pressed: true, repeat: false,
                                modifiers: eframe::egui::Modifiers { ctrl: true, command: true, ..Default::default() },
                            }),
                            _ => {},
                        }
                        continue;
                    }
                    ImeEdit::Delete { before, after } => {
                        for (key, count) in [(eframe::egui::Key::Backspace, before), (eframe::egui::Key::Delete, after)] {
                            for _ in 0..count {
                                for pressed in [true, false] {
                                    i.events.push(eframe::egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: eframe::egui::Modifiers::NONE });
                                }
                            }
                        }
                        continue;
                    }
                };
                if text == "\n" {
                    i.events.push(eframe::egui::Event::Key {
                        key: eframe::egui::Key::Enter,
                        physical_key: Some(eframe::egui::Key::Enter),
                        pressed: true,
                        repeat: false,
                        modifiers: eframe::egui::Modifiers::NONE,
                    });
                    i.events.push(eframe::egui::Event::Key {
                        key: eframe::egui::Key::Enter,
                        physical_key: Some(eframe::egui::Key::Enter),
                        pressed: false,
                        repeat: false,
                        modifiers: eframe::egui::Modifiers::NONE,
                    });
                } else {
                    i.events.push(eframe::egui::Event::Ime(eframe::egui::ImeEvent::Commit(text)));
                }
            }
        });
        ctx.request_repaint();
    }
}

static CURRENT_INPUT_TYPE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn show_soft_input() {
    let current = {
        CURRENT_INPUT_TYPE.lock().unwrap_or_else(|e| e.into_inner()).clone()
    };
    if current.is_empty() || current == "password" {
        show_soft_input_type("text");
    } else {
        show_soft_input_type(&current);
    }
}

pub fn show_soft_input_type(input_type: &str) {
    {
        let mut cur = CURRENT_INPUT_TYPE.lock().unwrap_or_else(|e| e.into_inner());
        *cur = input_type.to_string();
    }
    let _ = with_activity(|env, activity| {
        let jtype = JObject::from(env.new_string(input_type).map_err(|e| e.to_string())?);
        env.call_method(
            activity,
            jni::jni_str!("showKeyboard"),
            jni::jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&jtype)],
        ).map_err(|e| e.to_string())?;
        Ok(())
    });
}

pub fn hide_soft_input() {
    {
        let mut cur = CURRENT_INPUT_TYPE.lock().unwrap_or_else(|e| e.into_inner());
        cur.clear();
    }
    let _ = with_activity(|env, activity| {
        env.call_method(
            activity,
            jni::jni_str!("hideKeyboard"),
            jni::jni_sig!("()V"),
            &[],
        ).map_err(|e| e.to_string())?;
        Ok(())
    });
}

pub fn get_clipboard_text() -> Option<String> {
    with_activity(|env, activity| {
        let result = env.call_method(
            activity,
            jni::jni_str!("getClipboardText"),
            jni::jni_sig!("()Ljava/lang/String;"),
            &[],
        ).map_err(|e| e.to_string())?;
        let obj = result.l().map_err(|e| e.to_string())?;
        if obj.is_null() {
            return Ok(None);
        }
        let jstr = env.cast_local::<JString>(obj).map_err(|e| e.to_string())?;
        jstr.try_to_string(env).map(Some).map_err(|e| e.to_string())
    }).ok().flatten()
}

pub fn set_clipboard_text(text: &str) {
    let _ = with_activity(|env, activity| {
        let jtext = env.new_string(text).map_err(|e| e.to_string())?;
        env.call_method(
            activity,
            jni::jni_str!("setClipboardText"),
            jni::jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&jtext)],
        ).map_err(|e| e.to_string())?;
        Ok(())
    });
}

pub async fn document(filter: &'static str, save: Option<String>) -> Selection {
    let (sender, receiver) = oneshot::channel();
    {
        let mut pending = DOCUMENT.lock().map_err(|e| e.to_string())?;
        if pending.is_some() {
            return Err("Outro seletor de arquivos está aberto.".into());
        }
        *pending = Some(sender);
    }
    let launch = with_activity(|env, activity| {
        let mime = match filter {
            "image" => "image/*",
            "audio" => "audio/*",
            _ => "*/*",
        };
        let mime = JObject::from(env.new_string(mime).map_err(|e| e.to_string())?);
        let filename = JObject::from(
            env.new_string(save.as_deref().unwrap_or("arquivo"))
                .map_err(|e| e.to_string())?,
        );
        env.call_method(
            activity,
            jni::jni_str!("selectDocument"),
            jni::jni_sig!("(Ljava/lang/String;ZLjava/lang/String;)V"),
            &[
                JValue::Object(&mime),
                JValue::Bool(save.is_some()),
                JValue::Object(&filename),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    });
    if let Err(error) = launch {
        DOCUMENT.lock().unwrap().take();
        return Err(error);
    }
    receiver
        .await
        .map_err(|_| "Seleção de arquivo interrompida".to_owned())?
}

#[unsafe(no_mangle)]
extern "system" fn Java_com_nmessenger_app_MainActivity_nativeDocumentResult(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    path: JString<'_>,
    error: JString<'_>,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let result = if !error.is_null() {
            Err(error.try_to_string(env).unwrap_or_else(|error| error.to_string()))
        } else if path.is_null() {
            Ok(None)
        } else {
            path.try_to_string(env).map(|text| Some(PathBuf::from(text))).map_err(|e| e.to_string())
        };
        if let Ok(mut pending) = DOCUMENT.lock()
            && let Some(sender) = pending.take() {
            let _ = sender.send(result);
        }
        Ok(())
    }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

pub fn finish_save(path: PathBuf) -> Result<(), String> {
    with_activity(|env, activity| {
        let path = JObject::from(
            env.new_string(path.to_string_lossy())
                .map_err(|e| e.to_string())?,
        );
        let result = env
            .call_method(
                activity,
                jni::jni_str!("commitDocument"),
                jni::jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&path)],
            )
            .map_err(|e| e.to_string())?
            .l()
            .map_err(|e| e.to_string())?;
        if result.is_null() {
            Ok(())
        } else {
            Err(env.cast_local::<JString>(result).map_err(|e| e.to_string())?.try_to_string(env).map_err(|e| e.to_string())?)
        }
    })
}

pub fn microphone_permission() -> Result<(), String> {
    with_activity(|env, activity| {
        if env
            .call_method(activity, jni::jni_str!("ensureMicrophone"), jni::jni_sig!("()Z"), &[])
            .map_err(|e| e.to_string())?
            .z()
            .map_err(|e| e.to_string())?
        {
            Ok(())
        } else {
            Err("Acesso ao microfone não autorizado no Android.".into())
        }
    })
}

pub fn attention() {
    let _ = with_activity(|env, activity| {
        env.call_method(activity, jni::jni_str!("attention"), jni::jni_sig!("()V"), &[])
            .map_err(|error| error.to_string())?;
        Ok(())
    });
}

pub fn credential(
    account: &str,
    password: Option<&str>,
    remove: bool,
) -> Result<Option<String>, String> {
    with_activity(|env, activity| {
        let account = JObject::from(env.new_string(account).map_err(|error| error.to_string())?);
        let password = match password {
            Some(value) => JObject::from(env.new_string(value).map_err(|error| error.to_string())?),
            None => JObject::null(),
        };
        let value = env
            .call_method(
                activity,
                jni::jni_str!("credential"),
                jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;Z)Ljava/lang/String;"),
                &[
                    JValue::Object(&account),
                    JValue::Object(&password),
                    JValue::Bool(remove),
                ],
            )
            .map_err(|_| {
                "Não foi possível acessar o Android Keystore; nenhuma senha foi salva em texto puro"
                    .to_owned()
            })?
            .l()
            .map_err(|error| error.to_string())?;
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(
                env.cast_local::<JString>(value).map_err(|error| error.to_string())?.try_to_string(env).map_err(|error| error.to_string())?,
            ))
        }
    })
}
