use super::{dialog, nmessenger, theme};
use crate::i18n::tr;
use crate::{app::MessengerApp, transport::Action};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Vec2};
use nm_protocol::*;

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if app.contact_forms.add_open {
        app.contact_forms.add_open = dialog::show(
            ctx,
            "add-contact",
            tr("Add a contact", "Adicionar um contato"),
            [410.0, 310.0],
            |ui| add(app, ui),
        );
    }
    if app.contact_forms.category_open {
        app.contact_forms.category_open = dialog::show(
            ctx,
            "edit-category",
            tr("Contact category", "Categoria de contatos"),
            [360.0, 185.0],
            |ui| category(app, ui),
        );
    }
    if app.contact_forms.group_open {
        let open = dialog::show(
            ctx,
            "create-group",
            tr("New group conversation", "Nova conversa em grupo"),
            [480.0, 560.0],
            |ui| group(app, ui),
        );
        if !open {
            app.contact_forms.group_open = false;
            app.contact_forms.group_members.clear();
            app.contact_forms.group_title.clear();
            app.contact_forms.group_search.clear();
            app.contact_forms.group_filter = crate::app::GroupContactFilter::All;
        }
    }
    super::profile_view::show(app, ctx);
    super::profile_edit::show(app, ctx);
}

fn add(app: &mut MessengerApp, ui: &mut egui::Ui) -> bool {
    let enabled = !app.contact_forms.email.trim().is_empty();
    let action = dialog::buttons(ui, tr("Add contact", "Adicionar contato"), enabled);
    ui.label(
        RichText::new(tr("Enter the email address of the person you want to add.", "Digite o endereço de email da pessoa que deseja adicionar à sua lista."))
            .size(11.0)
            .color(theme::MUTED),
    );
    ui.add_space(10.0);
    ui.label(
        RichText::new(tr("Email address:", "Endereço de email:"))
            .strong()
            .size(11.5)
            .color(super::nmessenger::HEADING),
    );
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        let text_w = if crate::platform::is_mobile() { ui.available_width() - 54.0 } else { ui.available_width() };
        let res = ui.add(
            egui::TextEdit::singleline(&mut app.contact_forms.email)
                .hint_text("exemplo@email.com")
                .margin(egui::Margin::symmetric(10, 7))
                .desired_width(text_w),
        );
        if res.clicked() || res.gained_focus() {
            crate::platform::show_soft_input_type("email");
        }
        res.context_menu(|ui| {
            if ui.button(tr("Paste", "Colar")).clicked() {
                if let Some(text) = crate::platform::get_clipboard_text() {
                    app.contact_forms.email = text.trim().to_string();
                }
                ui.close();
            }
        });
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new(tr("Nickname (optional):", "Apelido (opcional):"))
            .strong()
            .size(11.5)
            .color(super::nmessenger::HEADING),
    );
    ui.add_space(3.0);
    let nick_res = ui.add(
        egui::TextEdit::singleline(&mut app.contact_forms.nickname)
            .hint_text(tr("How you'd like to identify this contact", "Como você deseja identificar este contato"))
            .margin(egui::Margin::symmetric(10, 7))
            .desired_width(f32::INFINITY),
    );
    if nick_res.clicked() || nick_res.gained_focus() {
        crate::platform::show_soft_input();
    }
    nick_res.context_menu(|ui| {
        if ui.button(tr("Paste", "Colar")).clicked() {
            if let Some(text) = crate::platform::get_clipboard_text() {
                app.contact_forms.nickname = text.trim().to_string();
            }
            ui.close();
        }
    });
    ui.add_space(10.0);
    ui.label(
        RichText::new(tr("The contact will receive a request and appear online after accepting.", "O contato receberá uma solicitação e aparecerá conectado assim que aceitar."))
            .small()
            .color(theme::MUTED),
    );
    match action {
        Some(true) => {
            app.send(Action::AddContact(AddContactRequest {
                email: app.contact_forms.email.trim().into(),
                nickname: (!app.contact_forms.nickname.trim().is_empty())
                    .then(|| app.contact_forms.nickname.trim().into()),
            }));
            app.contact_forms.email.clear();
            app.contact_forms.nickname.clear();
            true
        }
        Some(false) => {
            app.contact_forms.email.clear();
            app.contact_forms.nickname.clear();
            true
        }
        None => false,
    }
}

fn category(app: &mut MessengerApp, ui: &mut egui::Ui) -> bool {
    let action = dialog::buttons(ui, tr("Save", "Salvar"), !app.contact_forms.category_name.trim().is_empty());
    ui.label(
        RichText::new(tr("Organize contacts into custom categories.", "Organize seus contatos criando grupos personalizados."))
            .size(11.0)
            .color(theme::MUTED),
    );
    ui.add_space(10.0);
    ui.label(
        RichText::new(tr("Category name:", "Nome da categoria:"))
            .strong()
            .size(11.5)
            .color(super::nmessenger::HEADING),
    );
    ui.add_space(3.0);
    ui.add(
        egui::TextEdit::singleline(&mut app.contact_forms.category_name)
            .hint_text(tr("E.g. Family, Friends, Work...", "Ex: Família, Amigos, Trabalho..."))
            .margin(egui::Margin::symmetric(10, 7))
            .desired_width(f32::INFINITY),
    );
    match action {
        Some(true) => {
            app.send(Action::Category(
                app.contact_forms.category_id,
                app.contact_forms.category_name.trim().into(),
            ));
            app.contact_forms.category_name.clear();
            app.contact_forms.category_id = None;
            true
        }
        Some(false) => {
            app.contact_forms.category_name.clear();
            app.contact_forms.category_id = None;
            true
        }
        None => false,
    }
}

fn presence_rank(presence: Presence) -> u8 {
    match presence {
        Presence::Available => 0,
        Presence::Busy => 1,
        Presence::Away => 2,
        Presence::Invisible | Presence::Offline => 3,
    }
}

fn group(app: &mut MessengerApp, ui: &mut egui::Ui) -> bool {
    let contacts = app
        .data
        .as_ref()
        .map(|d| d.contacts.clone())
        .unwrap_or_default();
    // A request/blocked contact may have changed since this dialog was opened.
    app.contact_forms.group_members.retain(|id| {
        contacts
            .iter()
            .any(|c| c.user.id == *id && !c.blocked && c.state == ContactState::Accepted)
    });
    let selected_count = app.contact_forms.group_members.len();
    let btn_text = if selected_count > 0 {
        format!("{} ({selected_count})", tr("Create conversation", "Criar conversa"))
    } else {
        tr("Create conversation", "Criar conversa").to_string()
    };
    let action = dialog::buttons(ui, &btn_text, selected_count > 0);

    ui.label(
        RichText::new(tr("Create a group to chat with multiple contacts at once.", "Crie um grupo para conversar com múltiplos contatos ao mesmo tempo."))
            .size(11.0)
            .color(theme::MUTED),
    );
    ui.add_space(8.0);

    ui.label(
        RichText::new(tr("Group name (optional):", "Nome do grupo (opcional):"))
            .strong()
            .size(11.5)
            .color(super::nmessenger::HEADING),
    );
    ui.add_space(3.0);
    ui.add(
        egui::TextEdit::singleline(&mut app.contact_forms.group_title)
            .hint_text(tr("E.g. Friends, Family, Work...", "Ex: Amigos, Família, Trabalho..."))
            .margin(egui::Margin::symmetric(10, 7))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(10.0);

    let mut eligible: Vec<_> = contacts
        .iter()
        .filter(|c| !c.blocked && c.state == ContactState::Accepted)
        .cloned()
        .collect();

    eligible.sort_by(|a, b| {
        let rank_a = presence_rank(a.user.presence);
        let rank_b = presence_rank(b.user.presence);
        rank_a
            .cmp(&rank_b)
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });

    let online_count = eligible
        .iter()
        .filter(|c| !matches!(c.user.presence, Presence::Offline | Presence::Invisible))
        .count();

    // Search bar
    let search_h = 28.0;
    let (search_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), search_h),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(search_rect, 6, Color32::WHITE);
    ui.painter().rect_stroke(
        search_rect,
        6,
        egui::Stroke::new(1.0, Color32::from_rgb(205, 222, 238)),
        egui::StrokeKind::Inside,
    );
    nmessenger::paint(
        ui.ctx(),
        ui.painter(),
        "icons/search.png",
        Rect::from_center_size(
            Pos2::new(search_rect.min.x + 15.0, search_rect.center().y),
            Vec2::splat(14.0),
        ),
    );

    let has_search = !app.contact_forms.group_search.is_empty();
    let clear_w = if has_search { 22.0 } else { 0.0 };
    let input_rect = Rect::from_min_max(
        Pos2::new(search_rect.min.x + 28.0, search_rect.min.y + 2.0),
        Pos2::new(search_rect.max.x - 8.0 - clear_w, search_rect.max.y - 2.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(input_rect), |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.contact_forms.group_search)
                .hint_text(tr("Filter contacts by name, nickname or email...", "Filtrar contatos por nome, apelido ou email..."))
                .frame(egui::Frame::NONE)
                .desired_width(f32::INFINITY),
        );
    });
    if has_search {
        let clear_rect = Rect::from_center_size(
            Pos2::new(search_rect.max.x - 14.0, search_rect.center().y),
            Vec2::splat(16.0),
        );
        let clear_res = ui
            .interact(
                clear_rect,
                ui.scope_id().with("clear-group-search"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if clear_res.hovered() {
            ui.painter().circle_filled(
                clear_rect.center(),
                8.0,
                Color32::from_rgb(220, 232, 245),
            );
        }
        let r = 3.5;
        let c = clear_rect.center();
        let stroke = egui::Stroke::new(1.2, Color32::from_rgb(120, 140, 165));
        ui.painter().line_segment(
            [Pos2::new(c.x - r, c.y - r), Pos2::new(c.x + r, c.y + r)],
            stroke,
        );
        ui.painter().line_segment(
            [Pos2::new(c.x - r, c.y + r), Pos2::new(c.x + r, c.y - r)],
            stroke,
        );
        if clear_res.clicked() {
            app.contact_forms.group_search.clear();
        }
    }

    ui.add_space(8.0);

    // Filter tabs & quick actions
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr("Filter:", "Filtrar:"))
                .size(11.0)
                .color(theme::MUTED),
        );
        let tabs = [
            (crate::app::GroupContactFilter::All, format!("{} ({})", tr("All", "Todos"), eligible.len())),
            (crate::app::GroupContactFilter::Online, format!("Online ({online_count})")),
            (crate::app::GroupContactFilter::Selected, format!("{} ({selected_count})", tr("Selected", "Selecionados"))),
        ];
        for (filter, label) in tabs {
            let active = app.contact_forms.group_filter == filter;
            let text_w = ui
                .painter()
                .layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(10.5),
                    Color32::BLACK,
                )
                .rect
                .width();
            let (rect, res) = ui.allocate_exact_size(
                Vec2::new(text_w + 14.0, 20.0),
                egui::Sense::click(),
            );
            let bg = if active {
                Color32::from_rgb(215, 235, 255)
            } else if res.hovered() {
                Color32::from_rgb(240, 246, 252)
            } else {
                Color32::TRANSPARENT
            };
            if bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, 10, bg);
            }
            if active {
                ui.painter().rect_stroke(
                    rect,
                    10,
                    egui::Stroke::new(1.0, Color32::from_rgb(140, 195, 245)),
                    egui::StrokeKind::Inside,
                );
            }
            let text_color = if active {
                Color32::from_rgb(1, 45, 147)
            } else {
                super::nmessenger::HEADING
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(10.5),
                text_color,
            );
            if res.clicked() {
                app.contact_forms.group_filter = filter;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if selected_count > 0
                && ui
                    .add(
                        egui::Button::new(
                            RichText::new(tr("Clear", "Limpar"))
                                .size(10.5)
                                .color(Color32::from_rgb(180, 60, 60)),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    )
                    .on_hover_text(tr("Deselect all selected contacts", "Desmarcar todos os contatos selecionados"))
                    .clicked()
            {
                app.contact_forms.group_members.clear();
            }
        });
    });

    let search_query = app.contact_forms.group_search.trim().to_lowercase();
    let filtered: Vec<_> = eligible
        .into_iter()
        .filter(|c| match app.contact_forms.group_filter {
            crate::app::GroupContactFilter::All => true,
            crate::app::GroupContactFilter::Online => {
                !matches!(c.user.presence, Presence::Offline | Presence::Invisible)
            }
            crate::app::GroupContactFilter::Selected => {
                app.contact_forms.group_members.contains(&c.user.id)
            }
        })
        .filter(|c| {
            if search_query.is_empty() {
                return true;
            }
            c.user.display_name.to_lowercase().contains(&search_query)
                || c.nickname
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&search_query)
                || c.user.email.to_lowercase().contains(&search_query)
                || c.user
                    .personal_message
                    .to_lowercase()
                    .contains(&search_query)
        })
        .collect();

    ui.add_space(6.0);
    let list_height = 230.0;
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(200, 220, 240)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(2, 2))
        .show(ui, |ui| {
            ui.set_min_height(list_height);
            ui.set_max_height(list_height);
            ui.set_width(ui.available_width());

            if filtered.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    let empty_msg = if !search_query.is_empty() {
                        tr("No contacts match this search.", "Nenhum contato encontrado para esta busca.")
                    } else if app.contact_forms.group_filter
                        == crate::app::GroupContactFilter::Selected
                    {
                        tr("No participants selected yet.\nSelect contacts in the 'All' or 'Online' tab.", "Nenhum participante selecionado ainda.\nSelecione contatos na aba 'Todos' ou 'Online'.")
                    } else if app.contact_forms.group_filter
                        == crate::app::GroupContactFilter::Online
                    {
                        tr("No contacts are online right now.", "Nenhum contato está online no momento.")
                    } else {
                        tr("No contacts available to add to the group.", "Nenhum contato disponível para adicionar ao grupo.")
                    };
                    ui.label(
                        RichText::new(empty_msg)
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                    if !search_query.is_empty() {
                        ui.add_space(8.0);
                        if ui.button(tr("Clear search", "Limpar busca")).clicked() {
                            app.contact_forms.group_search.clear();
                        }
                    }
                });
                return;
            }

            let row_h = 38.0;
            super::scroll::vertical()
                .auto_shrink([false, false])
                .id_salt("group-select-contacts")
                .show_rows(ui, row_h, filtered.len(), |ui, range| {
                    for idx in range {
                        let contact = &filtered[idx];
                        let is_selected =
                            app.contact_forms.group_members.contains(&contact.user.id);
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row_h),
                            egui::Sense::click(),
                        );

                        // Row background hover & selection
                        if is_selected {
                            ui.painter().rect_filled(
                                rect,
                                4,
                                Color32::from_rgb(228, 242, 255),
                            );
                            let accent_rect = Rect::from_min_size(
                                rect.min,
                                Vec2::new(3.0, rect.height()),
                            );
                            ui.painter().rect_filled(
                                accent_rect,
                                2,
                                Color32::from_rgb(46, 131, 237),
                            );
                        } else if response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                4,
                                Color32::from_rgb(246, 250, 255),
                            );
                        }

                        // Square Checkbox (16x16 with corner radius 3)
                        let cb_size = 16.0;
                        let cb_rect = Rect::from_center_size(
                            Pos2::new(rect.min.x + 16.0, rect.center().y),
                            Vec2::splat(cb_size),
                        );
                        if is_selected {
                            ui.painter().rect_filled(
                                cb_rect,
                                3,
                                Color32::from_rgb(46, 131, 237),
                            );
                            ui.painter().rect_stroke(
                                cb_rect,
                                3,
                                egui::Stroke::new(1.0, Color32::from_rgb(30, 95, 180)),
                                egui::StrokeKind::Inside,
                            );
                            let center = cb_rect.center();
                            ui.painter().line_segment(
                                [center + Vec2::new(-3.5, 0.0), center + Vec2::new(-1.0, 3.0)],
                                egui::Stroke::new(1.6, Color32::WHITE),
                            );
                            ui.painter().line_segment(
                                [center + Vec2::new(-1.0, 3.0), center + Vec2::new(3.5, -3.5)],
                                egui::Stroke::new(1.6, Color32::WHITE),
                            );
                        } else {
                            ui.painter().rect_filled(
                                cb_rect,
                                3,
                                Color32::WHITE,
                            );
                            let border_color = if response.hovered() {
                                Color32::from_rgb(100, 160, 225)
                            } else {
                                Color32::from_rgb(180, 202, 224)
                            };
                            ui.painter().rect_stroke(
                                cb_rect,
                                3,
                                egui::Stroke::new(1.2, border_color),
                                egui::StrokeKind::Inside,
                            );
                        }

                        // Avatar (26x26)
                        let avatar_rect = Rect::from_center_size(
                            Pos2::new(rect.min.x + 44.0, rect.center().y),
                            Vec2::splat(26.0),
                        );
                        super::nmessenger::avatar_at(app, ui, &contact.user, avatar_rect, "grupo");
                        super::nmessenger::paint(
                            ui.ctx(),
                            ui.painter(),
                            &format!(
                                "status/{}_icone.png",
                                super::nmessenger::status_name(contact.user.presence)
                            ),
                            Rect::from_center_size(
                                avatar_rect.right_bottom() - Vec2::splat(1.0),
                                Vec2::splat(9.0),
                            ),
                        );

                        // Text: Display name + subtitle (status msg or email)
                        let text_left = rect.min.x + 64.0;
                        let text_right = rect.max.x - 70.0;
                        let name = contact.name();
                        ui.painter().text(
                            Pos2::new(text_left, rect.center().y - 6.5),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(11.5),
                            if is_selected {
                                Color32::from_rgb(1, 45, 147)
                            } else {
                                super::nmessenger::HEADING
                            },
                        );

                        let subtitle = if !contact.user.personal_message.is_empty() {
                            contact.user.personal_message.as_str()
                        } else {
                            &contact.user.email
                        };
                        super::nmessenger::left(
                            ui,
                            Rect::from_min_size(
                                Pos2::new(text_left, rect.center().y + 1.0),
                                Vec2::new((text_right - text_left).max(10.0), 14.0),
                            ),
                            egui::Label::new(
                                RichText::new(subtitle)
                                    .size(9.5)
                                    .color(theme::MUTED),
                            )
                            .truncate(),
                        );

                        // Presence label badge on right
                        let (status_text, status_color) = match contact.user.presence {
                            Presence::Available => (tr("Available", "Disponível"), Color32::from_rgb(40, 145, 40)),
                            Presence::Busy => (tr("Busy", "Ocupado"), Color32::from_rgb(200, 45, 45)),
                            Presence::Away => (tr("Away", "Ausente"), Color32::from_rgb(205, 140, 20)),
                            Presence::Offline | Presence::Invisible => {
                                ("Offline", Color32::from_rgb(150, 160, 170))
                            }
                        };
                        ui.painter().text(
                            Pos2::new(rect.max.x - 8.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            status_text,
                            egui::FontId::proportional(9.5),
                            status_color,
                        );

                        // Click toggles selection
                        if response.clicked() {
                            if is_selected {
                                app.contact_forms
                                    .group_members
                                    .retain(|id| *id != contact.user.id);
                            } else {
                                app.contact_forms.group_members.push(contact.user.id);
                            }
                        }
                    }
                });
        });

    match action {
        Some(true) => {
            app.send(Action::CreateConversation(CreateConversationRequest {
                member_ids: app.contact_forms.group_members.clone(),
                title: Some(app.contact_forms.group_title.trim().to_owned())
                    .filter(|s| !s.is_empty()),
                members_can_invite: true,
            }));
            app.contact_forms.group_members.clear();
            app.contact_forms.group_title.clear();
            app.contact_forms.group_search.clear();
            app.contact_forms.group_filter = crate::app::GroupContactFilter::All;
            true
        }
        Some(false) => {
            app.contact_forms.group_members.clear();
            app.contact_forms.group_title.clear();
            app.contact_forms.group_search.clear();
            app.contact_forms.group_filter = crate::app::GroupContactFilter::All;
            true
        }
        None => false,
    }
}
