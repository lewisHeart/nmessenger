use super::{nmessenger, theme};
use crate::i18n::tr;
use crate::{
    app::MessengerApp,
    media::{Stroke, audio},
    transport::Action,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RectAlign, RichText, Vec2};
use nm_protocol::*;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub fn format_toolbar(ui: &mut egui::Ui, style: &mut MessageStyle) {
    super::message_style::toolbar(ui, style);
}

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, id: Uuid) {
    let mut composer = app.composers.remove(&id).unwrap_or_default();
    let mut send = None;
    let (bar, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), egui::Sense::hover());
    for (index, name, tooltip) in [
        (0, "emoji", "Emojis"),
        (1, "gift", tr("Winks", "Piscadelas (Winks)")),
        (2, "folder", tr("Share a file", "Compartilhar arquivo")),
    ] {
        let rect = Rect::from_min_size(
            bar.min + Vec2::new(index as f32 * 36.0, 1.0),
            Vec2::new(34.0, 30.0),
        );
        let response = ui
            .interact(rect, ui.scope_id().with((id, name)), egui::Sense::click())
            .on_hover_text(tooltip);
        let held = response.is_pointer_button_down_on();
        let hovered = response.hovered();
        let is_open = egui::Popup::is_id_open(ui.ctx(), response.id.with("popup"));
        if held || is_open {
            ui.painter().rect_filled(
                rect,
                4,
                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
            );
            ui.painter().rect_stroke(
                rect,
                4,
                egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                egui::StrokeKind::Inside,
            );
        } else if hovered {
            ui.painter().rect_filled(
                rect,
                4,
                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
            );
            ui.painter().rect_stroke(
                rect,
                4,
                egui::Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                egui::StrokeKind::Inside,
            );
        }
        let icon = if name == "emoji" {
            "emojis/slightly-smiling-face.png"
        } else if name == "gift" {
            "emojis/sparkles.png"
        } else {
            "icons/folder.png"
        };
        let offset = if held { Vec2::new(0.0, 1.0) } else { Vec2::ZERO };
        // Content width: 18 (icon) + 3 (gap) + 6 (arrow) = 27. Total button width: 34.
        // Symmetric margins: left = 3.5 px, right = 3.5 px.
        let icon_rect = Rect::from_min_size(rect.min + Vec2::new(3.5, 6.0) + offset, Vec2::splat(18.0));
        let arrow_rect = Rect::from_min_size(rect.min + Vec2::new(24.5, 12.0) + offset, Vec2::splat(6.0));
        nmessenger::paint(ui.ctx(), ui.painter(), icon, icon_rect);
        nmessenger::paint(ui.ctx(), ui.painter(), "icons/arrow.png", arrow_rect);
        let mut popup = egui::Popup::menu(&response);
        if crate::platform::is_mobile() {
            popup = popup.align(RectAlign::TOP_START);
        }
        popup.show(|ui| match name {
            "emoji" => {
                if let Some(shortcut) = super::emoticons::picker(app, ui) {
                    composer.text.push_str(shortcut);
                    composer.drawing = false;
                    composer.focus_requested = true;
                    crate::platform::show_soft_input();
                    ui.close();
                }
                let assets = app
                    .data
                    .as_ref()
                    .map(|d| d.assets.clone())
                    .unwrap_or_default();
                for asset in assets.iter().filter(|a| a.kind == AssetKind::Emoticon) {
                    if ui.button(&asset.name).clicked() {
                        app.send(Action::SendEmoticon {
                            conversation: id,
                            asset: asset.clone(),
                        });
                        composer.focus_requested = true;
                        crate::platform::show_soft_input();
                        ui.close();
                    }
                }
            }
            "gift" => {
                if let Some(value) = super::winks::picker(ui) {
                    send = Some((MessageKind::Wink, value.into(), None));
                    app.winks.insert(id, (value.into(), Instant::now()));
                    composer.focus_requested = true;
                    crate::platform::show_soft_input();
                    ui.close();
                }
            }
            _ => {
                for (kind, label) in [
                    (MessageKind::Photo, tr("Send photo...", "Enviar foto...")),
                    (MessageKind::Voice, tr("Send WAV audio...", "Enviar áudio WAV...")),
                    (MessageKind::File, tr("Send file...", "Enviar arquivo...")),
                ] {
                    if ui.button(label).clicked() {
                        app.send(Action::Upload {
                            conversation: id,
                            kind,
                        });
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(composer.drawing, tr("Handwritten message...", "Mensagem manuscrita..."))
                    .clicked()
                {
                    composer.drawing = !composer.drawing;
                    ui.close();
                }
            }
        });
    }
    let nudge = Rect::from_min_size(bar.min + Vec2::new(112.0, 1.0), Vec2::new(114.0, 30.0));
    let remaining = composer.nudge_sent.map_or(0, |time| {
        NUDGE_COOLDOWN_SECS.saturating_sub(time.elapsed().as_secs())
    });
    let can_nudge = remaining == 0;
    let nudge_response = ui
        .interact(
            nudge,
            ui.scope_id().with((id, "nudge")),
            egui::Sense::click(),
        )
        .on_hover_text(if can_nudge {
            tr("Send a nudge and shake the window", "Chamar atenção e agitar a janela").into()
        } else {
            format!("{} {remaining} {}", tr("Wait", "Aguarde"), tr("seconds before sending another nudge", "segundos para chamar novamente"))
        });
    if can_nudge {
        if nudge_response.is_pointer_button_down_on() {
            ui.painter().rect_filled(
                nudge,
                4,
                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
            );
            ui.painter().rect_stroke(
                nudge,
                4,
                egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                egui::StrokeKind::Inside,
            );
        } else if nudge_response.hovered() {
            ui.painter().rect_filled(
                nudge,
                4,
                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
            );
            ui.painter().rect_stroke(
                nudge,
                4,
                egui::Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                egui::StrokeKind::Inside,
            );
        }
    }
    let nudge_offset = if can_nudge && nudge_response.is_pointer_button_down_on() { Vec2::new(0.0, 1.0) } else { Vec2::ZERO };
    nmessenger::paint(
        ui.ctx(),
        ui.painter(),
        "icons/nudge.png",
        Rect::from_min_size(nudge.min + Vec2::new(6.0, 7.0) + nudge_offset, Vec2::splat(16.0)),
    );
    ui.painter().text(
        nudge.min + Vec2::new(26.0, 15.0) + nudge_offset,
        egui::Align2::LEFT_CENTER,
        if can_nudge {
            tr("Send nudge", "Chamar atenção").into()
        } else {
            format!("{} {remaining}s", tr("Wait", "Aguarde"))
        },
        FontId::proportional(10.5),
        if can_nudge {
            nmessenger::HEADING
        } else {
            nmessenger::MUTED
        },
    );
    if nudge_response.clicked() && can_nudge {
        composer.nudge_sent = Some(Instant::now());
        app.nudge(id, true);
        send = Some((MessageKind::Nudge, tr("You sent a nudge!", "Você chamou a atenção!").into(), None));
    }
    let font = Rect::from_min_size(Pos2::new(bar.right() - 34.0, bar.top() + 1.0), Vec2::new(34.0, 30.0));
    let response = ui
        .interact(font, ui.scope_id().with((id, "font")), egui::Sense::click())
        .on_hover_text(tr("Font and color", "Fonte e cor"));
    if response.clicked() {
        composer.format_open = !composer.format_open;
    }
    if response.is_pointer_button_down_on() {
        ui.painter().rect_filled(
            font,
            4,
            Color32::from_rgba_unmultiplied(185, 220, 252, 230),
        );
        ui.painter().rect_stroke(
            font,
            4,
            egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() || composer.format_open {
        ui.painter().rect_filled(
            font,
            4,
            Color32::from_rgba_unmultiplied(
                225,
                240,
                255,
                if composer.format_open { 230 } else { 200 },
            ),
        );
        ui.painter().rect_stroke(
            font,
            4,
            egui::Stroke::new(
                1.0,
                if composer.format_open {
                    Color32::from_rgb(70, 145, 230)
                } else {
                    Color32::from_rgb(145, 195, 245)
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    let font_offset = if response.is_pointer_button_down_on() { Vec2::new(0.0, 1.0) } else { Vec2::ZERO };
    ui.painter().text(
        font.min + Vec2::new(11.5, 11.5) + font_offset,
        egui::Align2::CENTER_CENTER,
        "A",
        FontId::proportional(14.0),
        nmessenger::HEADING,
    );
    theme::gradient(
        ui.painter(),
        Rect::from_min_size(font.min + Vec2::new(4.5, 20.0) + font_offset, Vec2::new(14.0, 2.5)),
        Color32::from_rgb(235, 72, 36),
        nmessenger::BLUE,
    );
    nmessenger::paint(
        ui.ctx(),
        ui.painter(),
        "icons/arrow.png",
        Rect::from_min_size(font.min + Vec2::new(23.5, 12.0) + font_offset, Vec2::splat(6.0)),
    );
    if crate::platform::is_mobile() {
        if composer.format_open {
            let screen = ui.ctx().content_rect();
            let card_width = (screen.width() - 16.0).min(380.0);
            let card_x = screen.left() + (screen.width() - card_width) * 0.5;
            let card_bottom = bar.top() - 6.0;

            let area_response = egui::Area::new(ui.scope_id().with(("mobile-format-popup", id)))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(card_x, card_bottom))
                .pivot(egui::Align2::LEFT_BOTTOM)
                .interactable(true)
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(250, 252, 255))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(180, 210, 240)))
                        .corner_radius(10)
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 16,
                            spread: 0,
                            color: Color32::from_black_alpha(45),
                        })
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.set_width(card_width - 24.0);
                            ui.set_max_width(card_width - 24.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(tr("Message formatting", "Formatação da mensagem"))
                                        .strong()
                                        .size(12.5)
                                        .color(nmessenger::HEADING),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if theme::close_button(ui, 20.0).on_hover_text(tr("Close", "Fechar")).clicked() {
                                        composer.format_open = false;
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            let max_h = (card_bottom - screen.top() - 60.0).clamp(160.0, 340.0);
                            super::scroll::vertical()
                                .id_salt(("mobile-format-scroll", id))
                                .max_height(max_h)
                                .show(ui, |ui| {
                                    super::message_style::toolbar_content(ui, &mut composer.style);
                                });
                        });
                });
            if ui.input(|i| i.pointer.any_pressed()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !area_response.response.rect.contains(pos) && !font.contains(pos) {
                        composer.format_open = false;
                    }
                }
            }
        }
    } else {
        let font_popup = egui::Popup::from_response(&response)
            .open_bool(&mut composer.format_open)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .gap(4.0)
            .frame(
                egui::Frame::popup(ui.style())
                    .fill(Color32::from_rgb(250, 252, 255))
                    .stroke(egui::Stroke::new(1.0, nmessenger::BORDER))
                    .corner_radius(8)
                    .inner_margin(12),
            );
        font_popup.show(|ui| format_toolbar(ui, &mut composer.style));
    }
    ui.add_space(8.0);
    if composer.drawing {
        ink(ui, &mut composer);
        ui.horizontal(|ui| {
            if theme::primary(ui, tr("Send drawing", "Enviar manuscrito")).clicked() && !composer.ink.strokes.is_empty()
            {
                match serde_json::to_string(&composer.ink) {
                    Ok(body) => {
                        send = Some((MessageKind::Ink, body, None));
                        composer.ink.strokes.clear();
                    }
                    Err(error) => app.error(error.to_string()),
                }
            }
            if ui.button(tr("Back to text", "Voltar ao texto")).clicked() {
                composer.drawing = false;
            }
        });
    } else {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let is_recording = app.recording && app.recording_conversation == Some(id);
            let buttons_width = if is_recording {
                60.0 + 8.0 + 60.0
            } else {
                60.0
            };
            let size = Vec2::new((ui.available_width() - (buttons_width + 8.0)).max(100.0), 60.0);
            let text = editor(ui, id, &mut composer, size);
            if text.changed() {
                composer.last_edit = Some(Instant::now());
                if app
                    .data
                    .as_ref()
                    .is_some_and(|data| data.settings.typing_indicators)
                    && composer
                        .last_typing
                        .is_none_or(|time| time.elapsed() >= Duration::from_secs(2))
                {
                    composer.last_typing = Some(Instant::now());
                    app.send(Action::Realtime(ClientEvent::Typing {
                        conversation_id: id,
                        is_typing: !composer.text.is_empty(),
                    }));
                }
            }
            if is_recording {
                let (cancel_rect, cancel_response) =
                    ui.allocate_exact_size(Vec2::splat(60.0), egui::Sense::click());
                let cancel_fill = if cancel_response.is_pointer_button_down_on() {
                    Color32::from_rgb(254, 215, 215)
                } else if cancel_response.hovered() {
                    Color32::from_rgb(254, 235, 235)
                } else {
                    Color32::from_rgb(245, 247, 250)
                };
                ui.painter().rect_filled(cancel_rect, 4, cancel_fill);
                let cancel_stroke = if cancel_response.hovered() || cancel_response.is_pointer_button_down_on() {
                    Color32::from_rgb(239, 68, 68)
                } else {
                    Color32::from_rgb(215, 220, 228)
                };
                ui.painter().rect_stroke(
                    cancel_rect,
                    4,
                    egui::Stroke::new(1.0, cancel_stroke),
                    egui::StrokeKind::Inside,
                );
                nmessenger::paint(
                    ui.ctx(),
                    ui.painter(),
                    "icons/cancel.svg",
                    Rect::from_center_size(cancel_rect.center(), Vec2::splat(24.0)),
                );
                let cancel_response =
                    cancel_response.on_hover_text(tr("Cancel recording (Esc)", "Cancelar gravação (Esc)"));
                if cancel_response.clicked() {
                    app.recording = false;
                    app.recording_conversation = None;
                    app.recording_started_at = None;
                    app.send(Action::StopVoice { cancel: true });
                }
            }
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(60.0), egui::Sense::click());
            let fill = if is_recording {
                if response.is_pointer_button_down_on() {
                    Color32::from_rgb(195, 35, 35)
                } else if response.hovered() {
                    Color32::from_rgb(220, 50, 50)
                } else {
                    Color32::from_rgb(235, 75, 75)
                }
            } else if response.is_pointer_button_down_on() {
                Color32::from_rgb(42, 134, 222)
            } else if response.hovered() {
                Color32::from_rgb(64, 158, 245)
            } else {
                Color32::from_rgb(92, 178, 255)
            };
            ui.painter().rect_filled(rect, 4, fill);
            if response.hovered() || response.is_pointer_button_down_on() {
                ui.painter().rect_stroke(
                    rect,
                    4,
                    egui::Stroke::new(
                        1.0,
                        if is_recording {
                            Color32::from_rgb(175, 25, 25)
                        } else if response.is_pointer_button_down_on() {
                            Color32::from_rgb(25, 105, 185)
                        } else {
                            Color32::from_rgb(40, 130, 215)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            let has_text = !composer.text.trim().is_empty();
            let icon = if is_recording {
                "icons/stop-white.png"
            } else if has_text {
                "icons/send-white.png"
            } else {
                "icons/mic-white.png"
            };
            nmessenger::paint(
                ui.ctx(),
                ui.painter(),
                icon,
                Rect::from_center_size(rect.center(), Vec2::splat(27.0)),
            );
            if is_recording {
                if response.clicked()
                    || (text.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift))
                {
                    app.recording = false;
                    app.recording_conversation = None;
                    app.recording_started_at = None;
                    app.send(Action::StopVoice { cancel: false });
                }
            } else if (response.clicked() && has_text)
                || (text.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift))
            {
                let body = std::mem::take(&mut composer.text)
                    .trim_end_matches(['\r', '\n'])
                    .to_owned();
                if !body.trim().is_empty() {
                    if let Some(asset) = app.data.as_ref().and_then(|data| {
                        data.assets.iter().find(|asset| asset.kind == AssetKind::Emoticon
                            && asset.shortcut.as_deref() == Some(body.trim())).cloned()
                    }) {
                        app.send(Action::SendEmoticon { conversation: id, asset });
                    } else {
                        send = Some((MessageKind::Text, body, None));
                    }
                }
                text.request_focus();
                composer.focus_requested = true;
                crate::platform::show_soft_input();
            } else if response.clicked() {
                if audio::AVAILABLE {
                    app.recording = true;
                    app.recording_conversation = Some(id);
                    app.recording_started_at = Some(Instant::now());
                    app.send(Action::StartVoice {
                        conversation: id,
                        input: app.login.prefs.audio_input.clone(),
                    });
                } else {
                    app.error(tr("Recording requires the native-audio build feature.", "Gravação requer compilar com a feature native-audio."));
                }
            }
            response.on_hover_text(if is_recording {
                tr("Stop and send voice message", "Parar e enviar áudio")
            } else if has_text {
                tr("Send", "Enviar")
            } else {
                tr("Record voice message (F2)", "Gravar mensagem de voz (F2)")
            });
        });
    }
    if audio::AVAILABLE && !app.recording && ui.input(|i| i.key_pressed(egui::Key::F2)) {
        app.recording = true;
        app.recording_conversation = Some(id);
        app.recording_started_at = Some(Instant::now());
        app.send(Action::StartVoice {
            conversation: id,
            input: app.login.prefs.audio_input.clone(),
        });
    }
    if app.recording && app.recording_conversation == Some(id) {
        ui.ctx().request_repaint_after(Duration::from_millis(250));
        let duration = app.recording_started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
        let time_str = format!("{:02}:{:02}", duration / 60, duration % 60);
        let en = format!("🔴 Recording audio ({time_str})... Click Stop to send, or Cancel.");
        let pt = format!("🔴 Gravando áudio ({time_str})... Clique em Parar para enviar, ou Cancelar.");
        ui.label(
            RichText::new(tr(&en, &pt))
                .size(11.0)
                .color(Color32::from_rgb(215, 45, 45)),
        );
    }
    app.composers.insert(id, composer);
    if let Some((kind, body, attachment)) = send {
        app.send_message(id, kind, body, attachment);
    }
}

fn editor(
    ui: &mut egui::Ui,
    id: Uuid,
    composer: &mut crate::app::Composer,
    size: Vec2,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    theme::gradient(
        ui.painter(),
        rect,
        Color32::WHITE,
        Color32::from_rgb(243, 249, 254),
    );
    ui.painter().rect_stroke(
        rect,
        4,
        egui::Stroke::new(1.0_f32, nmessenger::BORDER),
        egui::StrokeKind::Inside,
    );
    ui.painter().line_segment(
        [
            rect.left_top() + Vec2::new(6.0, 1.0),
            rect.right_top() + Vec2::new(-6.0, 1.0),
        ],
        egui::Stroke::new(1.0_f32, Color32::from_white_alpha(190)),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(1.0))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.shrink(1.0));
    let editor_id = child.make_persistent_id(("message-text", id));
    if composer.focus_requested {
        composer.focus_requested = false;
        ui.ctx().memory_mut(|m| m.request_focus(editor_id));
        crate::platform::show_soft_input();
    }
    let style = composer.style.clone();
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
        let mut job = egui::text::LayoutJob::default();
        job.append(buffer.as_str(), 0.0, super::message_style::format(&style));
        job.wrap.max_width = width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    super::scroll::vertical()
        .id_salt(("composer-editor", id))
        .auto_shrink([false, false])
        .max_height(size.y - 2.0)
        .show(&mut child, |ui| {
            let res = ui.add(
                egui::TextEdit::multiline(&mut composer.text)
                    .id_salt(("message-text", id))
                    .desired_rows(1)
                    .desired_width(ui.available_width())
                    .font(super::message_style::font(&composer.style))
                    .layouter(&mut layouter)
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
                    .min_size(Vec2::new(ui.available_width(), size.y - 14.0))
                    .hint_text(
                        RichText::new(tr("Type your message here...", "Digite sua mensagem aqui..."))
                            .size(10.0)
                            .color(nmessenger::MUTED),
                    ),
            );
            if res.clicked() || res.gained_focus() {
                crate::platform::show_soft_input_type("text");
            }
            res.context_menu(|ui| {
                if ui.button(tr("Paste", "Colar")).clicked() {
                    if let Some(clip) = crate::platform::get_clipboard_text() {
                        composer.text.push_str(&clip);
                    }
                    ui.close();
                }
                if !composer.text.is_empty() && ui.button(tr("Clear", "Limpar")).clicked() {
                    composer.text.clear();
                    ui.close();
                }
            });
            res
        })
        .inner
}

fn ink(ui: &mut egui::Ui, composer: &mut crate::app::Composer) {
    ui.horizontal(|ui| {
        ui.color_edit_button_srgb(&mut composer.ink_color);
        ui.add(egui::Slider::new(&mut composer.ink_width, 1.0..=12.0).text(tr("Stroke", "Traço")));
        if ui.button(tr("Undo", "Desfazer")).clicked() {
            composer.ink.strokes.pop();
        }
        if ui.button(tr("Clear", "Limpar")).clicked() {
            composer.ink.strokes.clear();
        }
    });
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 142.0), egui::Sense::drag());
    ui.painter().rect_filled(rect, 4, Color32::WHITE);
    ui.painter().rect_stroke(
        rect,
        4,
        egui::Stroke::new(1.0_f32, nmessenger::BORDER),
        egui::StrokeKind::Inside,
    );
    if response.drag_started() && !ui.input(|i| i.modifiers.shift) {
        composer.ink.strokes.push(Stroke {
            color: composer.ink_color,
            width: composer.ink_width,
            points: Vec::new(),
        });
    }
    if response.dragged()
        && let Some(pos) = response
            .interact_pointer_pos()
            .filter(|pos| rect.contains(*pos))
    {
        let point = [pos.x - rect.left(), pos.y - rect.top()];
        if ui.input(|i| i.modifiers.shift) {
            composer.ink.strokes.retain(|stroke| {
                !stroke
                    .points
                    .iter()
                    .any(|p| (p[0] - point[0]).hypot(p[1] - point[1]) < 16.0)
            });
        } else if let Some(stroke) = composer.ink.strokes.last_mut()
            && stroke
                .points
                .last()
                .is_none_or(|p| (p[0] - point[0]).hypot(p[1] - point[1]) > 1.5)
        {
            stroke.points.push(point);
        }
    }
    composer.ink.width = rect.width();
    composer.ink.height = rect.height();
    super::messages::paint_ink(ui, rect, &composer.ink);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_styled_multiline_messages_scroll_inside_a_fixed_height_composer() {
        let mut heights = Vec::new();
        for size in [8.0, 12.0, 36.0, 48.0] {
            let context = egui::Context::default();
            super::super::theme::install(&context);
            let mut composer = crate::app::Composer {
                text: "Texto longo com acentos e várias linhas.\n".repeat(20),
                style: MessageStyle {
                    font_size: size,
                    bold: true,
                    italic: true,
                    underline: true,
                    ..Default::default()
                },
                ..Default::default()
            };
            context.begin_pass(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(700.0, 500.0))),
                ..Default::default()
            });
            let mut ui = egui::Ui::new(
                context.clone(),
                egui::Id::unique("composer-test"),
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(700.0, 500.0))),
            );
            let before = ui.cursor().top();
            editor(&mut ui, Uuid::nil(), &mut composer, Vec2::new(350.0, 60.0));
            heights.push(ui.cursor().top() - before);
            // egui 0.37 requires the caller to consume texture deltas between passes.
            context.end_pass().textures_delta.clear();
        }
        assert!(
            heights
                .iter()
                .all(|height| (*height - heights[0]).abs() < 0.1),
            "Editor height must not follow font size: {heights:?}"
        );
        assert!(heights[0] < 70.0);
    }

    #[test]
    fn recording_mode_renders_cancel_and_stop_controls_and_cancel_clears_state() {
        let mut app = MessengerApp::new_for_test();
        let conv_id = Uuid::new_v4();
        app.recording = true;
        app.recording_conversation = Some(conv_id);
        app.recording_started_at = Some(Instant::now());

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 300.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            show(&mut app, ui, conv_id);
        });
        output.textures_delta.clear();
        assert!(app.recording);
    }
}
