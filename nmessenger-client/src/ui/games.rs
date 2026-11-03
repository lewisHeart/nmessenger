use super::{activities, dialog, nmessenger};
use crate::{app::MessengerApp, transport::Action};
use eframe::egui::{self, Color32, RichText, Vec2};
use nm_protocol::*;

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    let Some(id) = app.options.game_browser else {
        return;
    };
    let Some(conversation) = app
        .data
        .as_ref()
        .and_then(|data| data.conversations.iter().find(|c| c.id == id))
        .cloned()
    else {
        app.options.game_browser = None;
        return;
    };
    let open = dialog::show(
        ctx,
        "nmessenger-games",
        "Jogos - NMessenger",
        [460.0, 430.0],
        |ui| {
            ui.heading("Jogos");
            ui.label(
                RichText::new("Convide seu contato e joguem juntos.").color(nmessenger::MUTED),
            );
            ui.add_space(14.0);
            if app.data.as_ref().is_some_and(|data| {
                data.activities.iter().any(|a| {
                    a.conversation_id == id
                        && a.kind == ActivityKind::Game
                        && matches!(a.status, ActivityStatus::Offered | ActivityStatus::Accepted)
                })
            }) {
                panel(app, ui, &conversation);
                return false;
            }
            egui::Frame::new().fill(Color32::from_rgb(243, 249, 255)).corner_radius(8).inner_margin(14)
            .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(205, 225, 243))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(90.0), egui::Sense::hover());
                for index in 1..3 {
                    let offset = index as f32 * 30.0;
                    let stroke = egui::Stroke::new(2.0_f32, Color32::from_rgb(150, 187, 220));
                    ui.painter().line_segment([rect.min + Vec2::new(offset, 0.0), rect.min + Vec2::new(offset, 90.0)], stroke);
                    ui.painter().line_segment([rect.min + Vec2::new(0.0, offset), rect.min + Vec2::new(90.0, offset)], stroke);
                }
                ui.painter().text(rect.min + Vec2::splat(15.0), egui::Align2::CENTER_CENTER, "X", egui::FontId::proportional(23.0), nmessenger::BLUE);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "O", egui::FontId::proportional(23.0), Color32::from_rgb(226, 84, 123));
                ui.vertical(|ui| {
                    ui.label(RichText::new("Jogo da velha").size(16.0).strong());
                    ui.label("Dois jogadores · partida em tempo real");
                    ui.add_space(8.0);
                    if ui.add_enabled(conversation.members.len() == 2, egui::Button::new("Convidar para jogar")).clicked() {
                        app.send(Action::Activity(id, ActivityRequest { kind: ActivityKind::Game, payload: serde_json::json!({"game":"tic_tac_toe","version":1}) }));
                    }
                });
            });
        });
            ui.add_space(10.0);
            panel(app, ui, &conversation);
            false
        },
    );
    if !open {
        app.options.game_browser = None;
    }
}

pub fn panel(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    let Some(data) = &app.data else {
        return;
    };
    let me = data.me.id;
    let activities: Vec<_> = data
        .activities
        .iter()
        .filter(|activity| {
            activity.conversation_id == conversation.id
                && activity.kind == ActivityKind::Game
                && matches!(
                    activity.status,
                    ActivityStatus::Offered | ActivityStatus::Accepted
                )
        })
        .cloned()
        .collect();
    for activity in activities {
        super::conversation_sidebar::card(ui, |ui| {
            ui.horizontal(|ui| {
                nmessenger::image(ui, "icons/game.png", Vec2::splat(22.0));
                ui.label(
                    RichText::new("Jogo da velha")
                        .size(12.0)
                        .strong()
                        .color(nmessenger::HEADING),
                );
            });
            ui.add_space(12.0);
            if activity.status == ActivityStatus::Offered {
                ui.label(
                    RichText::new(if activity.initiator_id == me {
                        "Convite enviado"
                    } else {
                        "Vamos jogar?"
                    })
                    .size(12.0)
                    .strong()
                    .color(nmessenger::HEADING),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(if activity.initiator_id == me {
                            "A partida começa assim que seu contato aceitar."
                        } else {
                            "Você recebeu um convite para uma partida."
                        })
                        .size(11.0)
                        .color(Color32::from_rgb(107, 130, 150)),
                    )
                    .wrap(),
                );
                ui.add_space(14.0);
                if activity.initiator_id != me
                    && super::theme::primary_button(
                        ui,
                        egui::RichText::new("Aceitar e jogar")
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                        Vec2::new(ui.available_width(), 30.0),
                    )
                    .clicked()
                {
                    app.send(Action::UpdateActivity(
                        activity.id,
                        UpdateActivityRequest {
                            status: ActivityStatus::Accepted,
                            payload: None,
                        },
                    ));
                }
                if ui
                    .add_sized(
                        [ui.available_width(), 28.0],
                        egui::Button::new(if activity.initiator_id == me {
                            "Cancelar convite"
                        } else {
                            "Recusar"
                        })
                        .frame(false),
                    )
                    .clicked()
                {
                    app.send(Action::UpdateActivity(
                        activity.id,
                        UpdateActivityRequest {
                            status: if activity.initiator_id == me {
                                ActivityStatus::Ended
                            } else {
                                ActivityStatus::Declined
                            },
                            payload: None,
                        },
                    ));
                }
            } else {
                activities::game(app, ui, conversation, &activity, me);
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 27.0],
                        egui::Button::new(
                            RichText::new("Encerrar partida")
                                .size(10.5)
                                .color(Color32::from_rgb(173, 72, 72)),
                        )
                        .frame(false),
                    )
                    .clicked()
                {
                    app.send(Action::UpdateActivity(
                        activity.id,
                        UpdateActivityRequest {
                            status: ActivityStatus::Ended,
                            payload: None,
                        },
                    ));
                }
            }
        });
    }
}
