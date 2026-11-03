use super::nmessenger;
use crate::i18n::tr;
use crate::{
    app::{ContactFilter, MessengerApp, ProfileField},
    transport::Action,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Vec2};
use nm_protocol::*;

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(me) = app.data.as_ref().map(|d| d.me.clone()) else {
        return;
    };
    let ctx = ui.ctx().clone();
    let ctx = &ctx;
    egui::Panel::top("nmessenger-profile")
        .exact_size(78.0)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(Color32::TRANSPARENT)
                .inner_margin(0),
        )
        .show(ui, |ui| {
            let r = ui.max_rect();
            let avatar =
                Rect::from_min_size(Pos2::new(r.left() + 16.0, r.top() + 8.0), Vec2::splat(56.0));
            nmessenger::avatar_at(app, ui, &me, avatar, "perfil");
            let response = ui.interact(avatar, ui.scope_id().with("avatar"), egui::Sense::click());
            if response.hovered() {
                ui.painter().rect_stroke(avatar, 6, egui::Stroke::new(2.0, nmessenger::BLUE), egui::StrokeKind::Inside);
            }
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tr("Click to change your display picture...", "Clique para alterar sua imagem de exibição..."))
                .clicked()
            {
                app.options.avatar_picker = true;
            }
            let is_editing_name = app.profile_edit == Some(ProfileField::Name);
            let name_font = FontId::new(14.0, egui::FontFamily::Name("Segoe UI Bold".into()));
            let name_width = ui
                .fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(me.display_name.clone(), name_font.clone(), nmessenger::INK)
                        .size()
                        .x
                })
                .min((r.width() - 194.0).max(90.0));
            let name_rect = if is_editing_name {
                Rect::from_min_size(
                    Pos2::new(r.left() + 88.0, r.top() + 7.0),
                    Vec2::new((r.width() - 88.0 - 42.0).max(120.0), 22.0),
                )
            } else {
                Rect::from_min_size(
                    Pos2::new(r.left() + 88.0, r.top() + 8.0),
                    Vec2::new(name_width + 4.0, 20.0),
                )
            };
            field(
                app,
                ui,
                ProfileField::Name,
                &me.display_name,
                name_rect,
                name_font,
                nmessenger::INK,
            );
            if !is_editing_name {
                let mut presence = me.presence;
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(Rect::from_min_size(
                        Pos2::new(name_rect.right(), r.top() + 8.0),
                        Vec2::new(105.0, 22.0),
                    )),
                    |ui| {
                        if nmessenger::presence_menu(ui, "profile-presence", &mut presence, false) {
                            app.auto_away = false;
                            app.send(Action::Profile(UpdateProfileRequest {
                                presence: Some(presence),
                                ..Default::default()
                            }));
                        }
                    },
                );
            }
            let psm_rect = Rect::from_min_size(
                Pos2::new(r.left() + 88.0, r.top() + 30.0),
                Vec2::new((r.width() - 88.0 - 42.0).max(100.0), 20.0),
            );
            field(
                app,
                ui,
                ProfileField::PersonalMessage,
                &me.personal_message,
                psm_rect,
                FontId::proportional(12.0),
                Color32::from_rgb(138, 138, 138),
            );
            {
                nmessenger::paint(
                    ctx,
                    ui.painter(),
                    "icons/musical-note.png",
                    Rect::from_min_size(
                        Pos2::new(r.left() + 88.0, r.top() + 52.0),
                        Vec2::splat(12.0),
                    ),
                );
                let music_rect = Rect::from_min_size(
                    Pos2::new(r.left() + 103.0, r.top() + 48.0),
                    Vec2::new((r.width() - 103.0 - 42.0).max(100.0), 20.0),
                );
                field(
                    app,
                    ui,
                    ProfileField::Music,
                    me.now_playing.as_deref().unwrap_or(""),
                    music_rect,
                    FontId::proportional(10.0),
                    nmessenger::BLUE,
                );
            }
            let gear = Rect::from_min_size(Pos2::new(r.right() - 36.0, r.top() + 8.0), Vec2::splat(24.0));
            let response = ui.interact(gear, ui.scope_id().with("settings"), egui::Sense::click());
            if response.hovered() {
                ui.painter()
                    .rect_filled(gear, 4, Color32::from_white_alpha(120));
            }
            nmessenger::paint(ctx, ui.painter(), "icons/settings.png", gear.shrink(3.0));
            egui::Popup::menu(&response).show(|ui| menu(app, ui));
        });
}

fn field(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    kind: ProfileField,
    value: &str,
    rect: Rect,
    font: FontId,
    color: Color32,
) {
    let id = ui.scope_id().with(match kind {
        ProfileField::Name => "edit-name",
        ProfileField::PersonalMessage => "edit-psm",
        ProfileField::Music => "edit-music",
    });
    if app.profile_edit == Some(kind) {
        let btn_width = 22.0;
        let gap = 4.0;
        let edit_width = (rect.width() - btn_width - gap).max(30.0);
        let edit_rect = Rect::from_min_size(rect.min, Vec2::new(edit_width, rect.height()));
        let btn_h = 20.0_f32.min(rect.height());
        let btn_rect = Rect::from_min_size(
            Pos2::new(edit_rect.right() + gap, rect.top() + (rect.height() - btn_h) / 2.0),
            Vec2::new(btn_width, btn_h),
        );

        let _edit_response = ui.put(
            edit_rect,
            egui::TextEdit::singleline(&mut app.profile_draft)
                .id(id)
                .font(font)
                .margin(Vec2::new(3.0, 2.0)),
        );

        let btn_id = id.with("confirm_btn");
        let btn_response = ui.interact(btn_rect, btn_id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tr("Confirm", "Confirmar"));

        let hovered = btn_response.hovered();
        let pressed = btn_response.is_pointer_button_down_on();

        let bg_color = if pressed {
            Color32::from_rgb(175, 225, 190)
        } else if hovered {
            Color32::from_rgb(215, 245, 225)
        } else {
            Color32::from_rgba_unmultiplied(235, 248, 238, 230)
        };

        let stroke_color = if hovered || pressed {
            Color32::from_rgb(45, 140, 70)
        } else {
            Color32::from_rgb(145, 195, 160)
        };

        ui.painter().rect_filled(btn_rect, 3.5, bg_color);
        ui.painter().rect_stroke(
            btn_rect,
            3.5,
            egui::Stroke::new(1.0, stroke_color),
            egui::StrokeKind::Inside,
        );

        let check_stroke_color = if hovered || pressed {
            Color32::from_rgb(20, 115, 45)
        } else {
            Color32::from_rgb(32, 125, 55)
        };
        let check_stroke = egui::Stroke::new(1.8, check_stroke_color);
        let cx = btn_rect.center().x;
        let cy = btn_rect.center().y;
        ui.painter().line_segment(
            [Pos2::new(cx - 4.5, cy + 0.5), Pos2::new(cx - 1.5, cy + 3.8)],
            check_stroke,
        );
        ui.painter().line_segment(
            [Pos2::new(cx - 1.5, cy + 3.8), Pos2::new(cx + 4.5, cy - 3.2)],
            check_stroke,
        );

        let confirm_clicked = btn_response.clicked();
        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let back_pressed = ui.input(|i| {
            i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::BrowserBack)
        });

        if back_pressed {
            app.profile_edit = None;
            app.profile_draft.clear();
            crate::platform::hide_soft_input();
        } else if confirm_clicked || enter_pressed {
            let val = app.profile_draft.trim().to_string();
            if kind != ProfileField::Name || !val.is_empty() {
                let mut request = UpdateProfileRequest::default();
                match kind {
                    ProfileField::Name => request.display_name = Some(val),
                    ProfileField::PersonalMessage => request.personal_message = Some(val),
                    ProfileField::Music => request.now_playing = Some(val),
                }
                app.send(Action::Profile(request));
            }
            app.profile_edit = None;
            app.profile_draft.clear();
            crate::platform::hide_soft_input();
        }
    } else {
        let placeholder = match kind {
            ProfileField::Name => "",
            ProfileField::PersonalMessage => tr("<Enter a personal message>", "<Insira uma mensagem pessoal>"),
            ProfileField::Music => tr("Silence (Add music)", "Silêncio (Adicionar música)"),
        };
        let text = if value.is_empty() { placeholder } else { value };
        if ui.rect_contains_pointer(rect) {
            ui.painter().rect_filled(rect, 3, Color32::from_white_alpha(120));
        }
        nmessenger::left(
            ui,
            rect,
            egui::Label::new(RichText::new(text).font(font).color(color)).truncate(),
        );
        let response = ui.interact(rect, id.with("activate"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tr("Click to edit", "Clique para editar"));
        if response.clicked() {
            app.profile_edit = Some(kind);
            app.profile_draft = value.to_owned();
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }
}

fn menu(app: &mut MessengerApp, ui: &mut egui::Ui) {
    if ui.button(tr("Change display picture...", "Alterar imagem de exibição...")).clicked() {
        app.options.avatar_picker = true;
        ui.close();
    }
    if ui.button(tr("View my profile...", "Ver meu perfil...")).clicked() {
        if let Some(me) = app.data.as_ref().map(|d| d.me.clone()) {
            app.contact_forms.view_profile = Some(me);
        }
        ui.close();
    }
    if ui.button(tr("Edit my profile...", "Editar meu perfil...")).clicked() {
        if let Some(me) = app.data.as_ref().map(|d| d.me.clone()) {
            app.contact_forms.edit_display_name = me.display_name;
            app.contact_forms.edit_personal_message = me.personal_message;
            app.contact_forms.edit_bio = me.bio;
            app.contact_forms.edit_profile_open = true;
        }
        ui.close();
    }
    if ui.button(tr("Settings...", "Configurações...")).clicked() {
        app.open_options();
        app.options.tab = 3;
        ui.close();
    }
    if app.announcements.feed.can_manage && ui.button(tr("Administrator panel...", "Painel do administrador...")).clicked() {
        super::admin::open(app);
        ui.close();
    }
    ui.separator();
    ui.menu_button(tr("Theme colors", "Cores da skin"), |ui| {
        for (name, label, color) in [
            ("azul", tr("Aero Blue", "Azul Aero"), Color32::from_rgb(194, 221, 244)),
            ("verde", tr("Nature Green", "Verde Natureza"), Color32::from_rgb(200, 230, 201)),
            ("rosa", tr("Hot Pink", "Rosa Choque"), Color32::from_rgb(252, 213, 206)),
            ("prata", tr("Metallic Silver", "Prata Metálico"), Color32::from_rgb(229, 229, 234)),
            ("purple", tr("Purple", "Roxo"), Color32::from_rgb(193, 170, 231)),
            ("orange", tr("Orange", "Laranja"), Color32::from_rgb(255, 199, 151)),
        ] {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2, color);
                ui.painter().rect_stroke(
                    rect,
                    2,
                    egui::Stroke::new(1.0, Color32::from_black_alpha(30)),
                    egui::StrokeKind::Inside,
                );
                if ui.button(label).clicked() {
                    let settings = app.data.as_mut().map(|data| {
                        data.settings.theme = name.into();
                        data.settings.clone()
                    });
                    if let Some(settings) = settings {
                        app.send(Action::Settings(Some(settings)));
                    }
                    ui.close();
                }
            });
        }
    });
    ui.menu_button(tr("Contacts", "Contatos"), |ui| {
        if ui.button(tr("Add contact...", "Adicionar contato...")).clicked() {
            app.contact_forms.add_open = true;
            ui.close();
        }
        if ui.button(tr("Create category...", "Criar categoria...")).clicked() {
            app.contact_forms.category_open = true;
            app.contact_forms.category_id = None;
            app.contact_forms.category_name.clear();
            ui.close();
        }
        for (mode, label) in [
            (ContactFilter::All, tr("All contacts", "Todos os contatos")),
            (ContactFilter::Online, "Online"),
            (ContactFilter::Requests, tr("Friend requests", "Solicitações de amizade")),
            (ContactFilter::Blocked, tr("Blocked", "Bloqueados")),
        ] {
            if ui
                .selectable_value(&mut app.contact_filter, mode, label)
                .clicked()
            {
                ui.close();
            }
        }
        ui.separator();
        if ui.button(tr("Import contacts...", "Importar contatos...")).clicked() {
            app.send(Action::ImportContacts);
            ui.close();
        }
        for extension in ["ctt", "json"] {
            if ui.button(format!("{} .{extension}", tr("Export", "Exportar"))).clicked() {
                app.send(Action::ExportContacts(extension.into()));
                ui.close();
            }
        }
    });
    ui.separator();
    if ui
        .button(RichText::new(tr("Sign out", "Desconectar")).color(Color32::from_rgb(220, 38, 38)))
        .clicked()
    {
        app.logout();
        ui.close();
    }
}
