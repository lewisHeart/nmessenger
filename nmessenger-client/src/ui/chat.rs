use super::{activities, composer, messages, nmessenger, theme};
use crate::i18n::tr;
use crate::{app::MessengerApp, media::images::ImageSource, transport::Action};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use nm_protocol::*;
use uuid::Uuid;

#[cfg(not(target_os = "android"))]
pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    detached(app, ctx);
}

fn call_avatar(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation, profile: &Profile, rect: Rect, role: &str) {
    let frame = app.current_call.and_then(|activity| {
        let video_call = app.data.as_ref().is_some_and(|data| data.activities.iter().any(|item|
            item.id == activity && item.conversation_id == conversation.id
                && matches!(item.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) && item.status == ActivityStatus::Accepted));
        video_call.then(|| app.live_frames.get(&(activity, profile.id)))
            .flatten().map(|texture| (texture.id(), texture.size_vec2()))
    });
    if let Some((texture, size)) = frame {
        let inner = nmessenger::frame(ui.ctx(), ui.painter(), rect, profile.presence, role);
        ui.put(inner, egui::Image::new((texture, inner.size()))
            .uv(nmessenger::cover_uv(size, inner.size()))
            .corner_radius(nmessenger::frame_radius(inner)));
        nmessenger::paint_avatar_rim(ui.painter(), inner, profile.presence);
    } else {
        nmessenger::avatar_at(app, ui, profile, rect, role);
    }
}

#[cfg(target_os = "android")]
pub fn mobile(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let Some(conversation) = app
        .active
        .and_then(|id| app.data.as_ref()?.conversations.iter().find(|c| c.id == id))
        .cloned()
    else {
        app.active = None;
        return;
    };
    let me = app.data.as_ref().map(|d| d.me.id);
    let other = conversation.members.iter().find(|m| Some(m.id) != me);
    let title = app.conversation_title(&conversation);

    if let Some(activity) = app.mobile_call_view.and_then(|id| app.data.as_ref()?.activities.iter()
        .find(|activity| activity.id == id && activity.conversation_id == conversation.id
            && matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) && activity.status == ActivityStatus::Accepted)).cloned() {
        super::mobile_call::show(app, ui, &conversation, &activity);
        return;
    }

    // Modern mobile chat top bar
    egui::Panel::top("mobile-chat-bar")
        .exact_size(52.0)
        .show_separator_line(true)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_white_alpha(235))
                .inner_margin(egui::Margin::symmetric(8, 6)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let back_resp = ui.add(
                    egui::Button::new(
                        RichText::new("‹")
                            .size(24.0)
                            .family(egui::FontFamily::Name("Segoe UI Bold".into()))
                            .color(theme::INK),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .min_size(Vec2::new(32.0, 40.0)),
                );
                if back_resp.clicked() {
                    app.active = None;
                }

                let avatar_size = 36.0;
                let (avatar_rect, avatar_resp) =
                    ui.allocate_exact_size(Vec2::splat(avatar_size), egui::Sense::click());
                let avatar_resp = avatar_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                if let Some(other) = other {
                    call_avatar(app, ui, &conversation, other, avatar_rect, "mobile-chat-av");
                    if avatar_resp.clicked() {
                        app.contact_forms.view_profile = Some(other.clone());
                    }
                } else if conversation.kind == ConversationKind::Group {
                    nmessenger::paint(
                        ui.ctx(),
                        ui.painter(),
                        "status/disponivel_icone.png",
                        avatar_rect.shrink(4.0),
                    );
                    if avatar_resp.clicked() {
                        app.conversation_forms.conversation = Some(conversation.id);
                        app.conversation_forms.title =
                            conversation.title.clone().unwrap_or_default();
                        app.conversation_forms.members_open = true;
                    }
                }

                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&title)
                            .size(13.5)
                            .strong()
                            .color(nmessenger::HEADING),
                    );
                    let subtitle = if let Some(other) = other {
                        nmessenger::status_label(other.presence).to_string()
                    } else {
                        format!("{} {}", conversation.members.len(), tr("participants", "participantes"))
                    };
                    ui.label(RichText::new(subtitle).size(10.5).color(nmessenger::MUTED));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (more_rect, more_resp) =
                        ui.allocate_exact_size(Vec2::new(36.0, 36.0), egui::Sense::click());
                    let more_resp = more_resp.on_hover_text(tr("More options", "Mais opções"));
                    let more_open = egui::Popup::is_id_open(ui.ctx(), more_resp.id.with("popup"));
                    if more_resp.is_pointer_button_down_on() || more_open {
                        ui.painter().rect_filled(
                            more_rect,
                            4,
                            Color32::from_rgba_unmultiplied(185, 220, 252, 230),
                        );
                        ui.painter().rect_stroke(
                            more_rect,
                            4,
                            Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                            egui::StrokeKind::Inside,
                        );
                    } else if more_resp.hovered() {
                        ui.painter().rect_filled(
                            more_rect,
                            4,
                            Color32::from_rgba_unmultiplied(225, 240, 255, 210),
                        );
                        ui.painter().rect_stroke(
                            more_rect,
                            4,
                            Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    nmessenger::paint(
                        ui.ctx(),
                        ui.painter(),
                        "icons/chevron.png",
                        egui::Rect::from_center_size(more_rect.center(), Vec2::splat(16.0)),
                    );
                    egui::Popup::menu(&more_resp).show(|ui| more(app, ui, &conversation));

                    let (pic_rect, pic_resp) =
                        ui.allocate_exact_size(Vec2::new(36.0, 36.0), egui::Sense::click());
                    let pic_resp = pic_resp.on_hover_text(tr("Shared media", "Mídias compartilhadas"));
                    if pic_resp.is_pointer_button_down_on() {
                        ui.painter().rect_filled(
                            pic_rect,
                            4,
                            Color32::from_rgba_unmultiplied(185, 220, 252, 230),
                        );
                        ui.painter().rect_stroke(
                            pic_rect,
                            4,
                            Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                            egui::StrokeKind::Inside,
                        );
                    } else if pic_resp.hovered() {
                        ui.painter().rect_filled(
                            pic_rect,
                            4,
                            Color32::from_rgba_unmultiplied(225, 240, 255, 210),
                        );
                        ui.painter().rect_stroke(
                            pic_rect,
                            4,
                            Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    nmessenger::paint(
                        ui.ctx(),
                        ui.painter(),
                        "icons/framed-picture.png",
                        egui::Rect::from_center_size(pic_rect.center(), Vec2::splat(18.0)),
                    );
                    if pic_resp.clicked() {
                        let h = app.histories.entry(conversation.id).or_default();
                        h.show_media_gallery = !h.show_media_gallery;
                    }

                    if other.is_some() && conversation.kind == ConversationKind::Direct {
                        let (call_rect, call_resp) =
                            ui.allocate_exact_size(Vec2::new(36.0, 36.0), egui::Sense::click());
                        let call_resp = call_resp.on_hover_text(tr("Start voice call", "Chamar por voz"));
                        if call_resp.is_pointer_button_down_on() {
                            ui.painter().rect_filled(
                                call_rect,
                                4,
                                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
                            );
                            ui.painter().rect_stroke(
                                call_rect,
                                4,
                                Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                                egui::StrokeKind::Inside,
                            );
                        } else if call_resp.hovered() {
                            ui.painter().rect_filled(
                                call_rect,
                                4,
                                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
                            );
                            ui.painter().rect_stroke(
                                call_rect,
                                4,
                                Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        nmessenger::paint(
                            ui.ctx(),
                            ui.painter(),
                            "icons/call.png",
                            egui::Rect::from_center_size(call_rect.center(), Vec2::splat(18.0)),
                        );
                        if call_resp.clicked() {
                            offer_call(app, conversation.id, ActivityKind::AudioCall);
                        }
                    } else if conversation.kind == ConversationKind::Group {
                        let (grp_rect, grp_resp) =
                            ui.allocate_exact_size(Vec2::new(36.0, 36.0), egui::Sense::click());
                        let grp_resp = grp_resp.on_hover_text(tr("Participants", "Participantes"));
                        if grp_resp.is_pointer_button_down_on() {
                            ui.painter().rect_filled(
                                grp_rect,
                                4,
                                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
                            );
                            ui.painter().rect_stroke(
                                grp_rect,
                                4,
                                Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                                egui::StrokeKind::Inside,
                            );
                        } else if grp_resp.hovered() {
                            ui.painter().rect_filled(
                                grp_rect,
                                4,
                                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
                            );
                            ui.painter().rect_stroke(
                                grp_rect,
                                4,
                                Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        nmessenger::paint(
                            ui.ctx(),
                            ui.painter(),
                            "icons/person.png",
                            egui::Rect::from_center_size(grp_rect.center(), Vec2::splat(18.0)),
                        );
                        if grp_resp.clicked() {
                            app.conversation_forms.conversation = Some(conversation.id);
                            app.conversation_forms.title =
                                conversation.title.clone().unwrap_or_default();
                            app.conversation_forms.members_open = true;
                        }
                    }
                });
            });
        });

    let shake_offset = super::window::shake(&ctx, app.nudges.get(&conversation.id).copied());
    let mut render_mobile_body = |ui: &mut egui::Ui| {
        body(app, ui, &conversation, true);
    };
    if shake_offset != Vec2::ZERO {
        ui.with_visual_transform(
            egui::emath::TSTransform::from_translation(shake_offset),
            |ui| render_mobile_body(ui),
        );
    } else {
        render_mobile_body(ui);
    }
    dialogs(app, &ctx);
    if app.options.game_browser == Some(conversation.id) {
        super::games::show(app, &ctx);
    }
}

fn body(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation, is_mobile: bool) {
    let id = conversation.id;
    let show_gallery = app.histories.get(&id).is_some_and(|h| h.show_media_gallery);
    if is_mobile && show_gallery {
        media_gallery_panel(app, ui, conversation, true);
        return;
    }

    let keyboard_inset = crate::platform::keyboard_height(ui.ctx());
    let bottom_margin = if is_mobile {
        if keyboard_inset > 0.0 {
            4
        } else {
            let nav = crate::platform::safe_area_bottom();
            (16.0 + nav.min(24.0)).round() as i8
        }
    } else {
        12
    };

    // Bottom panel: composer (must be declared before CentralPanel to anchor at bottom)
    egui::Panel::bottom(egui::Id::unique(("composer", id)))
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 14,
                    top: 0,
                    bottom: bottom_margin,
                }),
        )
        .show(ui, |ui| {
            composer::show(app, ui, id);
        });

    if is_mobile && let Some(activity) = app.current_call.and_then(|current| app.data.as_ref()?.activities.iter()
        .find(|activity| activity.id == current && activity.conversation_id == id
            && matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) && activity.status == ActivityStatus::Accepted)).cloned() {
        egui::Panel::top(egui::Id::unique(("mobile-call-banner", id)))
            .exact_size(38.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(Color32::from_rgb(228, 242, 252)).inner_margin(5))
            .show(ui, |ui| {
                let camera_on = app.media_status.as_ref().map_or(activity.kind == ActivityKind::VideoCall, |status| status.camera_enabled);
                let text = if camera_on { tr("Call · Camera on  ›", "Chamada · Câmera ligada  ›") }
                    else { tr("Call · Camera off  ›", "Chamada · Câmera desligada  ›") };
                if ui.add_sized(ui.available_size(), egui::Button::new(RichText::new(text).color(nmessenger::HEADING)).frame(false)).clicked() {
                    app.mobile_call_view = Some(activity.id);
                }
            });
    }

    if !is_mobile && ui.available_width() >= 560.0 {
            egui::Panel::left(egui::Id::unique(("display-pictures", id)))
                .exact_size(222.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .inner_margin(0),
                )
                .show(ui, |ui| {
                    let r = ui.max_rect();
                    let me = app.data.as_ref().map(|d| d.me.clone());
                    let has_game = app.data.as_ref().is_some_and(|data| {
                        data.activities.iter().any(|a| {
                            a.conversation_id == conversation.id
                                && a.kind == ActivityKind::Game
                                && matches!(
                                    a.status,
                                    ActivityStatus::Offered | ActivityStatus::Accepted
                                )
                        })
                    });
                    if has_game {
                        super::games::panel(app, ui, conversation);
                        return;
                    }
                    if conversation.kind == ConversationKind::Group {
                        super::conversation_sidebar::members(app, ui, conversation);
                        return;
                    }
                    if let Some(other) = conversation
                        .members
                        .iter()
                        .find(|m| Some(m.id) != me.as_ref().map(|m| m.id))
                    {
                        call_avatar(
                            app,
                            ui,
                            conversation,
                            other,
                            egui::Rect::from_min_size(
                                r.min + Vec2::new(44.0, 16.0),
                                Vec2::splat(160.0),
                            ),
                            "conversa",
                        );
                    }
                    if let Some(me) = &me {
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(r.left() + 44.0, r.bottom() - 176.0),
                            Vec2::splat(160.0),
                        );
                        call_avatar(app, ui, conversation, me, rect, "conversa");
                    }
                    let other = conversation.members.iter().find(|member| Some(member.id) != me.as_ref().map(|profile| profile.id)).cloned();
                    for (index, y) in [r.top() + 22.0, r.bottom() - 170.0].into_iter().enumerate() {
                        let arrow = egui::Rect::from_min_size(egui::pos2(r.left() + 19.0, y), Vec2::splat(16.0));
                        let arrow_response = ui.interact(arrow, ui.scope_id().with(("avatar-menu", index)), egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(tr("Profile and call options", "Opções de perfil e chamada"));
                        let arrow_open = egui::Popup::is_id_open(ui.ctx(), arrow_response.id.with("popup"));
                        let arrow_held = arrow_response.is_pointer_button_down_on();
                        let arrow_hovered = arrow_response.hovered();

                        if arrow_held || arrow_open {
                            ui.painter().rect_filled(
                                arrow,
                                3,
                                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
                            );
                            ui.painter().rect_stroke(
                                arrow,
                                3,
                                egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                                egui::StrokeKind::Inside,
                            );
                        } else if arrow_hovered {
                            ui.painter().rect_filled(
                                arrow,
                                3,
                                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
                            );
                            ui.painter().rect_stroke(
                                arrow,
                                3,
                                egui::Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let arrow_icon_rect = if arrow_held || arrow_open {
                            Rect::from_center_size(arrow.center() + Vec2::new(0.0, 1.0), Vec2::splat(9.0))
                        } else {
                            Rect::from_center_size(arrow.center(), Vec2::splat(9.0))
                        };
                        nmessenger::paint(
                            ui.ctx(),
                            ui.painter(),
                            "icons/arrow.png",
                            arrow_icon_rect,
                        );
                        egui::Popup::menu(&arrow_response).show(|ui| {
                            avatar_actions(app, ui, conversation, if index == 0 { other.as_ref() } else { me.as_ref() }, index == 1);
                        });

                        let camera = egui::Rect::from_min_size(
                            egui::pos2(r.left() + 19.0, y + 20.0),
                            Vec2::splat(16.0),
                        );
                        let response = ui.interact(
                                camera,
                                ui.scope_id().with(("camera", y as i32)),
                                egui::Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(tr("Camera options", "Opções da câmera"));
                        let cam_open = egui::Popup::is_id_open(ui.ctx(), response.id.with("popup"));
                        let cam_held = response.is_pointer_button_down_on();
                        let cam_hovered = response.hovered();

                        if cam_held || cam_open {
                            ui.painter().rect_filled(
                                camera,
                                3,
                                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
                            );
                            ui.painter().rect_stroke(
                                camera,
                                3,
                                egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                                egui::StrokeKind::Inside,
                            );
                        } else if cam_hovered {
                            ui.painter().rect_filled(
                                camera,
                                3,
                                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
                            );
                            ui.painter().rect_stroke(
                                camera,
                                3,
                                egui::Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let cam_icon_rect = if cam_held || cam_open {
                            Rect::from_center_size(camera.center() + Vec2::new(0.0, 1.0), Vec2::splat(13.0))
                        } else {
                            Rect::from_center_size(camera.center(), Vec2::splat(13.0))
                        };
                        nmessenger::paint(ui.ctx(), ui.painter(), "icons/camera.png", cam_icon_rect);
                        egui::Popup::menu(&response).show(|ui| {
                            camera_actions(app, ui, conversation, true);
                        });
                    }
                });
        }
        if !is_mobile && show_gallery {
            egui::Panel::right(egui::Id::unique(("media-gallery-sidebar", id)))
                .exact_size(320.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(252, 254, 255))
                        .inner_margin(0),
                )
                .show(ui, |ui| {
                    media_gallery_panel(app, ui, conversation, false);
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::TRANSPARENT).inner_margin(0))
            .show(ui, |ui| {
                if !is_mobile {
                    header(app, ui, conversation);
                }
                if app.histories.get(&id).is_some_and(|h| h.show_search) {
                    history_toolbar(app, ui, conversation);
                }
                activities::show(app, ui, conversation);
        let history = app.histories.entry(id).or_default();
        let history_messages = history.messages.clone();
        let loading = history.loading;
        let more = history.more;
        let query = history.query.clone();
        let error = history.error.clone();
        let grouped = app.data.as_ref().is_some_and(|d| d.settings.group_messages);
        egui::Frame::new()
            .fill(egui::Color32::TRANSPARENT)
            .inner_margin(egui::Margin {
                left: 12,
                right: 14,
                top: 5,
                bottom: 0,
            })
            .show(ui, |ui| {
                super::scroll::vertical()
                    .id_salt(("history", id))
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(ui.available_height().max(60.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if loading {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(tr("Loading…", "Carregando…"));
                            });
                        } else if more && ui.button(tr("Load earlier messages", "Carregar mensagens anteriores")).clicked() {
                            app.load_history(
                                id,
                                history_messages.first().map(|m| m.id),
                                query.clone(),
                            );
                        }
                        if error.is_some() && ui.button(tr("Try loading again", "Tentar carregar novamente")).clicked() {
                            app.load_history(id, None, query.clone());
                        }
                        if history_messages.is_empty() && !loading && !query.is_empty() {
                            ui.label(
                                RichText::new(tr("No messages found for this search.", "Nenhuma mensagem encontrada nesta busca."))
                                    .color(theme::MUTED),
                            );
                        }
                        let mut previous: Option<&Message> = None;
                        let scroll_target = app.histories.get(&id).and_then(|h| h.scroll_to_message);
                        let mut did_scroll = false;
                        for message in &history_messages {
                            let group = grouped
                                && previous.is_some_and(|p| {
                                    p.sender_id == message.sender_id
                                        && (message.created_at - p.created_at).num_seconds() < 120
                                });
                            let is_target = scroll_target == Some(message.id);
                            let resp = ui.push_id(message.id, |ui| {
                                if is_target {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(222, 240, 255))
                                        .stroke(Stroke::new(1.5, Color32::from_rgb(80, 150, 235)))
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::symmetric(6, 4))
                                        .show(ui, |ui| {
                                            messages::show(app, ui, message, group);
                                        });
                                } else {
                                    messages::show(app, ui, message, group);
                                }
                            });
                            if is_target {
                                resp.response.scroll_to_me(Some(egui::Align::Center));
                                did_scroll = true;
                            }
                            previous = Some(message);
                        }
                        if did_scroll
                            && let Some(h) = app.histories.get_mut(&id) {
                                h.scroll_to_message = None;
                            }
                        pending(app, ui, id);
                        let typers: Vec<_> = app
                            .typing
                            .keys()
                            .filter(|(conversation, _)| *conversation == id)
                            .map(|(_, user)| app.name(*user))
                            .collect();
                        if !typers.is_empty() {
                            ui.label(
                                RichText::new(format!("{} {}", typers.join(", "), tr("typing…", "digitando…")))
                                    .italics()
                                    .color(theme::MUTED),
                            );
                        }
                        ui.add_space(8.0);
                    });
            });
    });
    messages::effects(app, ui, id, ui.ctx().content_rect());
}

fn avatar_actions(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation, profile: Option<&Profile>, own: bool) {
    if let Some(profile) = profile && ui.button(tr("View profile", "Ver perfil")).clicked() {
        app.contact_forms.view_profile = Some(profile.clone());
        ui.close();
    }
    if own {
        if ui.button(tr("Change picture...", "Alterar imagem...")).clicked() {
            app.options.avatar_picker = true;
            ui.close();
        }
    } else {
        for (kind, label) in [(ActivityKind::AudioCall, tr("Start voice call", "Iniciar chamada de voz")),
                              (ActivityKind::VideoCall, tr("Start video call", "Iniciar videochamada"))] {
            if ui.add_enabled(!app.call_busy(None), egui::Button::new(label)).clicked() {
                offer_call(app, conversation.id, kind);
                ui.close();
            }
        }
    }
    ui.separator();
    camera_actions(app, ui, conversation, false);
}

fn camera_actions(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation, allow_start_call: bool) {
    let video = app.current_call.and_then(|id| app.data.as_ref()?.activities.iter()
        .find(|activity| activity.id == id && activity.conversation_id == conversation.id
            && matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) && activity.status == ActivityStatus::Accepted)).is_some();
    if video {
        let enabled = app.media_status.as_ref().is_none_or(|status| status.camera_enabled);
        if ui.button(if enabled { tr("Turn camera off", "Desligar câmera") } else { tr("Turn camera on", "Ligar câmera") }).clicked() {
            app.set_camera_enabled(!enabled);
            ui.close();
        }
        let sharing = app.media_status.as_ref().is_some_and(|status| status.screen_sharing);
        if ui.button(if sharing { tr("Stop sharing screen", "Parar compartilhamento de tela") } else { tr("Share screen", "Compartilhar tela") }).clicked() {
            app.set_screen_sharing(!sharing);
            ui.close();
        }
    } else if allow_start_call && !app.call_busy(None) && ui.button(tr("Start video call", "Iniciar videochamada")).clicked() {
        offer_call(app, conversation.id, ActivityKind::VideoCall);
        ui.close();
    }
    if ui.button(tr("Camera and audio settings...", "Configurações de câmera e áudio...")).clicked() {
        app.open_options(); app.options.tab = 7;
        ui.close();
    }
}

fn dialogs(app: &mut MessengerApp, ctx: &egui::Context) {
    if app
        .conversation_forms
        .viewport
        .unwrap_or(egui::ViewportId::ROOT)
        != ctx.viewport_id()
    {
        return;
    }
    members(app, ctx);
    clear_history(app, ctx);
}

#[cfg(not(target_os = "android"))]
fn detached(app: &mut MessengerApp, ctx: &egui::Context) {
    for id in app.detached.clone() {
        let Some(conversation) = app
            .data
            .as_ref()
            .and_then(|d| d.conversations.iter().find(|c| c.id == id))
            .cloned()
        else {
            app.detached.remove(&id);
            if app.active == Some(id) {
                app.active = None;
            }
            app.conversation_focus.remove(&id);
            continue;
        };
        let viewport = egui::ViewportId::from_hash_of(("nm-conversation", id));
        let chat_window_title = app.conversation_window_title(&conversation);
        ctx.show_viewport_immediate(
            viewport,
            egui::ViewportBuilder::default()
                .with_title(&chat_window_title)
                .with_inner_size(super::window::initial_size(ctx, &format!("chat:{id}"), nmessenger::CHAT_SIZE))
                .with_min_inner_size([460.0, 400.0])
                .with_decorations(super::window::system_titlebars(ctx))
                .with_transparent(true)
                .with_icon(nmessenger::app_icon()),
            |ui, class| {
                let ctx = ui.ctx().clone();
                if class != egui::ViewportClass::EmbeddedWindow { super::window::record_size(&ctx, &format!("chat:{id}")); }
                super::window::minimum(&ctx, Vec2::new(460.0, 400.0));
                let shake_offset = super::window::shake(&ctx, app.nudges.get(&id).copied());
                if ctx.input(|i| i.viewport().close_requested()) {
                    super::window::forget_initial_size(&ctx, &format!("chat:{id}"));
                    app.detached.remove(&id);
                    if app.active == Some(id) {
                        app.active = None;
                    }
                    app.conversation_focus.remove(&id);
                }
                if ctx.input(|i| !i.events.is_empty()) {
                    app.last_input = std::time::Instant::now();
                }
                let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
                if focused {
                    app.conversation_focus.insert(id);
                } else {
                    app.conversation_focus.remove(&id);
                }
                let mut render_content = |ui: &mut egui::Ui| {
                    if class == egui::ViewportClass::EmbeddedWindow {
                        let mut open = true;
                        let mut closed_by_title = false;
                        let screen = ctx.content_rect();
                        let win_w = (screen.width() - 16.0).min(760.0).max(480.0);
                        let win_h = (screen.height() - 24.0).min(660.0).max(400.0);
                        let default_pos = Pos2::new(8.0, 12.0) + shake_offset;
                        egui::Window::new(&chat_window_title)
                            .title_bar(false)
                            .frame(super::dialog::frame())
                            .id(egui::Id::unique(("chat-window", id)))
                            .open(&mut open)
                            .default_pos(default_pos)
                            .default_size([win_w, win_h])
                            .min_size([460.0, 400.0])
                            .resizable(true)
                            .show(&ctx, |ui| {
                                closed_by_title |= super::dialog::embedded_title(ui, &chat_window_title);
                                body(app, ui, &conversation, false);
                            });
                        if closed_by_title {
                            open = false;
                        }
                        if !open {
                            app.detached.remove(&id);
                            if app.active == Some(id) {
                                app.active = None;
                            }
                            app.conversation_focus.remove(&id);
                        }
                    } else {
                        theme::window_bar_with_controls(
                            ui,
                            &chat_window_title,
                            theme::WindowControls::All,
                        );
                        body(app, ui, &conversation, false);
                    }
                };
                if shake_offset != Vec2::ZERO {
                    ui.with_visual_transform(
                        egui::emath::TSTransform::from_translation(shake_offset),
                        |ui| {
                            render_content(ui);
                        },
                    );
                } else {
                    render_content(ui);
                }
                if focused {
                    app.mark_visible(id);
                }
                dialogs(app, &ctx);
                if app.options.game_browser == Some(id) {
                    super::games::show(app, &ctx);
                }
                // Each viewport repaints independently in egui 0.37: images, typing
                // indicators and animations must not wait for the next user input.
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            },
        );
    }
}

fn header(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    let me = app.data.as_ref().map(|d| d.me.id);
    let other = conversation.members.iter().find(|m| Some(m.id) != me);
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 86.0), egui::Sense::hover());
    let title = app.conversation_title(conversation);
    let compact = rect.width() < 500.0;
    let title_font = egui::FontId::proportional(if compact { 22.0 } else { 32.0 });
    let title_width = ui
        .fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(title.clone(), title_font.clone(), nmessenger::HEADING)
                .size()
                .x
        })
        .min((rect.width() - if compact { 155.0 } else { 278.0 }).max(90.0));
    ui.put(
        egui::Rect::from_min_size(
            rect.min + Vec2::new(12.0, 12.0),
            Vec2::new(title_width + 3.0, 43.0),
        ),
        egui::Label::new(
            RichText::new(title)
                .font(title_font)
                .color(nmessenger::HEADING),
        )
        .truncate(),
    );
    if let Some(other) = other
        && conversation.kind == ConversationKind::Direct
        && !compact
    {
        ui.painter().text(
            rect.min + Vec2::new(20.0 + title_width, 36.0),
            egui::Align2::LEFT_CENTER,
            format!("({})", nmessenger::status_label(other.presence)),
            egui::FontId::proportional(14.0),
            nmessenger::MUTED,
        );
    }
    let detail = if conversation.kind == ConversationKind::Group {
        format!(
            "{}: {}", tr("Members", "Membros"),
            conversation
                .members
                .iter()
                .map(|member| app.name(member.id))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        other
            .map(|m| m.personal_message.clone())
            .unwrap_or_default()
    };
    nmessenger::left(
        ui,
        egui::Rect::from_min_size(
            rect.min + Vec2::new(12.0, 52.0),
            Vec2::new((rect.width() - 225.0).max(40.0), 23.0),
        ),
        egui::Label::new(RichText::new(detail).size(14.0).color(nmessenger::MUTED)).truncate(),
    );
    for (name, offset, tooltip) in [
        ("game", 183.0, tr("Games and activities", "Jogos e atividades")),
        ("framed-picture", 143.0, tr("Shared media", "Mídias compartilhadas")),
        ("search-chat", 103.0, tr("Search this conversation", "Buscar nesta conversa")),
        ("call", 63.0, tr("Start call", "Iniciar chamada")),
        ("chevron", 31.0, tr("More options", "Mais opções")),
    ] {
        let area = egui::Rect::from_min_size(
            egui::pos2(rect.right() - offset, rect.top() + 28.0),
            Vec2::splat(28.0),
        );
        let response = ui
            .interact(
                area,
                ui.scope_id().with((conversation.id, name)),
                egui::Sense::click(),
            )
            .on_hover_text(tooltip);
        if response.is_pointer_button_down_on() {
            ui.painter().rect_filled(
                area,
                4,
                Color32::from_rgba_unmultiplied(185, 220, 252, 230),
            );
            ui.painter().rect_stroke(
                area,
                4,
                Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                egui::StrokeKind::Inside,
            );
        } else if response.hovered() {
            ui.painter().rect_filled(
                area,
                4,
                Color32::from_rgba_unmultiplied(225, 240, 255, 210),
            );
            ui.painter().rect_stroke(
                area,
                4,
                Stroke::new(1.0, Color32::from_rgb(145, 195, 245)),
                egui::StrokeKind::Inside,
            );
        }
        nmessenger::paint(
            ui.ctx(),
            ui.painter(),
            &format!("icons/{name}.png"),
            area.shrink(if name == "chevron" { 6.0 } else { 4.0 }),
        );
        match name {
            "game" if response.clicked() => {
                app.options.game_browser = Some(conversation.id);
            }
            "framed-picture" if response.clicked() => {
                let history = app.histories.entry(conversation.id).or_default();
                history.show_media_gallery = !history.show_media_gallery;
            }
            "search-chat" if response.clicked() => {
                let history = app.histories.entry(conversation.id).or_default();
                history.show_search = !history.show_search;
            }
            "call" if response.clicked() => {
                offer_call(app, conversation.id, ActivityKind::AudioCall)
            }
            "chevron" => {
                egui::Popup::menu(&response).show(|ui| more(app, ui, conversation));
            }
            _ => {}
        }
    }
}

fn offer_call(app: &mut MessengerApp, conversation: Uuid, kind: ActivityKind) {
    if app.call_busy(None) {
        app.error(tr("End your current call before starting another.", "Encerre a chamada atual antes de iniciar outra."));
        return;
    }
    if !crate::media::audio::AVAILABLE {
        app.error(tr("This build does not include native audio.", "Esta compilação não possui áudio nativo."));
        return;
    }
    if !app.data.as_ref().is_some_and(|d| {
        d.conversations
            .iter()
            .any(|c| c.id == conversation && c.members.len() == 2)
    }) {
        app.error(tr("Calls are available for conversations with two people.", "Chamadas disponíveis para conversas com duas pessoas."));
        return;
    }
    app.offered_calls.insert(conversation, kind);
    app.send(Action::Activity(conversation, ActivityRequest { kind,
        payload: serde_json::json!({"audio":"pcm_s16le","sample_rate":16000,"channels":1,"video":"jpeg"}) }));
}

fn more(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    let me = app.data.as_ref().map(|d| d.me.id);
    let other = conversation.members.iter().find(|m| Some(m.id) != me).cloned();
    if let Some(other) = &other && conversation.kind == ConversationKind::Direct {
        if ui.button(tr("View profile", "Ver perfil")).clicked() {
            app.contact_forms.view_profile = Some(other.clone());
            ui.close();
        }
    }
    ui.menu_button(tr("Calls and sharing", "Chamadas e compartilhamento"), |ui| {
        activities::menu_contents(app, ui, conversation)
    });
    if other.is_some() && conversation.kind == ConversationKind::Direct {
        if ui.button(tr("Start voice call", "Iniciar chamada de voz")).clicked() {
            offer_call(app, conversation.id, ActivityKind::AudioCall);
            ui.close();
        }
        if ui.button(tr("Start video call", "Iniciar videochamada")).clicked() {
            offer_call(app, conversation.id, ActivityKind::VideoCall);
            ui.close();
        }
    }
    if ui.button(tr("Participants and invitations...", "Participantes e convites...")).clicked() {
        app.conversation_forms.conversation = Some(conversation.id);
        app.conversation_forms.viewport = Some(ui.ctx().viewport_id());
        app.conversation_forms.members_open = true;
        app.conversation_forms.title = conversation.title.clone().unwrap_or_default();
        ui.close();
    }
    if ui.button(tr("Shared media...", "Mídias compartilhadas...")).clicked() {
        let history = app.histories.entry(conversation.id).or_default();
        history.show_media_gallery = true;
        ui.close();
    }
    ui.menu_button(tr("Save history", "Salvar histórico"), |ui| {
        for format in ["txt", "xml", "html"] {
            if ui.button(format!("{} .{format}", tr("Export", "Exportar"))).clicked() {
                app.send(Action::ExportHistory {
                    id: conversation.id,
                    title: app.conversation_title(conversation),
                    format: format.into(),
                    names: conversation
                        .members
                        .iter()
                        .map(|m| (m.id, m.display_name.clone()))
                        .collect(),
                });
                ui.close();
            }
        }
    });
    if ui.button(tr("Clear my history...", "Limpar meu histórico...")).clicked() {
        app.conversation_forms.conversation = Some(conversation.id);
        app.conversation_forms.viewport = Some(ui.ctx().viewport_id());
        app.conversation_forms.clear_confirm = true;
        ui.close();
    }
}

fn history_toolbar(app: &mut MessengerApp, ui: &mut egui::Ui, conversation: &Conversation) {
    let id = conversation.id;
    let history = app.histories.entry(id).or_default();
    let mut request = None;
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin {
            left: 12,
            right: 14,
            top: 2,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut history.query_input)
                        .hint_text(tr("Search server history…", "Buscar no histórico do servidor…"))
                        .desired_width(ui.available_width() - 140.0),
                );
                if ui.button(tr("Search", "Buscar")).clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    request = Some(history.query_input.clone());
                }
                if ui.button(tr("Clear", "Limpar")).clicked() {
                    history.query_input.clear();
                    request = Some(String::new());
                }
            });
        });
    if let Some(query) = request {
        app.load_history(id, None, query);
    }
}

fn pending(app: &mut MessengerApp, ui: &mut egui::Ui, id: Uuid) {
    let failed: Vec<_> = app
        .pending
        .iter()
        .filter(|(_, p)| p.conversation == id && p.error.is_some())
        .map(|(key, p)| (*key, p.request.clone(), p.error.clone().unwrap_or_default()))
        .collect();
    for (key, request, error) in failed {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(&request.body).color(theme::MUTED));
            ui.colored_label(egui::Color32::from_rgb(160, 65, 45), error);
            ui.horizontal(|ui| {
                if ui.button(tr("Try again", "Tentar novamente")).clicked() {
                    if let Some(p) = app.pending.get_mut(&key) {
                        p.error = None;
                    }
                    app.send(Action::Send(id, request.clone()));
                }
                if ui.button(tr("Discard", "Descartar")).clicked() {
                    app.pending.remove(&key);
                }
            });
        });
    }
}

fn members(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.conversation_forms.members_open {
        return;
    }
    let Some(conversation) = app
        .conversation_forms
        .conversation
        .or(app.active)
        .and_then(|id| {
            app.data
                .as_ref()?
                .conversations
                .iter()
                .find(|c| c.id == id)
                .cloned()
        })
    else {
        return;
    };
    let mut open = app.conversation_forms.members_open;
    let mut leave = false;
    let screen = ctx.content_rect();
    let is_mobile = crate::platform::is_mobile();
    let win_w = (screen.width() - 16.0).min(430.0);
    let mut window = egui::Window::new(tr("Conversation participants", "Participantes da conversa"))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .frame(if is_mobile {
            let top_inset = crate::platform::safe_area_top();
            let pad_top = (12.0 + top_inset).round() as i8;
            let keyboard_inset = crate::platform::keyboard_height(ctx);
            let pad_bottom = if keyboard_inset > 0.0 {
                6
            } else {
                let nav = crate::platform::safe_area_bottom();
                (12.0 + nav.min(24.0)).round() as i8
            };
            egui::Frame::new()
                .fill(Color32::from_rgb(250, 253, 255))
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: pad_top,
                    bottom: pad_bottom,
                })
        } else { super::dialog::frame() })
        .open(&mut open);
    if is_mobile {
        window = window.fixed_pos(screen.min).fixed_size(screen.size());
    } else {
        window = window.default_width(win_w).max_width(win_w);
    }
    window.show(ctx, |ui| {
        leave |= super::dialog::embedded_title(ui, tr("Conversation participants", "Participantes da conversa"));
        super::scroll::vertical()
            .id_salt("group-members-scroll")
            .show(ui, |ui| {
            let me = app.data.as_ref().map(|d| d.me.id);
            if conversation.kind == ConversationKind::Group {
                egui::Frame::new()
                    .fill(Color32::from_white_alpha(240))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 244)))
                    .corner_radius(6)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(tr("Group name:", "Nome do grupo:"))
                                    .strong()
                                    .size(11.0)
                                    .color(nmessenger::HEADING),
                            );
                            let edit_width = (ui.available_width() - 85.0).max(120.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut app.conversation_forms.title)
                                    .desired_width(edit_width),
                            );
                            let rename_btn = theme::primary_button(
                                ui,
                                RichText::new(tr("Rename", "Renomear"))
                                    .size(11.0)
                                    .color(Color32::from_rgb(1, 45, 147))
                                    .strong(),
                                Vec2::new(75.0, 23.0),
                            );
                            if rename_btn.clicked() {
                                app.send(Action::RenameConversation(
                                    conversation.id,
                                    app.conversation_forms.title.clone(),
                                ));
                            }
                        });
                    });
                ui.add_space(8.0);
            }

            ui.label(
                RichText::new(format!("{} ({})", tr("Group members", "Membros no grupo"), conversation.members.len()))
                    .size(11.0)
                    .strong()
                    .color(nmessenger::HEADING),
            );
            ui.add_space(4.0);

            egui::Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, Color32::from_rgb(220, 232, 244)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    for (i, member) in conversation.members.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        ui.horizontal(|ui| {
                            theme::avatar(app, ui, member, 34.0);
                            ui.add_space(4.0);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&member.display_name)
                                            .strong()
                                            .color(nmessenger::HEADING),
                                    );
                                    if Some(member.id) == me {
                                        ui.label(
                                            RichText::new(tr("(You)", "(Você)")).small().color(nmessenger::BLUE),
                                        );
                                    }
                                });
                                ui.horizontal(|ui| {
                                    theme::status_dot(ui, member.presence);
                                    ui.label(
                                        RichText::new(nmessenger::status_label(member.presence))
                                            .small()
                                            .color(nmessenger::MUTED),
                                    );
                                });
                            });
                            if conversation.kind == ConversationKind::Group && Some(member.id) != me
                            {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if theme::danger_button(
                                            ui,
                                            RichText::new(tr("Remove", "Remover"))
                                                .color(Color32::from_rgb(180, 50, 50))
                                                .size(10.5),
                                            Vec2::new(65.0, 22.0),
                                        )
                                        .clicked()
                                        {
                                            app.send(Action::RemoveMember(
                                                conversation.id,
                                                member.id,
                                            ));
                                        }
                                    },
                                );
                            }
                        });
                    }
                });

            if conversation.kind == ConversationKind::Group {
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(Color32::from_white_alpha(240))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 244)))
                    .corner_radius(6)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(tr("Add a participant", "Adicionar novo participante"))
                                .strong()
                                .size(11.0)
                                .color(nmessenger::HEADING),
                        );
                        ui.add_space(6.0);
                        let contacts = app
                            .data
                            .as_ref()
                            .map(|d| d.contacts.clone())
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            let combo_width = (ui.available_width() - 110.0).max(140.0);
                            egui::ComboBox::from_id_salt("invite-contact")
                                .width(combo_width)
                                .selected_text(
                                    app.conversation_forms
                                        .invite
                                        .map(|u| app.name(u))
                                        .unwrap_or_else(|| tr("Choose contact...", "Escolher contato...").into()),
                                )
                                .show_ui(ui, |ui| {
                                    for contact in contacts.iter().filter(|c| {
                                        !c.blocked
                                            && c.state == ContactState::Accepted
                                            && !conversation
                                                .members
                                                .iter()
                                                .any(|m| m.id == c.user.id)
                                    }) {
                                        ui.selectable_value(
                                            &mut app.conversation_forms.invite,
                                            Some(contact.user.id),
                                            contact.name(),
                                        );
                                    }
                                });
                            let can_invite = app.conversation_forms.invite.is_some();
                            let invite_btn = if can_invite {
                                theme::primary_button(
                                    ui,
                                    RichText::new(tr("+ Invite", "+ Convidar"))
                                        .size(11.0)
                                        .color(Color32::from_rgb(1, 45, 147))
                                        .strong(),
                                    Vec2::new(95.0, 24.0),
                                )
                            } else {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(RichText::new(tr("+ Invite", "+ Convidar")).size(11.0))
                                        .min_size(Vec2::new(95.0, 24.0)),
                                )
                            };
                            if invite_btn.clicked()
                                && let Some(user) = app.conversation_forms.invite.take()
                            {
                                app.send(Action::AddMember(conversation.id, user));
                            }
                        });
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if theme::danger_button(
                        ui,
                        RichText::new(tr("Leave group", "Sair do grupo"))
                            .color(Color32::from_rgb(180, 50, 50))
                            .size(11.0),
                        Vec2::new(100.0, 24.0),
                    )
                    .clicked()
                        && let Some(me) = me
                    {
                        app.send(Action::RemoveMember(conversation.id, me));
                        leave = true;
                    }
                });
            }
            });
        });
    app.conversation_forms.members_open = open && !leave;
}

fn media_gallery_panel(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    conversation: &Conversation,
    is_mobile: bool,
) {
    let (photos, files, audios) = if let Some(history) = app.histories.get(&conversation.id) {
        let mut photos = Vec::new();
        let mut files = Vec::new();
        let mut audios = Vec::new();
        for m in &history.messages {
            let is_image = m.kind == MessageKind::Photo
                || m.kind == MessageKind::Ink
                || (m.kind == MessageKind::File
                    && m.attachment_id
                        .and_then(|id| app.file_info.get(&id))
                        .is_some_and(|info| info.mime.starts_with("image/")));
            let is_audio = m.kind == MessageKind::Voice
                || (m.kind == MessageKind::File
                    && m.attachment_id
                        .and_then(|id| app.file_info.get(&id))
                        .is_some_and(|info| info.mime.starts_with("audio/")));
            if is_image {
                photos.push(m.clone());
            } else if is_audio {
                audios.push(m.clone());
            } else if m.kind == MessageKind::File {
                files.push(m.clone());
            }
        }
        (photos, files, audios)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };

    let active_tab = app
        .histories
        .get(&conversation.id)
        .map(|h| h.media_tab)
        .unwrap_or(0);
    let earlier = app.histories.get(&conversation.id).and_then(|history| {
        if history.more && !history.loading && history.query.is_empty() { history.messages.first().map(|message| message.id) }
        else { None }
    });

    let mut navigate_to_message = None;
    let mut download_attachment = None;

    ui.vertical(|ui| {
        // Top header bar
        egui::Frame::new().fill(Color32::from_rgb(240, 248, 254)).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (icon, _) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover());
            ui.painter().rect_filled(icon, 7, Color32::from_rgb(226, 241, 253));
            nmessenger::paint(ui.ctx(), ui.painter(), "icons/framed-picture.png", icon.shrink(6.0));
            ui.add_space(4.0);
            let title_text = if is_mobile {
                tr("‹ Shared media", "‹ Mídias compartilhadas")
            } else {
                tr("Shared media", "Mídias compartilhadas")
            };
            let heading = ui.vertical(|ui| {
              let response = ui.add(
                egui::Label::new(
                    RichText::new(title_text)
                        .size(14.0)
                        .strong()
                        .color(nmessenger::HEADING),
                )
                .sense(if is_mobile {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                }),
              );
              ui.label(RichText::new(tr("Photos, files and voice messages", "Fotos, arquivos e mensagens de voz")).size(10.0).color(nmessenger::MUTED));
              response
            }).inner;
            if is_mobile && heading.clicked()
                && let Some(h) = app.histories.get_mut(&conversation.id) {
                    h.show_media_gallery = false;
                }
            if !is_mobile { ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if theme::close_button(ui, 18.0)
                    .on_hover_text(tr("Close", "Fechar"))
                    .clicked()
                    && let Some(h) = app.histories.get_mut(&conversation.id) {
                        h.show_media_gallery = false;
                    }
            }); }
        });
        });
        ui.add_space(8.0);

        // Tabs bar
        let mut new_tab = active_tab;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(12.0);
            let tab_width = ((ui.available_width() - 8.0) / 3.0).clamp(48.0, 86.0);
            for (tab_idx, label) in [
                (0, format!("{} ({})", tr("Photos", "Fotos"), photos.len())),
                (1, format!("{} ({})", tr("Files", "Arquivos"), files.len())),
                (2, format!("{} ({})", tr("Audio", "Áudios"), audios.len())),
            ] {
                let is_selected = active_tab == tab_idx;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(tab_width, 32.0), egui::Sense::click());
                if resp.hovered() || is_selected {
                    ui.painter().rect_filled(rect, 6, if is_selected { Color32::from_rgb(226, 241, 253) } else { Color32::from_rgb(244, 249, 253) });
                }
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label,
                    FontId::proportional(11.5), if is_selected { nmessenger::BLUE } else { nmessenger::MUTED });
                if resp.clicked() {
                    new_tab = tab_idx;
                }
            }
        });
        if new_tab != active_tab
            && let Some(h) = app.histories.get_mut(&conversation.id) {
                h.media_tab = new_tab;
            }
        ui.add_space(8.0);
        if let Some(before) = earlier {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                if ui.small_button(tr("Load earlier media", "Carregar mídias anteriores")).clicked() {
                    app.load_history(conversation.id, Some(before), String::new());
                }
            });
            ui.add_space(5.0);
        }

        // Content per active tab
        egui::Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| match active_tab {
                0 => {
                    if photos.is_empty() {
                        gallery_empty(ui, "icons/framed-picture.png", tr("No photos yet", "Ainda não há fotos"), tr("Photos and drawings from this conversation will appear here.", "As fotos e desenhos desta conversa aparecerão aqui."));
                    } else {
                        super::scroll::vertical()
                            .id_salt(("gallery_photos_scroll", conversation.id))
                            .show(ui, |ui| {
                                ui.add_space(4.0);
                                let col_width = 92.0;
                                let available_w = ui.available_width();
                                let cols = ((available_w / (col_width + 6.0)) as usize).max(1);
                                egui::Grid::new(("gallery_photos_grid", conversation.id))
                                    .spacing(Vec2::new(6.0, 6.0))
                                    .min_col_width(col_width)
                                    .max_col_width(col_width)
                                    .show(ui, |ui| {
                                        for (idx, msg) in photos.iter().enumerate() {
                                            if idx > 0 && idx % cols == 0 {
                                                ui.end_row();
                                            }
                                            let msg_id = msg.id;
                                            let att_id = msg.attachment_id;
                                            let sender = app.name(msg.sender_id);
                                            let date_str =
                                                msg.created_at.format("%d/%m/%Y %H:%M").to_string();

                                            let (rect, response) = ui.allocate_exact_size(
                                                Vec2::splat(col_width),
                                                egui::Sense::click(),
                                            );

                                            ui.painter().rect_filled(
                                                rect,
                                                4,
                                                Color32::from_rgb(235, 242, 250),
                                            );

                                            if let Some(id) = att_id {
                                                if let Some((texture_id, size)) = app
                                                    .texture(ui.ctx(), ImageSource::File(id))
                                                {
                                                    let scale = (col_width / size.x)
                                                        .max(col_width / size.y);
                                                    let draw_size = size * scale;
                                                    let draw_rect = Rect::from_center_size(
                                                        rect.center(),
                                                        draw_size,
                                                    );
                                                    let painter = ui.painter().with_clip_rect(rect);
                                                    let uv = Rect::from_min_max(
                                                        Pos2::new(0.0, 0.0),
                                                        Pos2::new(1.0, 1.0),
                                                    );
                                                    let mut mesh =
                                                        egui::Mesh::with_texture(texture_id);
                                                    mesh.add_rect_with_uv(
                                                        draw_rect,
                                                        uv,
                                                        Color32::WHITE,
                                                    );
                                                    painter.add(mesh);
                                                } else {
                                                    ui.painter().text(
                                                        rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        tr("Photo", "Foto"),
                                                        FontId::proportional(11.0),
                                                        nmessenger::MUTED,
                                                    );
                                                }
                                            }

                                            let stroke_color = if response.hovered() {
                                                Color32::from_rgb(70, 145, 230)
                                            } else {
                                                Color32::from_rgb(205, 220, 235)
                                            };
                                            ui.painter().rect_stroke(
                                                rect,
                                                4,
                                                Stroke::new(
                                                    if response.hovered() { 1.5 } else { 0.5 },
                                                    stroke_color,
                                                ),
                                                egui::StrokeKind::Inside,
                                            );

                                            response.clone().on_hover_ui(|ui| {
                                                ui.label(RichText::new(&sender).strong());
                                                ui.label(
                                                    RichText::new(&date_str)
                                                        .small()
                                                        .color(nmessenger::MUTED),
                                                );
                                                ui.label(
                                                    RichText::new(
                                                        tr("Right-click for options", "Clique com botão direito para opções"),
                                                    )
                                                    .small()
                                                    .color(Color32::from_rgb(1, 45, 147)),
                                                );
                                            });

                                            response.context_menu(|ui| {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{}: {}", tr("Sent by", "Enviado por"),
                                                        sender
                                                    ))
                                                    .small()
                                                    .color(nmessenger::MUTED),
                                                );
                                                ui.separator();
                                                if ui.button(tr("Go to message", "Ir para mensagem")).clicked() {
                                                    navigate_to_message = Some(msg_id);
                                                    ui.close();
                                                }
                                                if let Some(id) = att_id
                                                    && ui.button(tr("Download photo", "Baixar foto")).clicked() {
                                                        download_attachment = Some(id);
                                                        ui.close();
                                                    }
                                            });

                                            if response.clicked() {
                                                if let Some(id) = att_id { app.image_viewer = Some(id); }
                                                else { navigate_to_message = Some(msg_id); }
                                            }
                                        }
                                    });
                            });
                    }
                }
                1 => {
                    if files.is_empty() {
                        gallery_empty(ui, "icons/folder.png", tr("No files yet", "Ainda não há arquivos"), tr("Shared documents and files will appear here.", "Os arquivos compartilhados aparecerão aqui."));
                    } else {
                        super::scroll::vertical()
                            .id_salt(("gallery_files_scroll", conversation.id))
                            .show(ui, |ui| {
                                for msg in &files {
                                    let msg_id = msg.id;
                                    let att_id = msg.attachment_id;
                                    let sender = app.name(msg.sender_id);
                                    let date_str =
                                        msg.created_at.format("%d/%m/%Y %H:%M").to_string();
                                    let info = att_id.and_then(|id| app.file_info.get(&id));
                                    let filename = info.map(|i| i.filename.clone()).unwrap_or_else(
                                        || {
                                            if !msg.body.trim().is_empty() {
                                                msg.body.clone()
                                            } else {
                                                tr("File", "Arquivo").to_string()
                                            }
                                        },
                                    );
                                    let size_str = info
                                        .map(|i| {
                                            let bytes = i.size;
                                            if bytes < 1024 {
                                                format!("{bytes} B")
                                            } else if bytes < 1024 * 1024 {
                                                format!("{:.1} KiB", bytes as f64 / 1024.0)
                                            } else {
                                                format!(
                                                    "{:.1} MiB",
                                                    bytes as f64 / (1024.0 * 1024.0)
                                                )
                                            }
                                        })
                                        .unwrap_or_else(|| tr("File", "Arquivo").to_string());

                                    if let Some(id) = att_id
                                        && info.is_none() {
                                            app.send(Action::FileInfo(id));
                                        }

                                    let item_frame = egui::Frame::new()
                                        .fill(Color32::from_rgb(255, 255, 255))
                                        .stroke(Stroke::new(0.5, Color32::from_rgb(215, 225, 235)))
                                        .corner_radius(6)
                                        .inner_margin(8);

                                    let item_resp = item_frame
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                let (icon_rect, _) = ui.allocate_exact_size(
                                                    Vec2::splat(32.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().rect_filled(
                                                    icon_rect,
                                                    4,
                                                    Color32::from_rgb(220, 235, 250),
                                                );
                                                ui.painter().text(
                                                    icon_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "▤",
                                                    FontId::proportional(16.0),
                                                    Color32::from_rgb(1, 45, 147),
                                                );

                                                ui.add_space(4.0);
                                                ui.vertical(|ui| {
                                                    ui.set_width(
                                                        (ui.available_width() - 48.0).max(60.0),
                                                    );
                                                    ui.label(
                                                        RichText::new(&filename)
                                                            .size(11.5)
                                                            .strong()
                                                            .color(nmessenger::INK),
                                                    )
                                                    .on_hover_text(&filename);
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            RichText::new(&size_str)
                                                                .size(10.0)
                                                                .color(nmessenger::MUTED),
                                                        );
                                                        ui.label(
                                                            RichText::new("·")
                                                                .size(10.0)
                                                                .color(nmessenger::MUTED),
                                                        );
                                                        ui.label(
                                                            RichText::new(&sender)
                                                                .size(10.0)
                                                                .color(nmessenger::MUTED),
                                                        );
                                                    });
                                                    ui.label(
                                                        RichText::new(&date_str)
                                                            .size(9.0)
                                                            .color(nmessenger::MUTED),
                                                    );
                                                });

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if let Some(id) = att_id
                                                            && ui
                                                                .button("⬇")
                                                                .on_hover_text(tr("Download file", "Baixar arquivo"))
                                                                .clicked()
                                                            {
                                                                download_attachment = Some(id);
                                                            }
                                                    },
                                                );
                                            });
                                        })
                                        .response;

                                    item_resp.clone().on_hover_ui(|ui| {
                                        ui.label(
                                            RichText::new(
                                                tr("Right-click for options or click to navigate", "Clique com botão direito para opções ou clique para navegar"),
                                            )
                                            .small()
                                            .color(Color32::from_rgb(1, 45, 147)),
                                        );
                                    });

                                    item_resp.context_menu(|ui| {
                                        ui.label(RichText::new(&filename).strong());
                                        ui.separator();
                                        if ui.button(tr("Go to message", "Ir para mensagem")).clicked() {
                                            navigate_to_message = Some(msg_id);
                                            ui.close();
                                        }
                                        if let Some(id) = att_id
                                            && ui.button(tr("Download file", "Baixar arquivo")).clicked() {
                                                download_attachment = Some(id);
                                                ui.close();
                                            }
                                    });

                                    if item_resp.clicked() {
                                        navigate_to_message = Some(msg_id);
                                    }
                                    ui.add_space(4.0);
                                }
                            });
                    }
                }
                _ => {
                    if audios.is_empty() {
                        gallery_empty(ui, "icons/mic.png", tr("No audio yet", "Ainda não há áudios"), tr("Voice messages and audio will appear here.", "As mensagens de voz e áudios aparecerão aqui."));
                    } else {
                        super::scroll::vertical()
                            .id_salt(("gallery_audios_scroll", conversation.id))
                            .show(ui, |ui| {
                                for msg in &audios {
                                    let msg_id = msg.id;
                                    let att_id = msg.attachment_id;
                                    let sender = app.name(msg.sender_id);
                                    let date_str =
                                        msg.created_at.format("%d/%m/%Y %H:%M").to_string();
                                    let info = att_id.and_then(|id| app.file_info.get(&id));

                                    if let Some(id) = att_id
                                        && info.is_none() {
                                            app.send(Action::FileInfo(id));
                                        }

                                    let item_frame = egui::Frame::new()
                                        .fill(Color32::from_rgb(255, 255, 255))
                                        .stroke(Stroke::new(0.5, Color32::from_rgb(215, 225, 235)))
                                        .corner_radius(6)
                                        .inner_margin(8);

                                    let item_resp = item_frame
                                        .show(ui, |ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    let (icon_rect, _) = ui.allocate_exact_size(
                                                        Vec2::splat(26.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    ui.painter().rect_filled(
                                                        icon_rect,
                                                        4,
                                                        Color32::from_rgb(230, 245, 230),
                                                    );
                                                    ui.painter().text(
                                                        icon_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "♫",
                                                        FontId::proportional(15.0),
                                                        Color32::from_rgb(40, 140, 40),
                                                    );
                                                    ui.add_space(4.0);
                                                    ui.vertical(|ui| {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "{} {}", tr("Audio from", "Áudio de"),
                                                                sender
                                                            ))
                                                            .size(11.5)
                                                            .strong()
                                                            .color(nmessenger::INK),
                                                        );
                                                        ui.label(
                                                            RichText::new(&date_str)
                                                                .size(9.5)
                                                                .color(nmessenger::MUTED),
                                                        );
                                                    });
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if let Some(id) = att_id
                                                                && ui
                                                                    .button("⬇")
                                                                    .on_hover_text(tr("Download audio", "Baixar áudio"))
                                                                    .clicked()
                                                                {
                                                                    download_attachment = Some(id);
                                                                }
                                                        },
                                                    );
                                                });
                                                if let Some(file_id) = att_id {
                                                    ui.add_space(4.0);
                                                    super::audio_player::show(app, ui, file_id);
                                                }
                                            });
                                        })
                                        .response;

                                    item_resp.clone().on_hover_ui(|ui| {
                                        ui.label(
                                            RichText::new(
                                                tr("Right-click for options or click to navigate", "Clique com botão direito para opções ou clique para navegar"),
                                            )
                                            .small()
                                            .color(Color32::from_rgb(1, 45, 147)),
                                        );
                                    });

                                    item_resp.context_menu(|ui| {
                                        ui.label(
                                            RichText::new(format!("{} {}", tr("Audio from", "Áudio de"), sender)).strong(),
                                        );
                                        ui.separator();
                                        if ui.button(tr("Go to message", "Ir para mensagem")).clicked() {
                                            navigate_to_message = Some(msg_id);
                                            ui.close();
                                        }
                                        if let Some(id) = att_id
                                            && ui.button(tr("Download audio", "Baixar áudio")).clicked() {
                                                download_attachment = Some(id);
                                                ui.close();
                                            }
                                    });

                                    if item_resp.clicked() {
                                        navigate_to_message = Some(msg_id);
                                    }
                                    ui.add_space(4.0);
                                }
                            });
                    }
                }
            });
    });

    if let Some(msg_id) = navigate_to_message
        && let Some(h) = app.histories.get_mut(&conversation.id) {
            h.scroll_to_message = Some(msg_id);
            if is_mobile {
                h.show_media_gallery = false;
            }
        }
    if let Some(att_id) = download_attachment {
        app.send(Action::Download(att_id));
    }
}

fn gallery_empty(ui: &mut egui::Ui, icon: &str, title: &str, description: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(58.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 29.0, Color32::from_rgb(230, 243, 253));
        nmessenger::paint(ui.ctx(), ui.painter(), icon, rect.shrink(15.0));
        ui.add_space(10.0);
        ui.label(RichText::new(title).strong().size(12.0).color(nmessenger::HEADING));
        ui.label(RichText::new(description).small().color(nmessenger::MUTED));
    });
}

fn clear_history(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.conversation_forms.clear_confirm {
        return;
    }
    if crate::platform::is_mobile() {
        app.conversation_forms.clear_confirm = super::dialog::mobile_page(
            ctx, "mobile-clear-history", tr("Conversation history", "Histórico da conversa"), |ui| {
                ui.label(tr("Hide this conversation's history for your account?", "Ocultar o histórico desta conversa para sua conta?"));
                if ui.button(tr("Clear my history", "Limpar meu histórico")).clicked()
                    && let Some(id) = app.conversation_forms.conversation.or(app.active) {
                    app.send(Action::ClearHistory(id));
                    return true;
                }
                false
            },
        );
        return;
    }
    let mut open = app.conversation_forms.clear_confirm;
    let mut confirmed = false;
    let screen = ctx.content_rect();
    let win_w = (screen.width() - 16.0).min(360.0);
    egui::Window::new(tr("Clear history", "Limpar histórico"))
        .title_bar(false)
        .collapsible(false)
        .frame(super::dialog::frame())
        .open(&mut open)
        .resizable(false)
        .default_width(win_w)
        .max_width(win_w)
        .show(ctx, |ui| {
            confirmed |= super::dialog::embedded_title(ui, tr("Conversation history", "Histórico da conversa"));
            ui.label(tr("Hide this conversation's history for your account?", "Ocultar o histórico desta conversa para sua conta?"));
            if ui.button(tr("Clear my history", "Limpar meu histórico")).clicked()
                && let Some(id) = app.conversation_forms.conversation.or(app.active)
            {
                app.send(Action::ClearHistory(id));
                confirmed = true;
            }
        });
    app.conversation_forms.clear_confirm = open && !confirmed;
}
