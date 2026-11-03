use std::path::PathBuf;

pub fn is_mobile() -> bool {
    if cfg!(target_os = "android") || cfg!(target_os = "ios") {
        return true;
    }
    if std::env::var_os("NM_MOBILE").is_some() {
        return true;
    }
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let d = desktop.to_ascii_lowercase();
        if d.contains("phosh") || d.contains("plasma-mobile") || d.contains("sxmo") {
            return true;
        }
    }
    false
}

#[cfg(target_os = "android")]
pub fn data_directory() -> Option<PathBuf> {
    crate::android::data_directory()
}
#[cfg(not(target_os = "android"))]
#[cfg_attr(test, allow(dead_code))]
pub fn data_directory() -> Option<PathBuf> {
    None
}

pub async fn pick_file(filter: &'static str) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "android")]
    {
        crate::android::document(filter, None).await
    }
    #[cfg(not(target_os = "android"))]
    {
        tokio::task::spawn_blocking(move || {
            let dialog = rfd::FileDialog::new();
            match filter {
                "image" => dialog
                    .add_filter("Imagens PNG, JPEG e GIF", &["png", "jpg", "jpeg", "gif"])
                    .pick_file(),
                "contact" => dialog
                    .add_filter("Contatos do Messenger", &["ctt", "xml", "json"])
                    .pick_file(),
                "audio" => dialog.add_filter("Áudio WAV", &["wav"]).pick_file(),
                _ => dialog.pick_file(),
            }
        })
        .await
        .map_err(|e| e.to_string())
    }
}

pub async fn pick_folder(directory: Option<String>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "android")]
    {
        let _ = directory;
        Ok(None)
    }
    #[cfg(not(target_os = "android"))]
    {
        tokio::task::spawn_blocking(move || {
            let mut dialog = rfd::FileDialog::new();
            if let Some(directory) = directory.filter(|value| !value.is_empty()) {
                dialog = dialog.set_directory(directory);
            }
            dialog.pick_folder()
        })
        .await
        .map_err(|e| e.to_string())
    }
}

pub async fn save_file(name: String, directory: Option<String>) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "android")]
    {
        let _ = directory;
        crate::android::document("all", Some(name)).await
    }
    #[cfg(not(target_os = "android"))]
    {
        tokio::task::spawn_blocking(move || {
            let mut dialog = rfd::FileDialog::new().set_file_name(name);
            if let Some(directory) = directory.filter(|value| !value.is_empty()) {
                dialog = dialog.set_directory(directory);
            }
            dialog.save_file()
        })
        .await
        .map_err(|e| e.to_string())
    }
}

pub async fn finish_save(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || crate::android::finish_save(path))
            .await
            .map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = path;
        Ok(())
    }
}

pub fn microphone_permission() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        crate::android::microphone_permission()
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(())
    }
}

pub fn safe_area_top() -> f32 {
    #[cfg(target_os = "android")]
    {
        crate::android::status_bar_height()
    }
    #[cfg(not(target_os = "android"))]
    {
        0.0
    }
}

pub fn safe_area_bottom() -> f32 {
    #[cfg(target_os = "android")]
    {
        crate::android::safe_area_bottom()
    }
    #[cfg(not(target_os = "android"))]
    {
        0.0
    }
}

pub fn keyboard_height(ctx: &eframe::egui::Context) -> f32 {
    #[cfg(target_os = "android")]
    {
        crate::android::keyboard_height(ctx)
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Ok(val) = std::env::var("NM_KEYBOARD_HEIGHT") {
            if let Ok(h) = val.parse::<f32>() {
                return h;
            }
        }
        let _ = ctx;
        0.0
    }
}

pub fn show_soft_input() {
    #[cfg(target_os = "android")]
    {
        crate::android::show_soft_input();
    }
}

pub fn show_soft_input_type(input_type: &str) {
    #[cfg(target_os = "android")]
    {
        crate::android::show_soft_input_type(input_type);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = input_type;
    }
}

#[allow(dead_code)]
pub fn hide_soft_input() {
    #[cfg(target_os = "android")]
    {
        crate::android::hide_soft_input();
    }
}

pub fn take_keyboard_back() -> bool {
    #[cfg(target_os = "android")]
    {
        crate::android::take_keyboard_back()
    }
    #[cfg(not(target_os = "android"))]
    {
        false
    }
}

pub fn poll_ime_events(ctx: &eframe::egui::Context) {
    #[cfg(target_os = "android")]
    {
        crate::android::poll_ime_events(ctx);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = ctx;
    }
}

pub fn sync_ime(ctx: &eframe::egui::Context) {
    #[cfg(target_os = "android")]
    {
        let ime = ctx.output(|o| o.ime.clone());
        let ime_active = ime.is_some();
        let is_text_focused = ctx.text_edit_focused() || ctx.egui_wants_keyboard_input();
        let should_be_active = ime_active || is_text_focused;

        let is_password = if let Some(ime) = &ime {
            matches!(ime.purpose, eframe::egui::viewport::IMEPurpose::Password)
        } else {
            false
        };

        ctx.output_mut(|o| {
            o.ime = None;
            for cmd in &o.commands {
                if let eframe::egui::OutputCommand::CopyText(text) = cmd {
                    crate::android::set_clipboard_text(text);
                }
            }
        });

        static LAST_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        static LAST_PASSWORD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        static INACTIVE_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

        let was_active = LAST_ACTIVE.swap(should_be_active, std::sync::atomic::Ordering::Relaxed);
        let was_password = LAST_PASSWORD.swap(is_password, std::sync::atomic::Ordering::Relaxed);

        let keyboard_visible = crate::android::keyboard_height(ctx) > 0.0;
        let any_click = ctx.input(|i| i.pointer.any_click());

        if should_be_active {
            INACTIVE_FRAMES.store(0, std::sync::atomic::Ordering::Relaxed);
            if !was_active || (was_password != is_password) || (any_click && !keyboard_visible) {
                if is_password {
                    crate::android::show_soft_input_type("password");
                } else {
                    crate::android::show_soft_input();
                }
            }
        } else if was_active {
            let frames = INACTIVE_FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if frames >= 4 {
                crate::android::hide_soft_input();
            } else {
                LAST_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = ctx;
    }
}

pub fn get_clipboard_text() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        crate::android::get_clipboard_text()
    }
    #[cfg(not(target_os = "android"))]
    {
        let mut cb = arboard::Clipboard::new().ok()?;
        cb.get_text().ok()
    }
}

pub fn set_clipboard_text(text: &str) {
    #[cfg(target_os = "android")]
    {
        crate::android::set_clipboard_text(text);
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(text);
        }
    }
}
