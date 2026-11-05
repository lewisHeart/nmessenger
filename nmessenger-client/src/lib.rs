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
        glow_options: eframe::egui_glow::GlowConfiguration {
            vsync: false,
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "NMessenger",
        options,
        Box::new(|cc| {
            let app = std::sync::Arc::new(std::sync::Mutex::new(app::MessengerApp::new(cc)));
            app.lock().unwrap().shared = Some(std::sync::Arc::clone(&app));
            Ok(Box::new(DesktopApp { app }))
        }),
    )
}

#[cfg(not(target_os = "android"))]
struct DesktopApp {
    app: std::sync::Arc<std::sync::Mutex<app::MessengerApp>>,
}

#[cfg(not(target_os = "android"))]
impl eframe::App for DesktopApp {
    fn clear_color(&self, visuals: &eframe::egui::Visuals) -> [f32; 4] {
        self.app.lock().unwrap().clear_color(visuals)
    }

    fn logic(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.app.lock().unwrap().logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.app.lock().unwrap().ui(ui, frame);
    }
}
mod secure;
