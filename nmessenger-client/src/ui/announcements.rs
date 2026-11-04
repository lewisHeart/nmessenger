use super::{dialog, nmessenger, theme};
use crate::{app::MessengerApp, media::images::ImageSource, transport::Action};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Vec2};
use nm_protocol::Announcement;

pub fn banner(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let items: Vec<_> = app
        .announcements
        .feed
        .items
        .iter()
        .filter(|item| item.enabled)
        .cloned()
        .collect();
    if items.is_empty() {
        return;
    }
    let now = std::time::Instant::now();
    let start = *app
        .announcements
        .rotation
        .get_or_insert(now);
    if start.elapsed() >= std::time::Duration::from_secs(12) {
        app.announcements.current_index = (app.announcements.current_index + 1) % items.len();
        app.announcements.rotation = Some(now);
    }
    if app.announcements.current_index >= items.len() {
        app.announcements.current_index = 0;
    }
    let current_index = app.announcements.current_index;
    let total_count = items.len();
    let item = &items[current_index];
    egui::Panel::bottom("announcement-banner")
        .exact_size(68.0)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::ZERO))
        .show(ui, |ui| {
            render_banner_content(app, ui, item, true, false, current_index, total_count);
        });
}

fn render_banner_content(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    item: &Announcement,
    clickable: bool,
    is_dialog_preview: bool,
    current_index: usize,
    total_count: usize,
) {
    let height = 68.0_f32;
    let size = Vec2::new(ui.available_width(), height);
    let (rect, _) = ui.allocate_exact_size(
        size,
        if clickable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let bottom_radius = if is_dialog_preview || crate::platform::is_mobile() || super::window::system_titlebars(ui.ctx()) {
        0
    } else {
        11
    };
    let banner_corners = egui::CornerRadius {
        nw: if is_dialog_preview { 4 } else { 0 },
        ne: if is_dialog_preview { 4 } else { 0 },
        sw: if is_dialog_preview { 4 } else { bottom_radius },
        se: if is_dialog_preview { 4 } else { bottom_radius },
    };

    let nav_top = rect.top() + 7.0;
    let next_btn_rect = Rect::from_min_size(Pos2::new(rect.right() - 25.0, nav_top), Vec2::splat(18.0));
    let prev_btn_rect = Rect::from_min_size(Pos2::new(rect.right() - 46.0, nav_top), Vec2::splat(18.0));
    let in_nav_zone = total_count > 1
        && !is_dialog_preview
        && ui.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                p.x >= prev_btn_rect.left() - 40.0 && p.y <= rect.top() + 32.0
            })
        });

    let is_hovered = clickable && !is_dialog_preview && ui.rect_contains_pointer(rect) && !in_nav_zone;
    let bg_color = if is_hovered {
        Color32::from_rgb(238, 246, 255)
    } else {
        Color32::from_rgb(245, 250, 255)
    };
    ui.painter()
        .rect_filled(rect, banner_corners, bg_color);

    if item.full_banner {
        if let Some(file) = item.image_file_id
            && let Some((texture, source_size)) = app.texture(ui.ctx(), ImageSource::File(file))
        {
            let uv = nmessenger::cover_uv(source_size, rect.size());
            ui.put(
                rect,
                egui::Image::new((texture, rect.size()))
                    .uv(uv)
                    .corner_radius(banner_corners),
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if item.title.is_empty() {
                    "Banner todo (sem imagem)"
                } else {
                    &item.title
                },
                FontId::proportional(12.0),
                nmessenger::HEADING,
            );
        }
    } else {
        let content_rect = rect.shrink2(Vec2::new(10.0, 5.0));
        let mut x_offset = content_rect.left();

        if let Some(file) = item.image_file_id
            && let Some((texture, source_size)) = app.texture(ui.ctx(), ImageSource::File(file))
        {
            let thumb_size = Vec2::new(72.0, 56.0);
            let thumb_rect = Rect::from_min_size(
                Pos2::new(x_offset, content_rect.center().y - thumb_size.y * 0.5),
                thumb_size,
            );
            let uv = nmessenger::cover_uv(source_size, thumb_size);
            ui.painter().image(texture, thumb_rect, uv, Color32::WHITE);
            ui.painter().rect_stroke(
                thumb_rect,
                3,
                egui::Stroke::new(1.0, Color32::from_rgb(215, 229, 241)),
                egui::StrokeKind::Inside,
            );
            x_offset += thumb_size.x + 10.0;
        }

        let nav_reserve = if total_count > 1 && !is_dialog_preview { 72.0 } else { 0.0 };
        let text_width = (content_rect.right() - x_offset - nav_reserve).max(10.0);
        let top_y = content_rect.top() + 3.0;

        ui.painter().text(
            Pos2::new(x_offset, top_y),
            egui::Align2::LEFT_TOP,
            "ANÚNCIO",
            FontId::proportional(8.0),
            Color32::from_rgb(120, 143, 163),
        );

        let title_font = FontId::new(11.0, egui::FontFamily::Name("Segoe UI Bold".into()));
        nmessenger::left(
            ui,
            Rect::from_min_size(
                Pos2::new(x_offset, top_y + 12.0),
                Vec2::new(text_width, 15.0),
            ),
            egui::Label::new(
                RichText::new(&item.title)
                    .font(title_font)
                    .color(nmessenger::HEADING),
            )
            .truncate(),
        );

        nmessenger::left(
            ui,
            Rect::from_min_size(
                Pos2::new(x_offset, top_y + 27.0),
                Vec2::new(text_width, 14.0),
            ),
            egui::Label::new(
                RichText::new(&item.body)
                    .size(9.5)
                    .color(Color32::from_rgb(95, 119, 140)),
            )
            .truncate(),
        );

        let link_text = if item.link_url.is_some() {
            "Saiba mais ›"
        } else {
            "Ver mais ›"
        };
        let link_color = if item.link_url.is_some() {
            nmessenger::BLUE
        } else {
            Color32::from_rgb(105, 138, 168)
        };
        ui.painter().text(
            Pos2::new(x_offset, top_y + 42.0),
            egui::Align2::LEFT_TOP,
            link_text,
            FontId::proportional(9.0),
            link_color,
        );
    }

    if clickable && !is_dialog_preview {
        let in_resize_zone = ui.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                p.y >= rect.bottom() - 8.0
                    || p.x <= rect.left() + 8.0
                    || p.x >= rect.right() - 8.0
            })
        });
        if !in_resize_zone && !in_nav_zone {
            let response = ui
                .interact(rect, ui.scope_id().with("banner-view-modal"), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(crate::i18n::tr("Click to view full announcement", "Clique para ver o anúncio completo"));
            if response.clicked() {
                app.announcements.viewing = Some(item.clone());
            }
        }
    }

    if total_count > 1 && !is_dialog_preview {
        let prev_res = ui
            .interact(
                prev_btn_rect,
                ui.scope_id().with("banner-ad-prev"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Anúncio anterior");
        let next_res = ui
            .interact(
                next_btn_rect,
                ui.scope_id().with("banner-ad-next"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Próximo anúncio");

        if prev_res.clicked() {
            app.announcements.current_index = (current_index + total_count - 1) % total_count;
            app.announcements.rotation = Some(std::time::Instant::now());
        }
        if next_res.clicked() {
            app.announcements.current_index = (current_index + 1) % total_count;
            app.announcements.rotation = Some(std::time::Instant::now());
        }

        ui.painter().text(
            Pos2::new(prev_btn_rect.left() - 4.0, nav_top + 9.0),
            egui::Align2::RIGHT_CENTER,
            format!("{}/{}", current_index + 1, total_count),
            FontId::proportional(8.5),
            Color32::from_rgb(120, 145, 168),
        );

        let draw_btn = |ui: &mut egui::Ui, btn_rect: Rect, label: &str, res: &egui::Response| {
            let bg = if res.is_pointer_button_down_on() {
                Color32::from_rgba_unmultiplied(200, 226, 250, 240)
            } else if res.hovered() {
                Color32::from_rgba_unmultiplied(235, 246, 255, 230)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 190)
            };
            let border_color = if res.hovered() {
                Color32::from_rgb(135, 190, 240)
            } else {
                Color32::from_rgb(210, 225, 240)
            };
            ui.painter().rect_filled(btn_rect, 4, bg);
            ui.painter().rect_stroke(
                btn_rect,
                4,
                egui::Stroke::new(0.8, border_color),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                if res.hovered() {
                    Color32::from_rgb(1, 45, 147)
                } else {
                    Color32::from_rgb(70, 95, 120)
                },
            );
        };

        draw_btn(ui, prev_btn_rect, "‹", &prev_res);
        draw_btn(ui, next_btn_rect, "›", &next_res);
    }

    if is_dialog_preview {
        ui.painter().rect_stroke(
            rect,
            banner_corners,
            egui::Stroke::new(1.0, Color32::from_rgb(215, 229, 241)),
            egui::StrokeKind::Inside,
        );
    } else {
        if bottom_radius > 0 {
            ui.painter().rect_stroke(
                rect,
                banner_corners,
                egui::Stroke::new(1.0_f32, Color32::from_black_alpha(48)),
                egui::StrokeKind::Inside,
            );
        }
        ui.painter().line_segment(
            [rect.left_top(), rect.right_top()],
            egui::Stroke::new(1.0_f32, Color32::from_rgb(215, 229, 241)),
        );
    }
}

pub fn modal(app: &mut MessengerApp, ctx: &egui::Context) {
    let Some(item) = app.announcements.viewing.clone() else {
        return;
    };

    let items: Vec<_> = app
        .announcements
        .feed
        .items
        .iter()
        .filter(|item| item.enabled)
        .cloned()
        .collect();
    let current_pos = items.iter().position(|it| it.id == item.id);

    let screen = ctx.content_rect();
    let modal_width = (screen.width() - 32.0).min(340.0).max(260.0);
    let modal = egui::Modal::new(egui::Id::unique("announcement-detail-modal"))
        .backdrop_color(Color32::from_black_alpha(130))
        .frame(
            egui::Frame::new()
                .fill(Color32::from_rgb(252, 254, 255))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(180, 210, 238)))
                .corner_radius(10)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 8],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(65),
                })
                .inner_margin(egui::Margin::symmetric(16, 14)),
        );

    let mut close = false;
    let mut next_item = None;

    let response = modal.show(ctx, |ui| {
        ui.set_width(modal_width);

        // Header: Category, pagination if multiple, and close button
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("📢 ANÚNCIO")
                    .size(9.5)
                    .color(Color32::from_rgb(115, 140, 165))
                    .strong(),
            );
            if let Some(idx) = current_pos
                && items.len() > 1
            {
                ui.add_space(4.0);
                if ui.button("‹").on_hover_text(crate::i18n::tr("Previous", "Anterior")).clicked() {
                    let prev_idx = (idx + items.len() - 1) % items.len();
                    next_item = Some((items[prev_idx].clone(), prev_idx));
                }
                ui.label(
                    RichText::new(format!("{}/{}", idx + 1, items.len()))
                        .size(9.5)
                        .color(Color32::from_rgb(120, 145, 168)),
                );
                if ui.button("›").on_hover_text(crate::i18n::tr("Next", "Próximo")).clicked() {
                    let next_idx = (idx + 1) % items.len();
                    next_item = Some((items[next_idx].clone(), next_idx));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if super::theme::close_button(ui, 18.0)
                    .on_hover_text(crate::i18n::tr("Close", "Fechar"))
                    .clicked()
                {
                    close = true;
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(8.0);

        // Image (if any)
        if let Some(file) = item.image_file_id
            && let Some((texture, source_size)) = app.texture(ui.ctx(), ImageSource::File(file))
        {
            let max_w = ui.available_width();
            let max_h = 160.0_f32;
            let ratio = (max_w / source_size.x.max(1.0))
                .min(max_h / source_size.y.max(1.0))
                .min(1.0);
            let img_size = Vec2::new(
                (source_size.x * ratio).round().max(10.0),
                (source_size.y * ratio).round().max(10.0),
            );
            ui.vertical_centered(|ui| {
                let (img_rect, _) = ui.allocate_exact_size(img_size, egui::Sense::hover());
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                ui.painter().image(texture, img_rect, uv, Color32::WHITE);
                ui.painter().rect_stroke(
                    img_rect,
                    6,
                    egui::Stroke::new(1.0, Color32::from_rgb(215, 229, 241)),
                    egui::StrokeKind::Inside,
                );
            });
            ui.add_space(10.0);
        }

        // Title (wrapped, bold, full)
        if !item.title.is_empty() {
            let title_font = FontId::new(13.5, egui::FontFamily::Name("Segoe UI Bold".into()));
            ui.add(
                egui::Label::new(
                    RichText::new(&item.title)
                        .font(title_font)
                        .color(nmessenger::HEADING),
                )
                .wrap(),
            );
            ui.add_space(6.0);
        }

        // Body (wrapped, full text, scrollable if long)
        if !item.body.is_empty() {
            let body_max_h = (screen.height() - 280.0).clamp(90.0, 220.0);
            super::scroll::vertical()
                .id_salt("announcement-modal-body-scroll")
                .max_height(body_max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&item.body)
                                .size(11.5)
                                .color(Color32::from_rgb(60, 85, 110)),
                        )
                        .wrap(),
                    );
                });
        }

        // Action Buttons
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if let Some(url) = &item.link_url {
                let link_btn = theme::primary_button(
                    ui,
                    RichText::new("Acessar link ↗")
                        .color(Color32::from_rgb(1, 45, 147))
                        .size(11.0)
                        .strong(),
                    Vec2::new(115.0, 28.0),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(url);
                if link_btn.clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [78.0, 28.0],
                        egui::Button::new(RichText::new(crate::i18n::tr("Close", "Fechar")).size(11.0)),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        });
    });

    if let Some((it, idx)) = next_item {
        app.announcements.viewing = Some(it);
        app.announcements.current_index = idx;
        app.announcements.rotation = Some(std::time::Instant::now());
    } else if response.should_close() || close {
        app.announcements.viewing = None;
    }
}

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.announcements.open || !app.announcements.feed.can_manage {
        return;
    }
    if !dialog::show(
        ctx,
        "announcement-manager",
        "Anúncios - NMessenger",
        [780.0, 620.0],
        |ui| {
            content(app, ui);
            false
        },
    ) {
        app.announcements.open = false;
    }
}

pub(crate) fn content(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let is_mobile = crate::platform::is_mobile() || ui.available_width() < 560.0;
    egui::Panel::top("announcements-heading")
        .exact_size(54.0)
        .show_separator_line(false)
        .frame(egui::Frame::new())
        .show(ui, |ui| {
            ui.label(
                RichText::new("Anúncios")
                    .size(if is_mobile { 18.0 } else { 22.0 })
                    .color(nmessenger::HEADING),
            );
            ui.label(
                RichText::new("Crie, edite e publique os banners exibidos na lista de contatos.")
                    .size(10.5)
                    .color(nmessenger::HEADING),
            );
        });
    if is_mobile {
        ui.horizontal(|ui| {
            let new_btn = if !app.announcements.busy {
                theme::primary_button(
                    ui,
                    RichText::new("+ Novo")
                        .color(Color32::from_rgb(1, 45, 147))
                        .size(11.0)
                        .strong(),
                    Vec2::new(75.0, 30.0),
                )
            } else {
                ui.add_enabled(false, egui::Button::new("+ Novo").min_size(Vec2::new(75.0, 30.0)))
            };
            if new_btn.clicked() {
                app.announcements.edit(None);
            }
            let items = app.announcements.feed.items.clone();
            super::scroll::horizontal()
                .id_salt("mobile-announcement-items")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for item in items {
                            let is_sel = app.announcements.selected == Some(item.id);
                            if ui
                                .add_enabled(
                                    !app.announcements.busy,
                                    egui::Button::new(RichText::new(&item.title).size(11.0))
                                        .selected(is_sel)
                                        .min_size(Vec2::new(80.0, 30.0)),
                                )
                                .clicked()
                            {
                                app.announcements.edit(Some(&item));
                            }
                        }
                    });
                });
        });
        ui.add_space(8.0);
    } else {
        egui::Panel::left("announcements-list")
            .exact_size(200.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: 0,
                right: 14,
                top: 4,
                bottom: 0,
            }))
            .show(ui, |ui| {
                let new_btn = if !app.announcements.busy {
                    theme::primary_button(
                        ui,
                        RichText::new("+ Novo anúncio")
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                        Vec2::new(178.0, 32.0),
                    )
                } else {
                    ui.add_enabled(
                        false,
                        egui::Button::new("+ Novo anúncio").min_size(Vec2::new(178.0, 32.0)),
                    )
                };
                if new_btn.clicked() {
                    app.announcements.edit(None);
                }
                ui.add_space(12.0);
                let items = app.announcements.feed.items.clone();
                super::scroll::vertical()
                    .id_salt("announcement-items")
                    .show(ui, |ui| {
                        for item in items {
                            let label = format!(
                                "{}\n{}",
                                item.title,
                                if item.enabled { "Publicado" } else { "Pausado" }
                            );
                            if ui
                                .add_enabled(
                                    !app.announcements.busy,
                                    egui::Button::new(RichText::new(label).size(11.0))
                                        .selected(app.announcements.selected == Some(item.id))
                                        .min_size(Vec2::new(ui.available_width(), 47.0))
                                        .wrap(),
                                )
                                .clicked()
                            {
                                app.announcements.edit(Some(&item));
                            }
                            ui.add_space(5.0);
                        }
                    });
            });
    }
    super::scroll::vertical()
        .id_salt("announcement-editor")
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(error) = &app.announcements.error {
                ui.colored_label(Color32::from_rgb(180, 50, 50), error);
            }
            if !app.announcements.notice.is_empty() {
                ui.colored_label(Color32::from_rgb(55, 135, 80), &app.announcements.notice);
            }
            ui.add_enabled_ui(!app.announcements.busy, |ui| {
                ui.label("Título");
                ui.add(
                    egui::TextEdit::singleline(&mut app.announcements.draft.title)
                        .desired_width(f32::INFINITY)
                        .char_limit(120),
                );
                ui.add_space(7.0);
                ui.label("Texto");
                ui.add(
                    egui::TextEdit::multiline(&mut app.announcements.draft.body)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .char_limit(2000),
                );
                ui.add_space(7.0);
                ui.label("Link (opcional)");
                ui.add(
                    egui::TextEdit::singleline(&mut app.announcements.link)
                        .hint_text("https://...")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Escolher imagem...").clicked() {
                        app.announcements.busy = true;
                        app.send(Action::UploadAnnouncementImage);
                    }
                    if app.announcements.draft.image_file_id.is_some()
                        && ui.button("Remover imagem").clicked()
                    {
                        app.announcements.draft.image_file_id = None;
                    }
                });
                ui.add_space(6.0);
                ui.checkbox(
                    &mut app.announcements.draft.full_banner,
                    "Banner todo (imagem cobre todo o banner)",
                );
                ui.add_space(4.0);
                ui.checkbox(
                    &mut app.announcements.draft.enabled,
                    "Publicado · visível para todos",
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if theme::primary(ui, "Salvar anúncio").clicked() {
                        if app.announcements.draft.title.trim().is_empty() {
                            app.announcements.error = Some("Informe o título do anúncio.".into());
                        } else {
                            app.announcements.draft.link_url =
                                (!app.announcements.link.trim().is_empty())
                                    .then(|| app.announcements.link.trim().into());
                            app.announcements.error = None;
                            app.announcements.notice.clear();
                            app.announcements.busy = true;
                            app.send(Action::SaveAnnouncement(
                                app.announcements.selected,
                                app.announcements.draft.clone(),
                            ));
                        }
                    }
                    if let Some(id) = app.announcements.selected
                        && theme::danger_button(
                            ui,
                            RichText::new("Excluir").color(Color32::from_rgb(180, 50, 50)),
                            Vec2::new(70.0, 26.0),
                        )
                        .clicked()
                    {
                        app.announcements.busy = true;
                        app.send(Action::DeleteAnnouncement(id));
                    }
                });
            });
            if app.announcements.busy {
                ui.spinner();
            }
            ui.add_space(16.0);
            ui.label(
                RichText::new("PRÉVIA DO BANNER")
                    .size(9.0)
                    .color(nmessenger::HEADING),
            );
            let now = chrono::Utc::now();
            let draft = app.announcements.draft.clone();
            render_banner_content(
                app,
                ui,
                &Announcement {
                    id: app.announcements.selected.unwrap_or_default(),
                    title: draft.title,
                    body: draft.body,
                    link_url: (!app.announcements.link.is_empty())
                        .then(|| app.announcements.link.clone()),
                    image_file_id: draft.image_file_id,
                    full_banner: draft.full_banner,
                    enabled: draft.enabled,
                    created_at: now,
                    updated_at: now,
                },
                false,
                true,
                0,
                1,
            );
        });
}
