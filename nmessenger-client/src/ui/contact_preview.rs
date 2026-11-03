use super::nmessenger;
use crate::app::MessengerApp;
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Vec2};
use nm_protocol::Contact;
use std::time::{Duration, Instant};

/// Delay keeps the roster readable while the pointer travels across contacts.
pub fn show(app: &mut MessengerApp, ui: &egui::Ui, contact: &Contact, response: &egui::Response) {
    let card_id = egui::Id::unique((
        "contact-profile-preview-rect",
        ui.ctx().viewport_id(),
        contact.user.id,
    ));
    let card_rect = ui.ctx().data(|data| data.get_temp::<Rect>(card_id));
    let over_card = card_rect.is_some_and(|rect| {
        ui.input(|input| {
            input
                .pointer
                .hover_pos()
                .is_some_and(|pos| rect.expand(12.0).contains(pos))
        })
    });
    if (!response.container_hovered() && !over_card) || ui.input(|input| input.pointer.any_down()) {
        app.hover.remove(&contact.user.id);
        ui.ctx().data_mut(|data| data.remove::<Rect>(card_id));
        return;
    }
    let since = *app
        .hover
        .entry(contact.user.id)
        .or_insert_with(Instant::now);
    let delay = Duration::from_millis(550);
    if since.elapsed() < delay {
        ui.ctx()
            .request_repaint_after(delay - since.elapsed().min(delay));
        return;
    }
    let screen = ui.ctx().content_rect();
    let card_width = 185.0_f32.min((screen.width() - 16.0).max(1.0));
    let card_height = card_rect.map_or(110.0, |rect| rect.height());
    let pointer = ui.input(|input| input.pointer.hover_pos()).unwrap_or(response.rect.right_top());
    // Once the pointer leaves the contact row for the card, keep the card still
    // so its badges remain reachable instead of fleeing from the pointer.
    let target_pos = if !response.container_hovered() && over_card {
        card_rect.map_or(pointer, |rect| rect.min)
    } else {
        position_near_pointer(screen, pointer, Vec2::new(card_width, card_height))
    };

    let card = egui::Area::new(egui::Id::unique((
        "contact-profile-preview",
        contact.user.id,
    )))
    .order(egui::Order::Tooltip)
    .fixed_pos(target_pos)
    .constrain_to(screen.shrink(6.0))
    .interactable(true)
    .show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(Color32::from_rgb(248, 252, 255))
            .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(180, 212, 238)))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(7))
            .shadow(egui::epaint::Shadow {
                offset: [0, 3],
                blur: 10,
                spread: 0,
                color: Color32::from_black_alpha(35),
            })
            .show(ui, |ui| {
                ui.set_width(card_width - 14.0);
                ui.horizontal(|ui| {
                    nmessenger::avatar(app, ui, &contact.user, 34.0);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(contact.name())
                                    .strong()
                                    .size(11.5)
                                    .color(nmessenger::HEADING),
                            )
                            .wrap(),
                        );
                        if contact.name() != contact.user.display_name {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&contact.user.display_name).size(9.5),
                                )
                                .wrap(),
                            );
                        }
                        ui.label(
                            RichText::new(nmessenger::status_label(contact.user.presence))
                                .size(9.5)
                                .color(nmessenger::HEADING),
                        );
                    });
                });
                if !contact.user.personal_message.trim().is_empty() {
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(RichText::new(&contact.user.personal_message).size(10.0))
                            .wrap(),
                    );
                }
                if let Some(music) = contact
                    .user
                    .now_playing
                    .as_deref()
                    .filter(|text| !text.trim().is_empty())
                {
                    ui.add_space(3.0);
                    ui.horizontal_wrapped(|ui| {
                        nmessenger::image(ui, "icons/musical-note.png", Vec2::splat(11.0));
                        ui.add(
                            egui::Label::new(
                                RichText::new(music).size(9.5).color(nmessenger::BLUE),
                            )
                            .wrap(),
                        );
                    });
                }
                if !contact.user.email.is_empty() {
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&contact.user.email)
                                .size(9.5)
                                .color(nmessenger::HEADING),
                        )
                        .wrap(),
                    );
                }
                if !contact.user.badges.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for b in &contact.user.badges {
                            let (badge_rect, b_res) =
                                ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                            if let Some(texture) =
                                nmessenger::badge_texture(ui.ctx(), &b.slug, &b.icon)
                            {
                                ui.painter().image(
                                    texture.id(),
                                    badge_rect,
                                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            } else {
                                ui.painter().text(
                                    badge_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &b.icon,
                                    egui::FontId::proportional(11.0),
                                    Color32::BLACK,
                                );
                            }
                            super::profile_view::show_badge_tooltip(&b_res, b);
                        }
                    });
                }
                if contact.favorite {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        nmessenger::image(ui, "emojis/star.png", Vec2::splat(11.0));
                        ui.label(
                            RichText::new("Contato favorito")
                                .size(9.0)
                                .color(nmessenger::HEADING),
                        );
                    });
                }
            });
    })
    .response
    .rect;
    ui.ctx().data_mut(|data| data.insert_temp(card_id, card));
}

fn position_near_pointer(screen: Rect, pointer: Pos2, size: Vec2) -> Pos2 {
    let margin = 6.0;
    let mut x = pointer.x + 14.0;
    if x + size.x > screen.right() - margin { x = pointer.x - size.x - 14.0; }
    let x = x.clamp(screen.left() + margin, (screen.right() - size.x - margin).max(screen.left() + margin));
    let y = (pointer.y + 12.0).clamp(screen.top() + margin, (screen.bottom() - size.y - margin).max(screen.top() + margin));
    egui::pos2(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_tracks_pointer_and_stays_inside_screen() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
        let size = Vec2::new(185.0, 110.0);
        assert_eq!(position_near_pointer(screen, egui::pos2(50.0, 40.0), size), egui::pos2(64.0, 52.0));
        let near_edge = position_near_pointer(screen, egui::pos2(395.0, 295.0), size);
        assert!(near_edge.x + size.x <= screen.right() && near_edge.y + size.y <= screen.bottom());
        assert!(near_edge.x < 395.0);
    }
}
