#[cfg(target_os = "android")]
mod android;
mod app;
pub mod bot_sdk;
mod encryption;
pub mod env;
mod identity_vault;
mod i18n;
mod media;
mod platform;
mod resources;
mod storage;
mod transport;
mod updater;
#[cfg(target_os = "linux")]
mod tray;
mod ui;

#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result {
    #[cfg(target_os = "linux")]
    {
        let has_x11 = std::env::var("DISPLAY").map(|v| !v.is_empty()).unwrap_or(false);
        let force_wayland = std::env::var("WAYLAND_FORCE").ok().as_deref() == Some("1")
            || std::env::var("WINIT_UNIX_BACKEND").ok().as_deref() == Some("wayland");

        if has_x11 && !force_wayland {
            // Under Wayland, winit's multi-window immediate viewports (chats, settings,
            // dialogs) suffer from compositor buffer scaling artifacts during interactive
            // resize because child viewports cannot render synchronously with configure.
            // X11 / XWayland provides rock-solid multi-window resizing and matches the
            // packaging intent (packaging/linux/nmessenger-launch).
            unsafe {
                std::env::remove_var("WAYLAND_DISPLAY");
                std::env::remove_var("WAYLAND_SOCKET");
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("NMessenger")
            .with_inner_size(ui::nmessenger::MAIN_SIZE)
            .with_min_inner_size([290.0, 440.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(ui::nmessenger::app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "NMessenger",
        options,
        Box::new(|cc| Ok(Box::new(app::MessengerApp::new(cc)))),
    )
}
mod secure;
