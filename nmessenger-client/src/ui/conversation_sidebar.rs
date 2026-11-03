use super::{nmessenger, theme};
use crate::{app::MessengerApp, transport::Action};
use eframe::egui::{self, Color32, RichText, Vec2};
use nm_protocol::*;

pub fn card(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_white_alpha(224))
        .corner_radius(9)
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(211, 228, 241)))
        .outer_margin(egui::Margin {
            left: 14,
            right: 8,
            top: 14,
            bottom: 8,
        })
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            super::glass::gloss_line(ui);
            body(ui);
        });
}

pub fn members(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            nmessenger::image(ui, "icons/person.png", Vec2::splat(22.0));
            ui.label(
                RichText::new("Participantes")
                    .size(13.0)
                    .strong()
                    .color(nmessenger::HEADING),
            );
        });
        ui.label(
            RichText::new(format!(
                "{} pessoas neste grupo",
                conversation.members.len()
            ))
            .size(10.0)
            .color(nmessenger::HEADING),
        );
        ui.add_space(12.0);
        super::scroll::vertical()
            .id_salt(("group-participants", conversation.id))
            .max_height((ui.available_height() - 140.0).clamp(100.0, 360.0))
            .show(ui, |ui| {
                for member in &conversation.members {
                    ui.horizontal(|ui| {
                        nmessenger::avatar(app, ui, member, 32.0);
                        ui.add_space(3.0);
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&member.display_name)
                                        .size(11.0)
                                        .color(nmessenger::HEADING),
                                )
                                .truncate(),
                            );
                            let own = app
                                .data
                                .as_ref()
                                .is_some_and(|data| data.me.id == member.id);
                            ui.label(
                                RichText::new(if own {
                                    crate::i18n::tr("You", "Você")
                                } else {
                                    nmessenger::status_label(member.presence)
                                })
                                .size(10.0)
                                .color(Color32::from_rgb(118, 139, 157)),
                            );
                        });
                    });
                    ui.add_space(8.0);
                }
            });
        ui.add_space(12.0);
        if theme::primary_button(
            ui,
            RichText::new("Gerenciar participantes")
                .size(10.5)
                .color(Color32::from_rgb(1, 45, 147))
                .strong(),
            Vec2::new(ui.available_width(), 30.0),
        )
        .clicked()
        {
            app.conversation_forms.conversation = Some(conversation.id);
            app.conversation_forms.viewport = Some(ui.ctx().viewport_id());
            app.conversation_forms.title = conversation.title.clone().unwrap_or_default();
            app.conversation_forms.members_open = true;
        }
        ui.add_space(5.0);
        if ui
            .add_sized(
                [ui.available_width(), 27.0],
                egui::Button::new(
                    RichText::new("Sair do grupo")
                        .size(10.5)
                        .color(Color32::from_rgb(173, 72, 72)),
                )
                .frame(false),
            )
            .clicked()
            && let Some(me) = app.data.as_ref().map(|data| data.me.id)
        {
            app.send(Action::RemoveMember(conversation.id, me));
        }
    });
}
