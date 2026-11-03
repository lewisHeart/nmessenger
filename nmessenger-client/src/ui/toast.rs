use super::{nmessenger, theme};
use crate::app::MessengerApp;
use eframe::egui::{self, Color32, RichText, Vec2};
use uuid::Uuid;

enum Outcome {
    Stay,
    Dismiss(Uuid),
    Open(Uuid, Uuid),
}

/// A small always-on-top window near the tray, like classic messenger alerts.
pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    let Some(toast) = app.toasts.last().cloned() else {
        return;
    };
    let mut outcome = Outcome::Stay;
    let native = super::window::system_titlebars(ctx);

    let gnome = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| desktop.to_ascii_lowercase().contains("gnome"));
    let initial_pos = if let Some((w, _h, off_x, off_y)) = target_monitor_bounds(ctx) {
        Some(toast_position(w, off_x, off_y, gnome))
    } else {
        ctx.input(|input| input.viewport().monitor_size)
            .map(|m| toast_position(m.x, 0.0, 0.0, gnome))
    };

    let mut builder = egui::ViewportBuilder::default()
        .with_title("NMessenger - Notificação")
        .with_inner_size([336.0, 106.0])
        .with_resizable(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_active(false)
        .with_decorations(native)
        .with_transparent(true);

    if let Some(pos) = initial_pos {
        builder = builder.with_position(pos);
    }

    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("nmessenger-toast"),
        builder,
        |ui, _class| {
            let ctx = ui.ctx().clone();
            let key = egui::Id::unique(("nmessenger-toast-position", toast.id));
            // Top-right corner near system tray
            if ctx.data(|data| data.get_temp::<bool>(key)) != Some(true) {
                let target_pos = if let Some((w, _h, off_x, off_y)) = target_monitor_bounds(&ctx) {
                    Some(toast_position(w, off_x, off_y, gnome))
                } else {
                    ctx.input(|input| input.viewport().monitor_size)
                        .map(|monitor| toast_position(monitor.x, 0.0, 0.0, gnome))
                };

                if let Some(pos) = target_pos {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                    ctx.data_mut(|data| data.insert_temp(key, true));
                }
            }
            if native {
                theme::window_bar(ui, "NMessenger");
            }
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(10),
                )
                .show(ui, |ui| {
                    let frame = egui::Frame::new()
                        .fill(Color32::from_rgb(248, 252, 255))
                        .corner_radius(9)
                        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(203, 224, 242)))
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                nmessenger::avatar(app, ui, &toast.profile, 52.0);
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&toast.profile.display_name)
                                                    .size(12.0)
                                                    .strong()
                                                    .color(nmessenger::HEADING),
                                            )
                                            .truncate(),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(egui::Button::new("×").frame(false))
                                                    .on_hover_text("Fechar")
                                                    .clicked()
                                                {
                                                    outcome = Outcome::Dismiss(toast.id);
                                                }
                                            },
                                        );
                                    });
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&toast.text)
                                                .size(10.5)
                                                .color(Color32::from_rgb(84, 108, 128)),
                                        )
                                        .truncate(),
                                    );
                                    ui.add_space(4.0);
                                    let progress = (1.0
                                        - toast.at.elapsed().as_secs_f32()
                                            / crate::app::TOAST_SECONDS)
                                        .clamp(0.0, 1.0);
                                    let (rect, _) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), 3.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        rect,
                                        2,
                                        Color32::from_rgb(226, 237, 246),
                                    );
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_size(
                                            rect.min,
                                            Vec2::new(rect.width() * progress, rect.height()),
                                        ),
                                        2,
                                        nmessenger::BLUE,
                                    );
                                });
                            });
                        });
                    let response = ui.interact(
                        frame.response.rect,
                        ui.scope_id().with(("toast-click", toast.id)),
                        egui::Sense::click(),
                    );
                    if response.clicked()
                        && let Some(conversation) = toast.conversation
                    {
                        outcome = Outcome::Open(toast.id, conversation);
                    }
                });
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        },
    );
    match outcome {
        Outcome::Stay => {}
        Outcome::Dismiss(id) => app.dismiss_toast(id),
        Outcome::Open(id, conversation) => {
            app.dismiss_toast(id);
            app.open_conversation(conversation);
        }
    }
}

fn toast_position(width: f32, x: f32, y: f32, gnome: bool) -> egui::Pos2 {
    egui::pos2(if gnome { x + (width - 336.0) / 2.0 } else { x + width - 336.0 - 16.0 }, y + 48.0)
}

fn target_monitor_bounds(ctx: &egui::Context) -> Option<(f32, f32, f32, f32)> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("xrandr")
            .arg("--listmonitors")
            .output()
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut monitors = Vec::new();
            for line in text.lines().skip(1) {
                if let Some(geom) = line.split_whitespace().nth(2)
                    && let Some((w, h, x, y)) = parse_xrandr_geom(geom)
                {
                    monitors.push((w, h, x, y));
                }
            }
            if !monitors.is_empty() {
                let ppp = ctx.pixels_per_point().max(0.5);
                // Locate the monitor that hosts the main NMessenger window
                let app_center = ctx.input(|i| i.viewport().outer_rect).map(|r| r.center());
                let chosen = if let Some(center) = app_center {
                    let phys_x = center.x * ppp;
                    let phys_y = center.y * ppp;
                    monitors
                        .iter()
                        .find(|(w, h, x, y)| {
                            phys_x >= *x && phys_x < *x + *w && phys_y >= *y && phys_y < *y + *h
                        })
                        .copied()
                } else {
                    None
                };

                let (w, h, x, y) = chosen
                    .or_else(|| monitors.iter().find(|m| m.2.abs() < 1.0).copied())
                    .unwrap_or(monitors[0]);

                return Some((w / ppp, h / ppp, x / ppp, y / ppp));
            }
        }
    }
    let _ = ctx;
    None
}

fn parse_xrandr_geom(geom: &str) -> Option<(f32, f32, f32, f32)> {
    if !geom.contains('x') {
        return None;
    }
    let (w_part, rest) = geom.split_once('x')?;
    let w: f32 = w_part.split('/').next()?.parse().ok()?;

    let sign_indices: Vec<usize> = rest
        .char_indices()
        .filter_map(|(i, c)| if c == '+' || c == '-' { Some(i) } else { None })
        .collect();

    if sign_indices.len() < 2 {
        return None;
    }

    let x_idx = sign_indices[sign_indices.len() - 2];
    let y_idx = sign_indices[sign_indices.len() - 1];

    let h: f32 = rest[..x_idx].split('/').next()?.parse().ok()?;
    let x: f32 = rest[x_idx..y_idx].parse().ok()?;
    let y: f32 = rest[y_idx..].parse().ok()?;

    Some((w, h, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_various_xrandr_monitor_geometries() {
        assert_eq!(
            parse_xrandr_geom("5760/630x3240/360+5760+0"),
            Some((5760.0, 3240.0, 5760.0, 0.0))
        );
        assert_eq!(
            parse_xrandr_geom("1920/508x1080/286+0+0"),
            Some((1920.0, 1080.0, 0.0, 0.0))
        );
        assert_eq!(
            parse_xrandr_geom("1920x1080-1920+0"),
            Some((1920.0, 1080.0, -1920.0, 0.0))
        );
        assert_eq!(
            parse_xrandr_geom("2560/597x1440/336+1920-50"),
            Some((2560.0, 1440.0, 1920.0, -50.0))
        );
    }

    #[test]
    fn gnome_toast_is_centered_on_the_selected_monitor() {
        assert_eq!(toast_position(1920.0, -1920.0, 0.0, true), egui::pos2(-1128.0, 48.0));
        assert_eq!(toast_position(1920.0, 0.0, 0.0, false), egui::pos2(1568.0, 48.0));
    }
}
