use super::{nmessenger, theme};
use crate::i18n::tr;
use crate::{
    app::MessengerApp,
    media::{audio, playback},
    transport::Action,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::time::Duration;
use uuid::Uuid;

fn time(seconds: f32) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, file: Uuid) {
    ui.push_id(("audio-player", file), |ui| {
        let state = playback::snapshot(file);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().min(270.0), 34.0),
            egui::Sense::hover(),
        );
    theme::gradient(
        ui.painter(),
        rect.shrink(0.5),
        Color32::from_rgb(227, 239, 250),
        Color32::from_rgb(210, 228, 246),
    );
    ui.painter().rect_stroke(
        rect,
        5,
        Stroke::new(0.8_f32, Color32::from_rgb(165, 199, 233)),
        egui::StrokeKind::Inside,
    );
    let play = Rect::from_center_size(rect.left_center() + Vec2::new(16.0, 0.0), Vec2::splat(20.0));
    ui.painter()
        .circle_filled(play.center(), 10.0, Color32::from_rgb(37, 126, 221));
    ui.painter().circle_stroke(
        play.center(),
        10.0,
        Stroke::new(0.8_f32, Color32::from_rgb(30, 90, 163)),
    );
    if state.playing {
        for x in [-3.0, 2.0] {
            ui.painter().rect_filled(
                Rect::from_center_size(play.center() + Vec2::new(x, 0.0), Vec2::new(2.0, 8.0)),
                0,
                Color32::WHITE,
            );
        }
    } else {
        let c = play.center();
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + Vec2::new(-2.5, -4.5),
                c + Vec2::new(-2.5, 4.5),
                c + Vec2::new(4.5, 0.0),
            ],
            Color32::WHITE,
            Stroke::NONE,
        ));
    }
    let response = ui.interact(
        play,
        ui.scope_id().with(("audio-play", file)),
        egui::Sense::click(),
    );
    if response
        .on_hover_text(state.error.as_deref().unwrap_or(if state.playing {
            "Pause"
        } else {
            tr("Play", "Reproduzir")
        }))
        .clicked()
    {
        if audio::AVAILABLE {
            app.send(Action::PlayVoice {
                file,
                output: app.login.prefs.audio_output.clone(),
            });
        } else {
            app.error(tr("Native audio is not enabled in this build.", "Áudio nativo não habilitado nesta compilação."));
        }
    }
    let track = Rect::from_min_max(
        Pos2::new(rect.left() + 35.0, rect.center().y - 2.5),
        Pos2::new((rect.right() - 132.0).max(rect.left() + 40.0), rect.center().y + 2.5),
    );
    ui.painter()
        .rect_filled(track, 3, Color32::from_rgb(203, 220, 238));
    let progress = if state.duration > 0.0 {
        (state.position / state.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = Rect::from_min_max(
        track.min,
        Pos2::new(track.left() + track.width() * progress, track.bottom()),
    );
    ui.painter()
        .rect_filled(filled, 3, Color32::from_rgb(61, 164, 229));
    let thumb = Pos2::new(filled.right(), track.center().y);
    ui.painter().circle_filled(thumb, 4.0, Color32::WHITE);
    ui.painter()
        .circle_stroke(thumb, 4.0, Stroke::new(0.7_f32, nmessenger::BLUE));
    let seek = ui.interact(
        track.expand(5.0),
        ui.scope_id().with(("audio-seek", file)),
        egui::Sense::click_and_drag(),
    );
    if (seek.clicked() || seek.dragged())
        && let Some(pointer) = seek.interact_pointer_pos()
    {
        playback::seek(
            file,
            ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0) * state.duration,
        );
    }
    ui.painter().text(
        rect.right_center() - Vec2::new(70.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        if state.loading {
            tr("Loading...", "Carregando...").into()
        } else {
            format!("{} / {}", time(state.position), time(state.duration))
        },
        FontId::proportional(9.0),
        Color32::from_rgb(70, 106, 147),
    );
    let speed_area = Rect::from_min_size(egui::pos2(rect.right() - 66.0, rect.top() + 4.0), Vec2::new(40.0, 26.0));
    let speed_response = ui.interact(speed_area, ui.scope_id().with(("audio-speed", file)), egui::Sense::click())
        .on_hover_text(tr("Change playback speed", "Alterar velocidade de reprodução"));
    ui.painter().rect_filled(speed_area, 5, if speed_response.hovered() { Color32::from_rgb(200, 226, 249) } else { Color32::from_rgb(232, 243, 252) });
    ui.painter().text(speed_area.center(), egui::Align2::CENTER_CENTER, format!("{:.1}x", state.speed),
        FontId::proportional(10.0), nmessenger::HEADING);
    if speed_response.clicked() { playback::set_speed(file, playback::next_speed(state.speed)); }
    let time_area = Rect::from_min_size(egui::pos2(rect.right() - 24.0, rect.top() + 4.0), Vec2::new(22.0, 26.0));
    ui.painter().text(time_area.center(), egui::Align2::CENTER_CENTER, "...", FontId::proportional(12.0), nmessenger::HEADING);
    let options = ui.interact(
        time_area,
        ui.scope_id().with(("audio-options", file)),
        egui::Sense::click(),
    ).on_hover_text(tr("Audio options", "Opções do áudio"));
    if options.secondary_clicked() {
        egui::Popup::toggle_id(ui.ctx(), options.id.with("popup"));
    }
    egui::Popup::menu(&options).show(|ui| audio_menu(app, ui, file));
    if let Some(error) = &state.error {
        ui.label(
            egui::RichText::new(error)
                .size(10.0)
                .color(Color32::from_rgb(185, 28, 28)),
        );
    }
    if ui.is_rect_visible(rect)
        && state.duration == 0.0
        && !state.loading
        && state.error.is_none()
        && playback::begin_load(file)
    {
        app.send(Action::LoadVoice { file });
    }
    if state.playing || state.loading {
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
    });
}

fn audio_menu(app: &mut MessengerApp, ui: &mut egui::Ui, file: Uuid) {
    ui.push_id(("audio-menu", file), |ui| {
        if ui.button(tr("Forward...", "Encaminhar...")).clicked() {
            app.forward_attachment = Some((file, nm_protocol::MessageKind::Voice));
            app.forward_search.clear();
            ui.close();
        }
        if ui.button(tr("Save audio...", "Salvar áudio...")).clicked() {
            app.send(Action::Download(file));
            ui.close();
        }
        if ui.button(tr("Stop", "Parar")).clicked() {
            app.send(Action::StopPlayback);
            ui.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_speed_button_cycles_without_opening_menu() {
        let _lock = playback::TEST_LOCK.lock().unwrap();
        let file = Uuid::new_v4();
        playback::loaded(file, Ok(vec![0; 16000]));
        let mut app = MessengerApp::new_for_test();
        let ctx = egui::Context::default();
        let mut origin = Pos2::ZERO;
        let input = |events| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
            events, ..Default::default()
        };
        let mut output = ctx.run_ui(input(vec![]), |ui| { origin = ui.cursor().min; show(&mut app, ui, file); });
        output.textures_delta.clear();
        let pointer = origin + Vec2::new(224.0, 17.0);
        for pressed in [true, false] {
            let mut output = ctx.run_ui(input(vec![egui::Event::PointerMoved(pointer), egui::Event::PointerButton {
                pos: pointer, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
            }]), |ui| show(&mut app, ui, file));
            output.textures_delta.clear();
        }
        assert_eq!(playback::snapshot(file).speed, 1.2);
    }
}
