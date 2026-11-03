#[cfg(target_os = "android")]
use crate::{app::MessengerApp, i18n::tr, transport::Action};
#[cfg(target_os = "android")]
use eframe::egui::{Color32, RichText};
#[cfg(target_os = "android")]
use nm_protocol::{Activity, ActivityStatus, Conversation, UpdateActivityRequest};
use eframe::egui::{self, Vec2};

/// Full-screen two-person video view. Leaving this view does not end the call.
#[cfg(target_os = "android")]
pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation, activity: &Activity) {
    egui::Panel::top("mobile-video-header")
        .exact_size(54.0).show_separator_line(false)
        .frame(egui::Frame::new().fill(Color32::from_rgb(24, 35, 51)).inner_margin(8))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                if ui.button(RichText::new("‹").size(22.0).color(Color32::WHITE)).clicked() {
                    app.mobile_call_view = None;
                }
                ui.label(RichText::new(if activity.kind == nm_protocol::ActivityKind::VideoCall || app.media_status.as_ref().is_some_and(|status| status.camera_enabled) {
                    tr("Video call", "Videochamada")
                } else { tr("Voice call", "Chamada de voz") }).strong().color(Color32::WHITE));
            });
        });
    egui::Panel::bottom("mobile-video-controls")
        .exact_size(84.0).show_separator_line(false)
        .frame(egui::Frame::new().fill(Color32::from_rgb(24, 35, 51)).inner_margin(10))
        .show(ui, |ui| {
            let muted = app.media_status.as_ref().is_some_and(|status| status.muted);
            let camera = app.media_status.as_ref().map_or(activity.kind == nm_protocol::ActivityKind::VideoCall, |status| status.camera_enabled);
            let sharing = app.media_status.as_ref().is_some_and(|status| status.screen_sharing);
            ui.horizontal_centered(|ui| {
                let width = ((ui.available_width() - 3.0 * ui.spacing().item_spacing.x) / 4.0).max(45.0);
                if ui.add_sized([width, 34.0], egui::Button::new(if muted { tr("Unmute", "Ativar mic.") } else { tr("Mute", "Silenciar") })).clicked() {
                    if let Some(status) = &mut app.media_status { status.muted = !muted; }
                    app.send(Action::MuteMicrophone(!muted));
                }
                if ui.add_sized([width, 34.0], egui::Button::new(if camera { tr("Camera off", "Desligar cam.") } else { tr("Camera on", "Ligar câmera") })).clicked() {
                    app.set_camera_enabled(!camera);
                }
                if ui.add_enabled(crate::media::remote::available(), egui::Button::new(if sharing { tr("Stop share", "Parar tela") } else { tr("Share screen", "Transmitir") }).min_size(Vec2::new(width, 34.0))).clicked() {
                    app.set_screen_sharing(!sharing);
                }
                if ui.add(egui::Button::new(RichText::new(tr("End", "Encerrar")).color(Color32::WHITE))
                    .min_size(Vec2::new(width, 34.0))
                    .fill(Color32::from_rgb(195, 62, 68)).stroke(egui::Stroke::NONE)).clicked() {
                    app.send(Action::StopCall);
                    app.send(Action::UpdateActivity(activity.id, UpdateActivityRequest { status: ActivityStatus::Ended, payload: None }));
                    app.mobile_call_view = None;
                }
            });
            if app.media_status.as_ref().is_some_and(|status| !status.errors.is_empty()) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Device unavailable", "Dispositivo indisponível")).small().color(Color32::from_rgb(255, 202, 128)));
                    if ui.small_button(tr("Devices", "Dispositivos")).clicked() {
                        app.open_options(); app.options.tab = 7;
                    }
                });
            }
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::from_rgb(16, 24, 35)).inner_margin(2))
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let (top, bottom) = split_video_area(rect);
            let me = app.data.as_ref().map(|data| data.me.id);
            let remote = conversation.members.iter().find(|profile| Some(profile.id) != me);
            let local = conversation.members.iter().find(|profile| Some(profile.id) == me);
            tile(app, ui, top, activity, remote, tr("Contact", "Contato"));
            tile(app, ui, bottom, activity, local, tr("You", "Você"));
        });
}

fn split_video_area(rect: egui::Rect) -> (egui::Rect, egui::Rect) {
    let gap = 2.0_f32.min(rect.height().max(0.0) / 2.0);
    (egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.center().y - gap)),
     egui::Rect::from_min_max(egui::pos2(rect.left(), rect.center().y + gap), rect.max))
}

#[cfg(target_os = "android")]
fn tile(app: &mut MessengerApp, ui: &mut egui::Ui, rect: egui::Rect, activity: &Activity, profile: Option<&nm_protocol::Profile>, fallback: &str) {
    ui.painter().rect_filled(rect, 4, Color32::from_rgb(36, 49, 66));
    let screen = profile.and_then(|profile| app.screen_frames.get(&(activity.id, profile.id)));
    let frame = screen.or_else(|| profile.and_then(|profile| app.live_frames.get(&(activity.id, profile.id))))
        .map(|texture| (texture.id(), texture.size_vec2()));
    if let Some((texture, size)) = frame {
        if screen.is_some() {
            let scale = (rect.width() / size.x.max(1.0)).min(rect.height() / size.y.max(1.0));
            let fitted = egui::Rect::from_center_size(rect.center(), size * scale);
            ui.painter().image(texture, fitted, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
        } else {
            ui.painter().image(texture, rect, super::nmessenger::cover_uv(size, rect.size()), Color32::WHITE);
        }
    } else if let Some(profile) = profile {
        let side = rect.width().min(rect.height()).min(100.0).max(1.0);
        super::nmessenger::avatar_at(app, ui, profile, egui::Rect::from_center_size(rect.center(), Vec2::splat(side)), "mobile-call");
    }
    let name = profile.map(|profile| profile.display_name.as_str()).unwrap_or(fallback);
    let label = if name.is_empty() { fallback } else { name };
    ui.painter().text(rect.left_bottom() + Vec2::new(10.0, -9.0), egui::Align2::LEFT_BOTTOM,
        label, egui::FontId::proportional(12.0), Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_tiles_share_the_available_mobile_space_without_overlap() {
        for size in [Vec2::new(320.0, 480.0), Vec2::new(360.0, 720.0)] {
            let bounds = egui::Rect::from_min_size(egui::pos2(0.0, 40.0), size);
            let (remote, local) = split_video_area(bounds);
            assert_eq!(remote.top(), bounds.top());
            assert_eq!(local.bottom(), bounds.bottom());
            assert!(remote.bottom() < local.top());
            assert!((remote.height() - local.height()).abs() < 0.01);
        }
    }
}
