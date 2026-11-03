use super::theme;
use crate::i18n::tr;
use crate::{
    app::MessengerApp,
    media::{Ink, images::ImageSource},
    transport::Action,
};
use eframe::egui::{self, Color32, RichText, Vec2};
use nm_protocol::*;

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, message: &Message, grouped: bool) {
    let me = app
        .data
        .as_ref()
        .is_some_and(|d| d.me.id == message.sender_id);
    let row = egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 2,
            right: 2,
            top: 3,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_width((ui.available_width() - 4.0).max(60.0));
            if !grouped {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}:", app.name(message.sender_id), tr("says", "diz")))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                    if app
                        .data
                        .as_ref()
                        .is_some_and(|d| d.settings.show_timestamps)
                    {
                        ui.label(
                            RichText::new(
                                message
                                    .created_at
                                    .with_timezone(&chrono::Local)
                                    .format("%d/%m %H:%M")
                                    .to_string(),
                            )
                            .small()
                            .color(theme::MUTED),
                        );
                    }
                });
            }
            match message.kind {
                MessageKind::Text => {
                    if app.editing_message.as_ref().is_some_and(|item| item.id == message.id) {
                        let edit_id = egui::Id::unique(("inline-message-edit", message.id));
                        ui.add_enabled(!app.edit_message_busy, egui::TextEdit::multiline(&mut app.edit_message_draft)
                            .id(edit_id).desired_rows(2).desired_width(ui.available_width().min(560.0))
                            .char_limit(MAX_MESSAGE_CHARS));
                        if let Some(error) = &app.edit_message_error {
                            ui.colored_label(Color32::from_rgb(185, 28, 28), error);
                        }
                        ui.horizontal_wrapped(|ui| {
                            let can_save = !app.edit_message_busy && !app.edit_message_draft.trim().is_empty();
                            if ui.add_enabled(can_save, egui::Button::new(tr("Save", "Salvar"))).clicked() {
                                app.edit_message_busy = true;
                                app.edit_message_error = None;
                                app.send(Action::EditMessage {
                                    conversation: message.conversation_id, message: message.id,
                                    request: SendMessageRequest { client_id: message.client_id, kind: MessageKind::Text,
                                        body: app.edit_message_draft.clone(), style: message.style.clone(), attachment_id: None },
                                });
                            }
                            if ui.add_enabled(!app.edit_message_busy, egui::Button::new(tr("Cancel", "Cancelar"))).clicked() {
                                app.editing_message = None;
                                app.edit_message_draft.clear();
                                app.edit_message_error = None;
                            }
                            if app.edit_message_busy { ui.spinner(); }
                        });
                    } else {
                        let enabled = app.data.as_ref().is_some_and(|d| d.settings.emoticons);
                        super::emoticons::show(app, ui, &message.body, &message.style, enabled);
                        if let Some(id) = message.attachment_id {
                            image(app, ui, id, Vec2::splat(70.0));
                        }
                    }
                }
                MessageKind::Nudge => {
                    ui.label(
                        RichText::new(tr("Sent a nudge!", "Chamou a atenção!"))
                            .strong()
                            .color(theme::BLUE),
                    );
                }
                MessageKind::Wink => {
                    ui.horizontal(|ui| {
                        let (name, graphic) = super::winks::details(&message.body);
                        super::nmessenger::image(ui, graphic, Vec2::splat(26.0));
                        ui.label(RichText::new(format!("{}: {name}", tr("Wink", "Piscadela"))).color(theme::BLUE));
                        if ui
                            .small_button(tr("Replay", "Repetir"))
                            .on_hover_text(tr("Replay the wink", "Repetir a piscadela já reproduzida automaticamente"))
                            .clicked()
                        {
                            app.winks.insert(
                                message.conversation_id,
                                (message.body.clone(), std::time::Instant::now()),
                            );
                        }
                    });
                }
                MessageKind::Ink => match serde_json::from_str::<Ink>(&message.body) {
                    Ok(ink)
                        if ink.width.is_finite()
                            && ink.height.is_finite()
                            && ink.width > 0.0
                            && ink.height > 0.0 =>
                    {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width().min(600.0), 160.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 5, Color32::WHITE);
                        paint_ink(ui, rect, &ink);
                    }
                    _ => {
                        ui.colored_label(theme::MUTED, tr("Unrecognized drawing format.", "Manuscrito em formato não reconhecido."));
                    }
                },
                MessageKind::Photo => {
                    ui.label(&message.body);
                    if let Some(id) = message.attachment_id {
                        if image(app, ui, id, Vec2::new(300.0, 210.0)).is_some_and(|r| r.clicked())
                        {
                            app.image_viewer = Some(id);
                        }
                        attachment(app, ui, id);
                    }
                }
                MessageKind::Voice => {
                    if let Some(file) = message.attachment_id {
                        super::audio_player::show(app, ui, file);
                    }
                }
                MessageKind::File => {
                    ui.label(format!(
                        "{} {}",
                        if message.kind == MessageKind::Voice {
                            "♫"
                        } else {
                            "▤"
                        },
                        message.body
                    ));
                    if let Some(id) = message.attachment_id {
                        attachment(app, ui, id);
                    }
                }
                _ => {
                    ui.label(RichText::new(&message.body).italics().color(theme::MUTED));
                }
            }
            if me {
                let readers: Vec<_> = app
                    .reads
                    .iter()
                    .filter(|((c, u), m)| {
                        *c == message.conversation_id
                            && *u != message.sender_id
                            && **m == Some(message.id)
                    })
                    .map(|((_, u), _)| app.name(*u))
                    .collect();
                if !readers.is_empty() {
                    ui.label(
                        RichText::new(if readers.is_empty() {
                            String::new()
                        } else {
                            format!("{} {}", tr("Read by", "Lida por"), readers.join(", "))
                        })
                        .size(10.0)
                        .color(theme::MUTED),
                    );
                }
            }
        }).response;
    if message.kind == MessageKind::Text && message.attachment_id.is_none()
        && !app.editing_message.as_ref().is_some_and(|item| item.id == message.id)
        && !message.body.starts_with("[Mensagem protegida indisponível:") {
        let response = ui.interact(row.rect, ui.scope_id().with(("message-actions", message.id)), egui::Sense::click());
        response.context_menu(|ui| {
            if ui.button(tr("Copy", "Copiar")).clicked() {
                crate::platform::set_clipboard_text(&message.body);
                ui.ctx().copy_text(message.body.clone());
                ui.close();
            }
            if me {
                let editable = (chrono::Utc::now() - message.created_at).num_seconds() <= MESSAGE_EDIT_WINDOW_SECS;
                if ui.add_enabled(editable, egui::Button::new(tr("Edit", "Editar")))
                    .on_disabled_hover_text(tr("Messages can be edited for 15 minutes.", "Mensagens podem ser editadas por 15 minutos."))
                    .clicked() {
                    app.edit_message_draft = message.body.clone();
                    app.editing_message = Some(message.clone());
                    app.edit_message_busy = false;
                    app.edit_message_error = None;
                    ui.memory_mut(|memory| memory.request_focus(egui::Id::unique(("inline-message-edit", message.id))));
                    ui.close();
                }
                if ui.button(tr("Delete", "Excluir")).clicked() {
                    app.send(Action::DeleteMessage { conversation: message.conversation_id, message: message.id });
                    ui.close();
                }
            }
        });
    }
}

pub fn image(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    id: uuid::Uuid,
    maximum: Vec2,
) -> Option<egui::Response> {
    if let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::File(id)) {
        let scale = (maximum.x / size.x).min(maximum.y / size.y).min(1.0);
        Some(
            ui.add(
                egui::Image::new((texture, size * scale))
                    .corner_radius(6)
                    .sense(egui::Sense::click()),
            ),
        )
    } else {
        ui.label(
            RichText::new(tr("Image: loading or unavailable", "Imagem: carregando ou indisponível"))
                .small()
                .color(theme::MUTED),
        );
        None
    }
}

fn attachment(app: &mut MessengerApp, ui: &mut egui::Ui, id: uuid::Uuid) {
    ui.horizontal_wrapped(|ui| {
        if let Some(info) = app.file_info.get(&id) {
            ui.label(
                RichText::new(format!(
                    "{} · {:.1} KiB · {}",
                    info.filename,
                    info.size as f64 / 1024.0,
                    info.mime
                ))
                .small()
                .color(theme::MUTED),
            );
        } else if ui.small_button(tr("Details", "Detalhes")).clicked() {
            app.send(Action::FileInfo(id));
        }
        if ui.small_button(tr("Save as…", "Salvar como…")).clicked() {
            app.send(Action::Download(id));
        }
    });
}

pub fn paint_ink(ui: &egui::Ui, rect: egui::Rect, ink: &Ink) {
    let painter = ui.painter().with_clip_rect(rect);
    for stroke in &ink.strokes {
        let points: Vec<_> = stroke
            .points
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .map(|p| {
                rect.min
                    + Vec2::new(
                        p[0] / ink.width * rect.width(),
                        p[1] / ink.height * rect.height(),
                    )
            })
            .collect();
        if points.len() >= 2 {
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(
                    stroke.width.clamp(0.5, 30.0),
                    Color32::from_rgb(stroke.color[0], stroke.color[1], stroke.color[2]),
                ),
            ));
        }
    }
}

pub fn effects(app: &MessengerApp, ui: &egui::Ui, id: uuid::Uuid, rect: egui::Rect) {
    if let Some((kind, time)) = app.winks.get(&id) {
        super::winks::effect(ui, rect, kind, time.elapsed().as_secs_f32());
    }
}
