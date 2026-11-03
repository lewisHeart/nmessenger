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
