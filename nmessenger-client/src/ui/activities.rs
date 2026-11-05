use crate::{app::MessengerApp, media::audio, transport::Action};
use crate::i18n::tr;
use eframe::egui::{self, RichText, Vec2};
use nm_protocol::*;

pub fn menu_contents(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    let can_call = conversation.members.len() >= 2;
    for (kind, label) in [
        (ActivityKind::AudioCall, "Chamada de voz"),
        (ActivityKind::VideoCall, "Videochamada"),
    ] {
        if ui
            .add_enabled(can_call && audio::AVAILABLE && !app.call_busy(None), egui::Button::new(label))
            .on_hover_text(if app.call_busy(None) {
                tr("End the current call first.", "Encerre a chamada atual primeiro.")
            } else if audio::AVAILABLE {
                "A captura começa quando o convite é aceito."
            } else {
                "Requer compilar com --features native-audio (vídeo também usa ffmpeg)."
            })
            .clicked()
        {
            app.offered_calls.insert(conversation.id, kind);
            app.send(Action::Activity(conversation.id, ActivityRequest { kind, payload: serde_json::json!({"audio":"pcm_s16le","sample_rate":16000,"channels":1,"video":"jpeg"}) }));
            ui.close();
        }
    }
    ui.separator();
    if ui
        .add_enabled(
            can_call && crate::media::remote::available() && !app.call_busy(None),
            egui::Button::new("Compartilhar tela"),
        )
        .on_hover_text("A tela é enviada após o convite ser aceito. Controle remoto está disponível no Linux/X11.")
        .clicked()
    {
        app.offered_calls.insert(conversation.id, ActivityKind::RemoteAssistance);
        app.send(Action::Activity(
            conversation.id,
            ActivityRequest {
                kind: ActivityKind::RemoteAssistance,
                payload: serde_json::json!({"screen":"jpeg","input":"xdotool","control":crate::media::remote::control_available(),"version":1}),
            },
        ));
        ui.close();
    }
}

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    let Some(data) = &app.data else {
        return;
    };
    let me = data.me.id;
    let activities: Vec<_> = data
        .activities
        .iter()
        .filter(|a| {
            a.conversation_id == conversation.id
                && a.kind != ActivityKind::Game
                && matches!(a.status, ActivityStatus::Offered | ActivityStatus::Accepted)
        })
        .cloned()
        .collect();
    if activities.is_empty() {
        return;
    }
    egui::Frame::new()
        .fill(egui::Color32::TRANSPARENT)
        .inner_margin(egui::Margin {
            left: 12,
            right: 14,
            top: 4,
            bottom: 6,
        })
        .show(ui, |ui| {
            super::scroll::vertical()
                .id_salt(("activities", conversation.id))
                .max_height(410.0)
                .show(ui, |ui| {
                    for (index, activity) in activities.into_iter().enumerate() {
                        if index > 0 {
                            ui.add_space(6.0);
                        }
                        if activity.status == ActivityStatus::Accepted
                            && matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall)
                        {
                    if crate::platform::is_mobile()
                                && app.current_call == Some(activity.id) { continue; }
                            super::call_panel::show(app, ui, &activity);
                            continue;
                        }
                        egui::Frame::group(ui.style())
                            .fill(egui::Color32::from_rgb(245, 251, 255))
                            .inner_margin(10)
                            .show(ui, |ui| {
                        let label = match activity.kind {
                            ActivityKind::Game if activity.payload["game"] == "tic_tac_toe" => {
                                "Jogo da velha"
                            }
                            ActivityKind::Game => "Jogo não suportado por este cliente",
                            ActivityKind::AudioCall => "Chamada de voz",
                            ActivityKind::VideoCall => "Videochamada",
                            ActivityKind::RemoteAssistance => {
                                "Assistência remota · tela compartilhada"
                            }
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(label);
                            if activity.status == ActivityStatus::Offered {
                                let offered_by_me = activity.initiator_id == me;
                                ui.label(if offered_by_me {
                                    "Aguardando resposta…"
                                } else {
                                    "Convite recebido"
                                });
                                let supported = (activity.kind == ActivityKind::Game
                                    && activity.payload["game"] == "tic_tac_toe")
                                    || activity.kind == ActivityKind::RemoteAssistance
                                    || (audio::AVAILABLE
                                        && matches!(
                                            activity.kind,
                                            ActivityKind::AudioCall | ActivityKind::VideoCall
                                        ));
                                if !offered_by_me
                                    && ui
                                        .add_enabled(supported && !app.call_busy(Some(activity.id)), egui::Button::new("Aceitar"))
                                        .clicked()
                                {
                                    if activity.kind != ActivityKind::Game {
                                        app.authorized_calls.insert(activity.id);
                                    }
                                    app.send(Action::UpdateActivity(
                                        activity.id,
                                        UpdateActivityRequest {
                                            status: ActivityStatus::Accepted,
                                            payload: None,
                                        },
                                    ));
                                }
                                if ui
                                    .button(if offered_by_me {
                                        "Cancelar convite"
                                    } else {
                                        "Recusar"
                                    })
                                    .clicked()
                                {
                                    app.send(Action::UpdateActivity(
                                        activity.id,
                                        UpdateActivityRequest {
                                            status: if offered_by_me {
                                                ActivityStatus::Ended
                                            } else {
                                                ActivityStatus::Declined
                                            },
                                            payload: None,
                                        },
                                    ));
                                }
                            } else {
                                ui.label("Atividade aceita");
                                if ui.button("Encerrar").clicked() {
                                    if app.current_call == Some(activity.id) {
                                        app.send(Action::StopCall);
                                    }
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
                        if activity.status == ActivityStatus::Accepted {
                            if activity.kind == ActivityKind::Game
                                && activity.payload["game"] == "tic_tac_toe"
                            {
                                game(app, ui, conversation, &activity, me);
                            } else if matches!(
                                activity.kind,
                                ActivityKind::AudioCall
                                    | ActivityKind::VideoCall
                                    | ActivityKind::RemoteAssistance
                            ) {
                                if app.current_call != Some(activity.id) {
                                    if ui
                                        .add_enabled(
                                            audio::AVAILABLE
                                                || activity.kind == ActivityKind::RemoteAssistance,
                                            egui::Button::new("Iniciar mídia nesta sessão"),
                                        )
                                        .clicked()
                                    {
                                        app.authorized_calls.insert(activity.id);
                                        app.current_call = Some(activity.id);
                                        app.send(Action::StartCall {
                                            activity: activity.clone(),
                                            config: app.media_config(),
                                            restart: false,
                                        });
                                    }
                                } else {
                                    if activity.kind == ActivityKind::RemoteAssistance {
                                        super::remote::show(app, ui, &activity, me);
                                        if ui.button("Parar compartilhamento local").clicked() {
                                            app.send(Action::StopCall);
                                        }
                                        return;
                                    }
                                    super::call_panel::show(app, ui, &activity);
                                }
                            }
                        }
                    });
            }
        });
    });
}

pub(super) fn game(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    conversation: &Conversation,
    activity: &Activity,
    me: uuid::Uuid,
) {
    let Some(other) = conversation
        .members
        .iter()
        .find(|m| m.id != activity.initiator_id)
        .map(|m| m.id)
    else {
        return;
    };
    let mut game = app.games.remove(&activity.id).unwrap_or_default();
    let turn = game.turn(activity.initiator_id, other);
    let status = if let Some(winner) = game.winner() {
        format!("{} venceu!", app.name(winner))
    } else if game.board.iter().all(|c| c.is_some()) {
        "Empate!".into()
    } else {
        if turn == me {
            "Sua vez de jogar".into()
        } else {
            format!("Vez de {}", app.name(turn))
        }
    };
    ui.add(
        egui::Label::new(
            RichText::new(status)
                .size(11.0)
                .strong()
                .color(super::nmessenger::HEADING),
        )
        .wrap(),
    );
    ui.label(
        RichText::new(format!(
            "Você joga com {}",
            if me == activity.initiator_id {
                "X"
            } else {
                "O"
            }
        ))
        .size(10.0)
        .color(egui::Color32::from_rgb(115, 137, 155)),
    );
    ui.add_space(8.0);
    let side = ((ui.available_width() - 12.0) / 3.0).clamp(30.0, 54.0);
    egui::Grid::new(("tic-tac-toe",activity.id)).spacing(Vec2::splat(5.0)).show(ui, |ui| {
            for index in 0..9 {
                let enabled = app.connection == "Conectado" && turn == me && game.board[index].is_none() && game.winner().is_none();
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
                ui.painter().rect_filled(rect, 5, if enabled && response.hovered() { egui::Color32::from_rgb(229, 244, 255) } else { egui::Color32::WHITE });
                ui.painter().rect_stroke(rect, 5, egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(204, 224, 239)), egui::StrokeKind::Inside);
                if let Some(owner) = game.board[index] {
                    let center = rect.center(); let radius = side * 0.25;
                    if owner == activity.initiator_id {
                        for direction in [1.0, -1.0] { ui.painter().line_segment([center - Vec2::new(radius, radius * direction), center + Vec2::new(radius, radius * direction)], egui::Stroke::new(3.0_f32, super::nmessenger::BLUE)); }
                    } else { ui.painter().circle_stroke(center, radius, egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(230, 104, 144))); }
                }
                if response.clicked()
                    && game.apply(me, index, activity.initiator_id, other) {
                    app.send(Action::Realtime(ClientEvent::Signal { activity_id: activity.id, data: serde_json::json!({"game":"tic_tac_toe","action":"move","cell":index}) }));
                }
                if index % 3 == 2 { ui.end_row(); }
            }
        });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if turn == me && game.winner().is_none() && game.board.iter().any(|c| c.is_none()) {
            if ui.button(tr("Pass turn", "Passar a vez")).clicked() {
                game.pass_turn();
                app.send(Action::Realtime(ClientEvent::Signal {
                    activity_id: activity.id,
                    data: serde_json::json!({"game":"tic_tac_toe","action":"pass_turn"}),
                }));
            }
        }
        if ui.small_button(tr("Sync match", "Sincronizar partida")).clicked() {
            app.send(Action::Realtime(ClientEvent::Signal {
                activity_id: activity.id,
                data: serde_json::json!({"game":"tic_tac_toe","action":"sync"}),
            }));
        }
    });
    app.games.insert(activity.id, game);
}
