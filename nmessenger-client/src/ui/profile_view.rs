//! Rich Aero Profile View dialog with Orkut-style Markdown bio, badges row, and point counter.
use super::{dialog, nmessenger, theme};
use crate::app::MessengerApp;
use crate::i18n::tr;
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, Vec2};
use nm_protocol::*;

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if app.contact_forms.view_profile.is_none() {
        return;
    }

    let title = if let Some(p) = &app.contact_forms.view_profile {
        let name = p.display_name.trim();
        let email = p.email.trim();
        if !name.is_empty() && !email.is_empty() && name != email {
            format!("{} {name} ({email}) - NMessenger", tr("Profile:", "Perfil de"))
        } else if !name.is_empty() {
            format!("{} {name} - NMessenger", tr("Profile:", "Perfil de"))
        } else {
            tr("User Profile - NMessenger", "Perfil de Usuário - NMessenger").to_string()
        }
    } else {
        tr("User Profile - NMessenger", "Perfil de Usuário - NMessenger").to_string()
    };

    if let Some(shared) = app.shared.clone() {
        if !ctx.embed_viewports() && !crate::platform::is_mobile() {
            dialog::show_deferred(
                shared,
                ctx,
                "user-profile-view",
                &title,
                [480.0, 580.0],
                content,
                |app| {
                    app.contact_forms.view_profile = None;
                },
            );
            return;
        }
    }

    let open = dialog::show(ctx, "user-profile-view", &title, [480.0, 580.0], |ui| {
        content(app, ui)
    });

    if !open {
        app.contact_forms.view_profile = None;
    }
}

fn content(app: &mut MessengerApp, ui: &mut egui::Ui) -> bool {
    let Some(profile) = app.contact_forms.view_profile.clone() else {
        return true;
    };
    let is_me = app
        .data
        .as_ref()
        .map(|d| d.me.id == profile.id)
        .unwrap_or(false);
    let mut done = false;

    // 1. Header Banner & Identity
    egui::Frame::new()
        .fill(Color32::from_rgb(243, 248, 253))
        .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 245)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            let inner_w = ui.available_width();
            ui.set_max_width(inner_w);
            ui.set_width(inner_w);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.horizontal(|ui| {
                // Large Avatar (64x64) with status frame
                theme::avatar(app, ui, &profile, 64.0);
                ui.add_space(12.0);

                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width().max(1.0));
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(&profile.display_name)
                                .size(17.0)
                                .strong()
                                .color(nmessenger::HEADING),
                        ).wrap());
                        let (status_text, status_color) = match profile.presence {
                            Presence::Available => (tr("Available", "Disponível"), Color32::from_rgb(40, 150, 40)),
                            Presence::Busy => (tr("Busy", "Ocupado"), Color32::from_rgb(210, 45, 45)),
                            Presence::Away => (tr("Away", "Ausente"), Color32::from_rgb(215, 140, 20)),
                            Presence::Offline | Presence::Invisible => {
                                ("Offline", Color32::from_rgb(140, 150, 160))
                            }
                        };
                        ui.label(
                            RichText::new(format!("({status_text})"))
                                .size(11.0)
                                .color(status_color),
                        );
                    });

                    if !profile.personal_message.trim().is_empty() {
                        ui.add_space(2.0);
                        ui.add(egui::Label::new(
                            RichText::new(format!("\"{}\"", profile.personal_message.trim()))
                                .size(11.0)
                                .italics()
                                .color(Color32::from_rgb(70, 95, 120)),
                        ).wrap());
                    }

                    ui.add_space(3.0);
                    ui.add(egui::Label::new(
                        RichText::new(&profile.email)
                            .size(10.5)
                            .color(nmessenger::MUTED),
                    ).wrap());

                    if let Some(music) = profile
                        .now_playing
                        .as_deref()
                        .filter(|m| !m.trim().is_empty())
                    {
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            nmessenger::image(ui, "icons/musical-note.png", Vec2::splat(12.0));
                            ui.label(RichText::new(music).size(10.5).color(nmessenger::BLUE));
                        });
                    }
                });
            });
        });

    ui.add_space(10.0);

    // 2. Badges (Selos) Section
    ui.label(
        RichText::new(tr("Badges and Achievements", "Selos e Conquistas"))
            .size(12.5)
            .strong()
            .color(nmessenger::HEADING),
    );
    ui.add_space(4.0);

    // Badges Container: placed side-by-side without name text, full details on hover
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(220, 235, 248)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            let inner_w = ui.available_width();
            ui.set_max_width(inner_w);
            ui.set_width(inner_w);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            if profile.badges.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(tr("No badges earned yet.", "Nenhum selo conquistado ainda."))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(6.0);
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                    for badge in &profile.badges {
                        render_badge_icon_only(ui, badge);
                    }
                });
            }
        });

    ui.add_space(10.0);

    // 3. Biography / "Sobre mim" (Orkut style in Markdown)
    ui.label(
        RichText::new(tr("About me", "Sobre mim"))
            .size(12.5)
            .strong()
            .color(nmessenger::HEADING),
    );
    ui.add_space(4.0);

    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(220, 235, 248)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            let inner_w = ui.available_width();
            ui.set_max_width(inner_w);
            ui.set_width(inner_w);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let is_mob = crate::platform::is_mobile();
            if is_mob {
                ui.set_min_height(80.0);
                super::markdown::render(app, ui, &profile.bio);
            } else {
                ui.set_min_height(120.0);
                ui.set_max_height(180.0);
                super::scroll::vertical()
                    .auto_shrink([false, false])
                    .id_salt("profile-bio-scroll")
                    .show(ui, |ui| {
                        super::markdown::render(app, ui, &profile.bio);
                    });
            }
        });

    ui.add_space(14.0);

    // 4. Quick Actions Footer (properly spaced without overlapping)
    ui.horizontal_wrapped(|ui| {
        if is_me {
            let edit_btn = theme::primary_button(
                ui,
                RichText::new(tr("Edit my profile...", "Editar meu perfil..."))
                    .size(11.5)
                    .color(Color32::from_rgb(1, 45, 147))
                    .strong(),
                Vec2::new(145.0, 28.0),
            );
            if edit_btn.clicked() {
                app.contact_forms.edit_display_name = profile.display_name.clone();
                app.contact_forms.edit_personal_message = profile.personal_message.clone();
                app.contact_forms.edit_bio = profile.bio.clone();
                app.contact_forms.edit_profile_open = true;
                done = true;
            }
        } else {
            let send_btn = theme::primary_button(
                ui,
                RichText::new(tr("Send message", "Enviar mensagem"))
                    .size(11.5)
                    .color(Color32::from_rgb(1, 45, 147))
                    .strong(),
                Vec2::new(130.0, 28.0),
            );
            if send_btn.clicked() {
                app.send(crate::transport::Action::CreateConversation(
                    CreateConversationRequest {
                        member_ids: vec![profile.id],
                        title: None,
                        members_can_invite: true,
                    },
                ));
                done = true;
            }

            // Favorite toggle
            if let Some(contact) = app
                .data
                .as_ref()
                .and_then(|d| d.contacts.iter().find(|c| c.user.id == profile.id))
            {
                let is_fav = contact.favorite;
                let fav_text = if is_fav {
                    tr("Remove from favorites", "Remover dos favoritos")
                } else {
                    tr("Add to favorites", "Adicionar aos favoritos")
                };
                ui.add_space(6.0);
                if ui.button(RichText::new(fav_text).size(11.0)).clicked() {
                    app.send(crate::transport::Action::Contact(
                        profile.id,
                        UpdateContactRequest {
                            favorite: Some(!is_fav),
                            ..Default::default()
                        },
                    ));
                }
            }
        }

        if !crate::platform::is_mobile() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new(tr("Close", "Fechar")).size(11.0)).clicked() {
                    done = true;
                }
            });
        }
    });

    done
}

/// Render a badge icon neatly side-by-side without name text, with a rich Aero tooltip on hover.
fn render_badge_icon_only(ui: &mut egui::Ui, badge: &Badge) -> egui::Response {
    let size = Vec2::splat(42.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let hovered = response.hovered();

    let bg = if hovered {
        Color32::from_rgb(235, 246, 255)
    } else {
        Color32::from_rgb(248, 251, 255)
    };
    let border = if hovered {
        Color32::from_rgb(105, 175, 240)
    } else {
        Color32::from_rgb(215, 230, 245)
    };

    ui.painter().rect_filled(rect, 8, bg);
    ui.painter().rect_stroke(
        rect,
        8,
        Stroke::new(if hovered { 1.5 } else { 1.0 }, border),
        egui::StrokeKind::Inside,
    );

    // Centered badge image (32x32)
    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(32.0));
    if let Some(texture) = nmessenger::badge_texture(ui.ctx(), &badge.slug, &badge.icon) {
        ui.painter().image(
            texture.id(),
            icon_rect,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            &badge.icon,
            egui::FontId::proportional(18.0),
            Color32::from_rgb(40, 40, 40),
        );
    }

    show_badge_tooltip(&response, badge);
    response
}

pub(super) fn show_badge_tooltip(response: &egui::Response, badge: &Badge) {
    let mut tooltip = egui::Tooltip::for_enabled(response).width(260.0);
    tooltip.popup = tooltip.popup.frame(
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, Color32::from_rgb(175, 205, 235)))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .shadow(egui::epaint::Shadow {
                offset: [0, 2],
                blur: 6,
                spread: 0,
                color: Color32::from_black_alpha(24),
            }),
    );
    tooltip.show(|ui| {
        ui.set_max_width(260.0);

        // Top row: badge image and name.
        ui.horizontal(|ui| {
            let (img_rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), egui::Sense::hover());
            if let Some(tex) = nmessenger::badge_texture(ui.ctx(), &badge.slug, &badge.icon) {
                ui.painter().image(
                    tex.id(),
                    img_rect,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.painter().text(
                    img_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &badge.icon,
                    egui::FontId::proportional(30.0),
                    Color32::BLACK,
                );
            }

            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(&badge.name)
                        .size(13.0)
                        .strong()
                        .color(nmessenger::HEADING),
                );
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // Description
        ui.label(
            RichText::new(&badge.description)
                .size(11.0)
                .color(Color32::from_rgb(50, 70, 90)),
        );

        // Awarded date
        if let Some(dt) = badge.awarded_at {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("Earned on:", "Conquistado em:"))
                        .size(9.5)
                        .color(nmessenger::MUTED),
                );
                ui.label(
                    RichText::new(
                        dt.with_timezone(&chrono::Local)
                            .format("%d/%m/%Y")
                            .to_string(),
                    )
                    .size(9.5)
                    .strong()
                    .color(Color32::from_rgb(40, 110, 180)),
                );
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires X11, xdotool and WINIT_UNIX_BACKEND=x11"]
    fn native_profile_window_can_be_resized_repeatedly() {
        use std::{process::Command, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
        use winit::platform::x11::EventLoopBuilderExtX11;
        struct Probe { app: MessengerApp, done: Arc<AtomicBool> }
        impl eframe::App for Probe {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                show(&mut self.app, ui.ctx());
                if self.done.load(Ordering::Relaxed) { ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close); }
                ui.ctx().request_repaint_after(Duration::from_millis(20));
            }
        }
        let done = Arc::new(AtomicBool::new(false));
        let finished = done.clone();
        let worker = std::thread::spawn(move || -> Result<(), String> {
            let result = (|| {
                let start = Instant::now();
                let id = loop {
                    let output = Command::new("xdotool").args(["search", "--name", "Resize Probe"]).output().map_err(|e| e.to_string())?;
                    if output.status.success() && let Some(id) = String::from_utf8_lossy(&output.stdout).lines().next() { break id.to_owned(); }
                    if start.elapsed() > Duration::from_secs(8) { return Err("profile window not found".into()); }
                    std::thread::sleep(Duration::from_millis(100));
                };
                for (width, height) in [(640, 500), (480, 700), (720, 550)] {
                    let status = Command::new("xdotool").args(["windowsize", &id, &width.to_string(), &height.to_string()]).status().map_err(|e| e.to_string())?;
                    if !status.success() { return Err("window resize failed".into()); }
                    std::thread::sleep(Duration::from_millis(400));
                    let output = Command::new("xdotool").args(["getwindowgeometry", "--shell", &id]).output().map_err(|e| e.to_string())?;
                    let geometry = String::from_utf8_lossy(&output.stdout);
                    if !geometry.lines().any(|line| line == format!("WIDTH={width}")) || !geometry.lines().any(|line| line == format!("HEIGHT={height}")) {
                        return Err(format!("expected {width}x{height}, got {geometry}"));
                    }
                }
                if std::env::var_os("NM_TEST_RESIZE_DRAG").is_some() {
                    let activate = Command::new("xdotool").args(["windowactivate", "--sync", &id]).status().map_err(|e| e.to_string())?;
                    if !activate.success() { return Err("could not activate profile window".into()); }
                    let status = Command::new("xdotool").args(["mousemove", "--window", &id, "718", "548", "mousedown", "1"]).status().map_err(|e| e.to_string())?;
                    if !status.success() { return Err("could not press resize corner".into()); }
                    std::thread::sleep(Duration::from_millis(250));
                    let status = Command::new("xdotool").args(["mousemove_relative", "--sync", "80", "60"]).status().map_err(|e| e.to_string())?;
                    if !status.success() { return Err("could not drag resize corner".into()); }
                    std::thread::sleep(Duration::from_millis(300));
                    let _ = Command::new("xdotool").args(["mouseup", "1"]).status();
                    std::thread::sleep(Duration::from_millis(500));
                    let output = Command::new("xdotool").args(["getwindowgeometry", "--shell", &id]).output().map_err(|e| e.to_string())?;
                    let geometry = String::from_utf8_lossy(&output.stdout);
                    let dimension = |name: &str| geometry.lines().find_map(|line| line.strip_prefix(name)?.parse::<u32>().ok());
                    if dimension("WIDTH=").unwrap_or(0) <= 720 || dimension("HEIGHT=").unwrap_or(0) <= 550 {
                        return Err(format!("drag did not resize profile: {geometry}"));
                    }
                }
                Ok(())
            })();
            finished.store(true, Ordering::Relaxed);
            result
        });
        let mut app = MessengerApp::new_for_test();
        app.contact_forms.view_profile = Some(Profile {
            id: uuid::Uuid::new_v4(), email: "probe@example.test".into(), display_name: "Resize Probe".into(),
            personal_message: String::new(), presence: Presence::Available, avatar_id: None,
            scene: String::new(), now_playing: None, created_at: chrono::Utc::now(), bio: String::new(), badges: vec![],
        });
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_title("Resize Test Root").with_inner_size([300.0, 300.0]),
            event_loop_builder: Some(Box::new(|builder| { builder.with_any_thread(true); })), ..Default::default()
        };
        eframe::run_native("Resize Test Root", options, Box::new(|_| Ok(Box::new(Probe { app, done })))).unwrap();
        worker.join().unwrap().unwrap();
    }

    #[test]
    fn profile_content_fits_when_window_width_changes() {
        let mut app = MessengerApp::new_for_test();
        app.contact_forms.view_profile = Some(Profile {
            id: uuid::Uuid::new_v4(), email: "contact@example.test".into(),
            display_name: "A long display name for a contact".into(),
            personal_message: "A personal message that should wrap when the window is narrow".into(),
            presence: Presence::Available, avatar_id: None, scene: String::new(), now_playing: None,
            created_at: chrono::Utc::now(), bio: String::new(), badges: vec![],
        });
        let ctx = egui::Context::default();
        for width in [460.0, 720.0, 480.0] {
            let mut actual = 0.0;
            let mut output = ctx.run_ui(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 700.0))),
                ..Default::default()
            }, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    content(&mut app, ui);
                    actual = ui.min_rect().width();
                });
            });
            output.textures_delta.clear();
            assert!(actual <= width + 1.0, "profile overflowed after resize: {actual} > {width}");
        }
    }

    #[test]
    fn badge_responds_to_hover_without_a_click() {
        let ctx = egui::Context::default();
        let badge = Badge {
            id: uuid::Uuid::new_v4(),
            slug: String::new(),
            name: "Badge".into(),
            description: "Description".into(),
            icon: "*".into(),
            points: 0,
            awarded_at: None,
        };
        let mut center = Pos2::ZERO;
        let input = |events| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input(vec![]), |ui| {
            center = render_badge_icon_only(ui, &badge).rect.center();
        });
        output.textures_delta.clear();
        let mut hovered = false;
        let mut output = ctx.run_ui(input(vec![egui::Event::PointerMoved(center)]), |ui| {
            hovered = render_badge_icon_only(ui, &badge).hovered();
        });
        output.textures_delta.clear();
        assert!(hovered);
    }
}
