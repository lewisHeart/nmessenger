use super::nmessenger;
use eframe::egui::{self, Color32, Rect, Vec2};

#[path = "wink_animation.rs"]
mod animation;

pub fn details(kind: &str) -> (&'static str, &'static str) {
    animation::clip(kind)
        .map(|clip| (clip.name, clip.preview))
        .unwrap_or(("Piscadela", "emojis/sparkles.png"))
}

pub fn picker(ui: &mut egui::Ui) -> Option<&'static str> {
    let mut selected = None;
    ui.set_min_width(250.0);
    ui.label(
        egui::RichText::new("Piscadelas")
            .strong()
            .color(nmessenger::HEADING),
    );
    ui.add_space(8.0);
    egui::Grid::new("wink-gallery")
        .num_columns(3)
        .spacing(Vec2::splat(6.0))
        .show(ui, |ui| {
            for (index, clip) in animation::PICKER.iter().filter_map(|kind| animation::clip(kind)).enumerate() {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(76.0, 84.0), egui::Sense::click());
                let image =
                    Rect::from_center_size(rect.center() - Vec2::new(0.0, 10.0), Vec2::splat(56.0));
                let mut animated = false;
                if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, 6, Color32::from_rgb(230, 243, 255));
                    if let Some(movie) = animation::load(ui.ctx(), clip) {
                        let time = ui.input(|input| input.time);
                        let frame = movie.frame_at((time * 1000.0) as u64 % movie.duration_ms);
                        animation::paint(
                            ui.ctx(),
                            ui.painter(),
                            clip,
                            &movie,
                            frame,
                            image,
                            1.0,
                            true,
                        );
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(
                                movie.frames[frame].delay_ms,
                            ));
                        animated = true;
                    }
                }
                if !animated {
                    nmessenger::paint(ui.ctx(), ui.painter(), clip.preview, image);
                }
                ui.painter().text(
                    rect.center_bottom() - Vec2::new(0.0, 11.0),
                    egui::Align2::CENTER_CENTER,
                    clip.name,
                    egui::FontId::proportional(11.0),
                    nmessenger::HEADING,
                );
                if response
                    .on_hover_text(format!("Prévia animada · Enviar {}", clip.name))
                    .clicked()
                {
                    let _ = animation::load(ui.ctx(), clip);
                    selected = Some(clip.kind);
                }
                if index % 3 == 2 {
                    ui.end_row();
                }
            }
        });
    selected
}

#[derive(Clone)]
struct Playback {
    event_origin: f64,
    started: f64,
    duration_ms: u64,
}

pub fn effect(ui: &egui::Ui, rect: Rect, kind: &str, seconds: f32) {
    if !(0.0..30.0).contains(&seconds) || !rect.is_positive() {
        return;
    }
    let Some(clip) = animation::clip(kind) else {
        return;
    };
    let now = ui.input(|input| input.time);
    let event_origin = now - f64::from(seconds);
    let key = ui.scope_id().with(("wink-playback", clip.kind));
    let finished = ui.ctx().data(|data| {
        data.get_temp::<Playback>(key).is_some_and(|state| {
            (state.event_origin - event_origin).abs() <= 0.1
                && (now - state.started) * 1000.0 >= state.duration_ms as f64
        })
    });
    if finished {
        return;
    }
    let Some(movie) = animation::load(ui.ctx(), clip) else {
        return;
    };
    let duration_ms = movie.playback_duration_ms();
    let playback = ui.ctx().data_mut(|data| {
        let state = data.get_temp_mut_or_insert_with(key, || Playback {
            event_origin,
            started: now,
            duration_ms,
        });
        if (state.event_origin - event_origin).abs() > 0.1 {
            *state = Playback {
                event_origin,
                started: now,
                duration_ms,
            };
        }
        state.clone()
    });
    // Repeat short clips for a few seconds, always ending on a complete cycle.
    let elapsed_ms = ((now - playback.started).max(0.0) * 1000.0) as u64;
    if elapsed_ms >= duration_ms {
        return;
    }
    let frame = movie.frame_at(elapsed_ms % movie.duration_ms);
    let alpha =
        (elapsed_ms as f32 / 100.0).min(1.0) * ((duration_ms - elapsed_ms) as f32 / 140.0).min(1.0);
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            ui.scope_id().with("wink-overlay"),
        ))
        .with_clip_rect(rect);
    // Keep the effect prominent while avoiding excessive enlargement of 256px artwork.
    let side = (rect.width().min(rect.height()) * 0.62)
        .min(320.0)
        .min(384.0 / ui.ctx().pixels_per_point());
    painter.rect_filled(rect, 0, Color32::from_black_alpha((20.0 * alpha) as u8));
    let image = Rect::from_center_size(rect.center(), Vec2::splat(side));
    animation::paint(ui.ctx(), &painter, clip, &movie, frame, image, alpha, false);
    let frame_remaining = movie.frame_end_ms(frame) - elapsed_ms % movie.duration_ms;
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(
            frame_remaining.min(duration_ms - elapsed_ms).max(1),
        ));
}
