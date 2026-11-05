use super::nmessenger;
use crate::i18n::tr;
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
            ui.menu_button("⚙", |ui| {
                let is_owner = app.data.as_ref().is_some_and(|data| Some(data.me.id) == conversation.owner_id);
                if is_owner {
                    let mut can_invite = conversation.members_can_invite;
                    if ui.checkbox(&mut can_invite, "Membros podem convidar").changed() {
                        app.send(Action::SetGroupInvitePermission(conversation.id, can_invite));
                    }
                    ui.separator();
                }
                if ui.button(tr("Invite / Manage participants", "Convidar / Gerenciar participantes")).clicked() {
                    app.conversation_forms.conversation = Some(conversation.id);
                    app.conversation_forms.viewport = Some(ui.ctx().viewport_id());
                    app.conversation_forms.title = conversation.title.clone().unwrap_or_default();
                    app.conversation_forms.members_open = true;
                    ui.close();
                }
                if ui.button("Sair do grupo").clicked() {
                    if let Some(me) = app.data.as_ref().map(|data| data.me.id) {
                        app.send(Action::RemoveMember(conversation.id, me));
                    }
                    ui.close();
                }
            });
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
                    let compact = conversation.members.len() > 8;
                    let own = app.data.as_ref().is_some_and(|data| data.me.id == member.id);
                    let is_owner = app.data.as_ref().is_some_and(|data| Some(data.me.id) == conversation.owner_id);
                    let row_h = if compact { 26.0 } else { 38.0 };
                    let (row_rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), row_h),
                        egui::Sense::click(),
                    );
                    let is_held = response.is_pointer_button_down_on();
                    let is_hovered = response.hovered();
                    if is_held {
                        ui.painter().rect_filled(
                            row_rect,
                            4,
                            Color32::from_rgba_unmultiplied(190, 222, 250, 200),
                        );
                        ui.painter().rect_stroke(
                            row_rect,
                            4,
                            egui::Stroke::new(1.0, Color32::from_rgb(120, 180, 240)),
                            egui::StrokeKind::Inside,
                        );
                    } else if is_hovered {
                        ui.painter().rect_filled(
                            row_rect,
                            4,
                            Color32::from_rgba_unmultiplied(225, 240, 255, 180),
                        );
                    }
                    if response.double_clicked() {
                        app.contact_forms.view_profile = Some(member.clone());
                    }
                    response.context_menu(|ui| {
                        if ui.button("Ver perfil").clicked() {
                            app.contact_forms.view_profile = Some(member.clone());
                            ui.close();
                        }
                        if !own && ui.button("Conversar no privado").clicked() {
                            app.send(Action::CreateConversation(CreateConversationRequest {
                                member_ids: vec![member.id],
                                title: None,
                                members_can_invite: true,
                            }));
                            ui.close();
                        }
                        if is_owner && !own {
                            ui.separator();
                            if ui.button("Tornar administrador").clicked() {
                                app.send(Action::TransferGroupOwner(conversation.id, member.id));
                                ui.close();
                            }
                            if ui.button("Expulsar do grupo").clicked() {
                                app.send(Action::RemoveMember(conversation.id, member.id));
                                ui.close();
                            }
                        }
                    });
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(row_rect.shrink2(Vec2::new(3.0, 1.0))),
                        |ui| {
                            ui.horizontal(|ui| {
                                nmessenger::avatar(app, ui, member, if compact { 22.0 } else { 32.0 });
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
                                    if !compact {
                                        ui.label(
                                            RichText::new(if own {
                                                crate::i18n::tr("You", "Você")
                                            } else if Some(member.id) == conversation.owner_id {
                                                "Administrador"
                                            } else {
                                                nmessenger::status_label(member.presence)
                                            })
                                            .size(10.0)
                                            .color(Color32::from_rgb(118, 139, 157)),
                                        );
                                    }
                                });
                            });
                        },
                    );
                    ui.add_space(if compact { 2.0 } else { 4.0 });
                }
            });
    });
}
