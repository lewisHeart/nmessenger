use super::{dialog, nmessenger};
use crate::i18n::tr;
use crate::{
    app::{AvatarChoice, MessengerApp},
    media::images::ImageSource,
    transport::Action,
};
use eframe::egui::{self, Color32, Rect, RichText, Vec2};
use nm_protocol::AssetKind;

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.options.avatar_picker {
        return;
    }
    let open = dialog::show(
        ctx,
        "nmessenger-avatar-picker",
        tr("Profile picture", "Imagem de perfil"),
        [640.0, 480.0],
        |ui| content(app, ui),
    );
    app.options.avatar_picker = open && app.options.avatar_picker;
    if !open {
        app.options.avatar_choice = None;
        app.options.avatar_error = None;
    }
}

fn content(app: &mut MessengerApp, ui: &mut egui::Ui) -> bool {
    let mut close = false;
    let is_mobile = crate::platform::is_mobile() || ui.available_width() < 500.0;
    let panel_h = if is_mobile { 52.0 } else { 48.0 };
    let btn_h = if is_mobile { 34.0 } else { 29.0 };
    egui::Panel::bottom("avatar-picker-actions")
        .exact_size(panel_h)
        .show_separator_line(false)
        .frame(egui::Frame::new().inner_margin(5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !app.options.avatar_saving,
                        egui::Button::new(if is_mobile {
                            tr("Choose file...", "Escolher arquivo...")
                        } else {
                            tr("Choose from computer...", "Escolher do computador...")
                        })
                        .min_size(Vec2::new(0.0, btn_h)),
                    )
                    .clicked()
                {
                    app.send(Action::PickAvatarFile);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_apply =
                        !app.options.avatar_saving && app.options.avatar_choice.is_some();
                    let btn_text = if app.options.avatar_saving {
                        tr("Applying...", "Aplicando...")
                    } else {
                        tr("Use this picture", "Usar esta imagem")
                    };
                    let apply_btn = if can_apply {
                        super::theme::primary_button(
                            ui,
                            RichText::new(btn_text)
                                .color(Color32::from_rgb(1, 45, 147))
                                .size(if is_mobile { 12.0 } else { 11.0 })
                                .strong(),
                            Vec2::new(if is_mobile { 130.0 } else { 118.0 }, btn_h),
                        )
                    } else {
                        ui.add_enabled(
                            false,
                            egui::Button::new(RichText::new(btn_text))
                                .min_size(Vec2::new(if is_mobile { 130.0 } else { 118.0 }, btn_h)),
                        )
                    };
                    if apply_btn.clicked() {
                        apply(app);
                    }
                    if !is_mobile && ui
                        .add_enabled(
                            !app.options.avatar_saving,
                            egui::Button::new(tr("Cancel", "Cancelar")).min_size(Vec2::new(if is_mobile { 88.0 } else { 82.0 }, btn_h)),
                        )
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        });
    ui.label(
        RichText::new(tr("Choose your profile picture", "Escolha sua imagem de perfil"))
            .size(if is_mobile { 16.0 } else { 18.0 })
            .color(nmessenger::HEADING),
    );
    ui.label(
        RichText::new(tr("Select a picture and preview it before applying.", "Selecione uma imagem e confira a prévia antes de aplicar."))
            .size(10.5)
            .color(nmessenger::MUTED),
    );
    ui.add_space(8.0);

    if is_mobile {
        ui.horizontal(|ui| {
            let (texture, name) = if let Some(choice) = app.options.avatar_choice.clone() {
                match choice {
                    AvatarChoice::Preset { name, preview, .. } => (
                        nmessenger::texture(ui.ctx(), &preview)
                            .map(|texture| (texture.id(), texture.size_vec2())),
                        name,
                    ),
                    AvatarChoice::Existing(asset) => (
                        app.texture(ui.ctx(), ImageSource::File(asset.file_id)),
                        asset.name,
                    ),
                    AvatarChoice::Local { path, name } => {
                        (app.texture(ui.ctx(), ImageSource::Local(path)), name)
                    }
                }
            } else {
                (None, tr("Current", "Atual").to_string())
            };
            if let Some((texture, size)) = texture {
                ui.add(
                    egui::Image::new((texture, Vec2::splat(58.0)))
                        .uv(nmessenger::cover_uv(size, Vec2::splat(58.0)))
                        .corner_radius(6),
                );
            } else if let Some(me) = app.data.as_ref().map(|data| data.me.clone()) {
                nmessenger::avatar(app, ui, &me, 58.0);
            }
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(tr("Preview", "Prévia")).size(11.5).strong().color(nmessenger::HEADING));
                ui.add_space(1.0);
                ui.label(RichText::new(&name).size(11.0).color(nmessenger::MUTED));
                if app.options.avatar_saving {
                    ui.spinner();
                }
                if let Some(error) = &app.options.avatar_error {
                    ui.colored_label(Color32::from_rgb(185, 28, 28), error);
                }
            });
        });
        ui.add_space(6.0);
    } else {
        egui::Panel::right("avatar-preview")
            .exact_size(148.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(6),
            )
            .show(ui, |ui| {
                ui.label(RichText::new(tr("Preview", "Prévia")).strong().color(nmessenger::HEADING));
                ui.add_space(8.0);
                if let Some(choice) = app.options.avatar_choice.clone() {
                    let (texture, name) = match choice {
                        AvatarChoice::Preset { name, preview, .. } => (
                            nmessenger::texture(ui.ctx(), &preview)
                                .map(|texture| (texture.id(), texture.size_vec2())),
                            name,
                        ),
                        AvatarChoice::Existing(asset) => (
                            app.texture(ui.ctx(), ImageSource::File(asset.file_id)),
                            asset.name,
                        ),
                        AvatarChoice::Local { path, name } => {
                            (app.texture(ui.ctx(), ImageSource::Local(path)), name)
                        }
                    };
                    if let Some((texture, size)) = texture {
                        ui.add(
                            egui::Image::new((texture, Vec2::splat(128.0)))
                                .uv(nmessenger::cover_uv(size, Vec2::splat(128.0)))
                                .corner_radius(9),
                        );
                    }
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(RichText::new(name).size(11.0)).wrap());
                } else if let Some(me) = app.data.as_ref().map(|data| data.me.clone()) {
                    nmessenger::avatar(app, ui, &me, 128.0);
                }
                if app.options.avatar_saving {
                    ui.spinner();
                }
                if let Some(error) = &app.options.avatar_error {
                    ui.colored_label(Color32::from_rgb(185, 28, 28), error);
                }
            });
    }
    ui.add_enabled_ui(!app.options.avatar_saving, |ui| {
        super::scroll::vertical().id_salt("avatar-gallery").show(ui, |ui| {
            ui.label(RichText::new(tr("Preset pictures", "Imagens predefinidas")).size(11.0).color(nmessenger::MUTED));
            let spacing_x = if is_mobile { 8.0 } else { 10.0 };
            let spacing_y = if is_mobile { 8.0 } else { 10.0 };
            let scrollbar_reserve = if is_mobile { 4.0 } else { 18.0 };
            let content_w = (ui.available_width() - scrollbar_reserve).max(80.0);
            let columns = if is_mobile {
                // Fit at least 3 columns on mobile screens dynamically
                let target = 72.0;
                ((content_w + spacing_x) / (target + spacing_x))
                    .floor()
                    .max(3.0) as usize
            } else {
                let target = 86.0;
                ((content_w + spacing_x) / (target + spacing_x))
                    .floor()
                    .max(1.0) as usize
            };
            let card_w = ((content_w - spacing_x * (columns as f32 - 1.0)) / (columns as f32))
                .floor()
                .clamp(52.0, if is_mobile { 110.0 } else { 96.0 });
            let card_h = {
                let img_w = (card_w - 12.0).clamp(44.0, 72.0);
                img_w + 29.0
            };
            egui::Grid::new("avatar-presets").num_columns(columns).spacing([spacing_x, spacing_y]).show(ui, |ui| {
                for (index, (name, preview, original)) in nmessenger::AVATAR_CATALOG.iter().enumerate() {
                    let selected = matches!(&app.options.avatar_choice, Some(AvatarChoice::Preset { original: current, .. }) if current == original);
                    if let Some(texture) = nmessenger::texture(ui.ctx(), preview) {
                        let response = avatar_card(
                            ui,
                            texture.id(),
                            texture.size_vec2(),
                            name,
                            selected,
                            true,
                            card_w,
                        );
                        if response.clicked() {
                            app.options.avatar_choice = Some(AvatarChoice::Preset {
                                name: (*name).into(),
                                preview: (*preview).into(),
                                original: (*original).into(),
                            });
                            app.options.avatar_error = None;
                        }
                    } else {
                        ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                    }
                    if index % columns == columns - 1 {
                        ui.end_row();
                    }
                }
            });
            let assets = app.data.as_ref().map(|data| data.assets.clone()).unwrap_or_default();
            let user_avatars: Vec<_> = assets.into_iter().filter(|asset| asset.kind == AssetKind::Avatar).collect();
            if !user_avatars.is_empty() {
                ui.add_space(14.0);
                ui.label(RichText::new(tr("My pictures", "Minhas imagens")).size(11.0).color(nmessenger::MUTED));
                let my_spacing = spacing_x;
                let my_target = if is_mobile { 66.0 } else { 76.0 };
                let my_columns = ((content_w + my_spacing) / (my_target + my_spacing)).floor().max(1.0) as usize;
                let my_card_side = ((content_w - my_spacing * (my_columns as f32 - 1.0)) / (my_columns as f32))
                    .floor()
                    .clamp(52.0, if is_mobile { 76.0 } else { 86.0 });
                egui::Grid::new("avatar-user-images")
                    .num_columns(my_columns)
                    .spacing([my_spacing, my_spacing])
                    .show(ui, |ui| {
                        for (index, asset) in user_avatars.into_iter().enumerate() {
                            let selected = matches!(&app.options.avatar_choice, Some(AvatarChoice::Existing(current)) if current.file_id == asset.file_id);
                            if let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::File(asset.file_id)) {
                                let response = avatar_card(
                                    ui,
                                    texture,
                                    size,
                                    &asset.name,
                                    selected,
                                    false,
                                    my_card_side,
                                );
                                if response.clicked() {
                                    app.options.avatar_choice = Some(AvatarChoice::Existing(asset));
                                    app.options.avatar_error = None;
                                }
                            } else {
                                ui.allocate_exact_size(Vec2::splat(my_card_side), egui::Sense::hover());
                            }
                            if index % my_columns == my_columns - 1 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });
    });
    close
}

fn avatar_card(
    ui: &mut egui::Ui,
    texture: egui::TextureId,
    texture_size: Vec2,
    name: &str,
    selected: bool,
    show_label: bool,
    card_w: f32,
) -> egui::Response {
    let (card_size, image_size) = if show_label {
        let img_w = (card_w - 12.0).clamp(44.0, 72.0);
        (Vec2::new(card_w, img_w + 29.0), Vec2::splat(img_w))
    } else {
        let img_w = (card_w - 10.0).max(36.0);
        (Vec2::splat(card_w), Vec2::splat(img_w))
    };

    let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click());
    let response = response.on_hover_text(name);

    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();
        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let bg_color = if selected {
            Color32::from_rgb(232, 243, 255)
        } else if is_hovered {
            Color32::from_rgb(246, 250, 255)
        } else {
            Color32::WHITE
        };

        let (stroke_color, stroke_width) = if selected {
            (nmessenger::BLUE, 2.0)
        } else if is_hovered {
            (Color32::from_rgb(130, 185, 245), 1.2)
        } else {
            (Color32::from_rgb(218, 226, 235), 1.0)
        };

        ui.painter().rect_filled(rect, 8, bg_color);
        ui.painter().rect_stroke(
            rect,
            8,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Inside,
        );

        let img_min = if show_label {
            egui::pos2(
                rect.min.x + (rect.width() - image_size.x) / 2.0,
                rect.min.y + 6.0,
            )
        } else {
            egui::pos2(
                rect.min.x + (rect.width() - image_size.x) / 2.0,
                rect.min.y + (rect.height() - image_size.y) / 2.0,
            )
        };
        let img_rect = Rect::from_min_size(img_min, image_size);

        egui::Image::new((texture, image_size))
            .uv(nmessenger::cover_uv(texture_size, image_size))
            .corner_radius(5)
            .paint_at(ui, img_rect);

        ui.painter().rect_stroke(
            img_rect,
            5,
            egui::Stroke::new(0.5, Color32::from_rgba_premultiplied(0, 0, 0, 30)),
            egui::StrokeKind::Inside,
        );

        if selected {
            let badge_radius = if card_w < 75.0 { 6.5 } else { 7.5 };
            let badge_center = egui::pos2(img_rect.max.x - badge_radius, img_rect.min.y + badge_radius);
            ui.painter().circle_filled(badge_center, badge_radius, nmessenger::BLUE);
            ui.painter().circle_stroke(badge_center, badge_radius, egui::Stroke::new(1.5, Color32::WHITE));
            let s = badge_radius / 7.5;
            ui.painter().line_segment(
                [badge_center + egui::vec2(-3.0 * s, 0.0), badge_center + egui::vec2(-0.8 * s, 2.3 * s)],
                egui::Stroke::new(1.5 * s, Color32::WHITE),
            );
            ui.painter().line_segment(
                [badge_center + egui::vec2(-0.8 * s, 2.3 * s), badge_center + egui::vec2(3.0 * s, -2.2 * s)],
                egui::Stroke::new(1.5 * s, Color32::WHITE),
            );
        }

        if show_label {
            let text_color = if selected {
                Color32::from_rgb(12, 75, 168)
            } else if is_hovered {
                Color32::from_rgb(20, 20, 20)
            } else {
                Color32::from_rgb(60, 65, 75)
            };

            let max_chars = if card_w < 75.0 { 8 } else if card_w < 90.0 { 10 } else { 12 };
            let display_name = if name.chars().count() > max_chars {
                let truncated: String = name.chars().take(max_chars.saturating_sub(1)).collect();
                format!("{truncated}…")
            } else {
                name.to_string()
            };

            let font_size = if card_w < 76.0 { 10.0 } else { 11.0 };
            let label_y = img_rect.max.y + 11.0;

            ui.painter().text(
                egui::pos2(rect.center().x, label_y),
                egui::Align2::CENTER_CENTER,
                display_name,
                egui::FontId::proportional(font_size),
                text_color,
            );
        }
    }

    response
}

fn apply(app: &mut MessengerApp) {
    let Some(choice) = app.options.avatar_choice.clone() else {
        return;
    };
    app.options.avatar_saving = true;
    app.options.avatar_error = None;
    app.send(match choice {
        AvatarChoice::Preset { name, original, .. } => Action::UseAvatarPreset {
            name,
            asset: original,
        },
        AvatarChoice::Existing(asset) => Action::UseAsset(asset),
        AvatarChoice::Local { path, name } => Action::UploadAsset {
            kind: AssetKind::Avatar,
            name,
            shortcut: None,
            local: Some(path),
        },
    });
}
