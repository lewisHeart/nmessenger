use super::{nmessenger, theme};
use crate::i18n::tr;
use crate::{
    app::{ContactDisplaySize, ContactFilter, MessengerApp},
    media::{images::ImageSource, scene::Scene},
    transport::Action,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use nm_protocol::*;
use uuid::Uuid;

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let ctx = &ctx;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(Color32::TRANSPARENT)
                .inner_margin(0),
        )
        .show(ui, |ui| {
            let r = ui.max_rect();
            let search = Rect::from_min_size(
                Pos2::new(r.left() + 14.0, r.top() + 8.0),
                Vec2::new(r.width() - 70.0, 27.0),
            );
            nmessenger::input(
                ui,
                search,
                "nmessenger-contact-search",
                &mut app.search,
                tr("Search contacts...", "Procure um contato..."),
                false,
            );
            if let Some(texture) = nmessenger::texture(ctx, "icons/search.png") {
                ui.painter().image(
                    texture.id(),
                    Rect::from_min_size(
                        Pos2::new(search.right() - 22.0, search.top() + 6.0),
                        Vec2::splat(15.0),
                    ),
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::from_white_alpha(150),
                );
            }
            // Add Contact and tools split button
            let add = Rect::from_min_size(
                Pos2::new(r.right() - 48.0, search.top()),
                Vec2::new(25.0, 27.0),
            );
            let arrow = Rect::from_min_size(
                Pos2::new(r.right() - 23.0, search.top()),
                Vec2::new(15.0, 27.0),
            );
            let add_res = ui
                .interact(add, ui.scope_id().with("add-contact"), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tr("Add contact", "Adicionar contato"));
            let arrow_res = ui
                .interact(arrow, ui.scope_id().with("contact-tools"), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(tr("Contact options", "Opções de contatos"));

            let add_held = add_res.is_pointer_button_down_on();
            let add_hovered = add_res.hovered();
            let arrow_held = arrow_res.is_pointer_button_down_on();
            let arrow_hovered = arrow_res.hovered();
            let arrow_open = egui::Popup::is_id_open(ui.ctx(), arrow_res.id.with("popup"));

            let radius_left = egui::CornerRadius { nw: 4, ne: 0, sw: 4, se: 0 };
            let radius_right = egui::CornerRadius { nw: 0, ne: 4, sw: 0, se: 4 };

            // Left (Add Contact) button background & stroke
            if add_held {
                ui.painter().rect_filled(add, radius_left, Color32::from_rgba_unmultiplied(185, 220, 252, 230));
                ui.painter().rect_stroke(add, radius_left, egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)), egui::StrokeKind::Inside);
            } else if add_hovered {
                ui.painter().rect_filled(add, radius_left, Color32::from_rgba_unmultiplied(225, 240, 255, 210));
                ui.painter().rect_stroke(add, radius_left, egui::Stroke::new(1.0, Color32::from_rgb(145, 195, 245)), egui::StrokeKind::Inside);
            }

            // Right (Tools Arrow) button background & stroke
            if arrow_held || arrow_open {
                ui.painter().rect_filled(arrow, radius_right, Color32::from_rgba_unmultiplied(185, 220, 252, 230));
                ui.painter().rect_stroke(arrow, radius_right, egui::Stroke::new(1.0, Color32::from_rgb(70, 145, 230)), egui::StrokeKind::Inside);
            } else if arrow_hovered {
                ui.painter().rect_filled(arrow, radius_right, Color32::from_rgba_unmultiplied(225, 240, 255, 210));
                ui.painter().rect_stroke(arrow, radius_right, egui::Stroke::new(1.0, Color32::from_rgb(145, 195, 245)), egui::StrokeKind::Inside);
            }

            // Subtle divider line between split button segments when active/hovered
            if add_held || add_hovered || arrow_held || arrow_hovered || arrow_open {
                ui.painter().line_segment(
                    [Pos2::new(add.max.x, add.min.y + 3.0), Pos2::new(add.max.x, add.max.y - 3.0)],
                    egui::Stroke::new(1.0, Color32::from_rgb(175, 210, 242)),
                );
            }

            let add_icon_rect = if add_held {
                Rect::from_center_size(add.center() + Vec2::new(0.0, 1.0), Vec2::splat(16.0))
            } else {
                Rect::from_center_size(add.center(), Vec2::splat(16.0))
            };
            nmessenger::paint(ctx, ui.painter(), "icons/add-contact.svg", add_icon_rect);
            if add_res.clicked() {
                app.contact_forms.add_open = true;
            }

            let arrow_icon_rect = if arrow_held || arrow_open {
                Rect::from_center_size(arrow.center() + Vec2::new(0.0, 1.0), Vec2::splat(7.0))
            } else {
                Rect::from_center_size(arrow.center(), Vec2::splat(7.0))
            };
            nmessenger::paint(ctx, ui.painter(), "icons/arrow.svg", arrow_icon_rect);
            egui::Popup::menu(&arrow_res).show(|ui| tools(app, ui, true));
            let scroll = Rect::from_min_max(
                Pos2::new(r.left() + 4.0, r.top() + 52.0),
                r.max - Vec2::new(4.0, 4.0),
            );
            let bg_res = ui.interact(
                scroll,
                ui.scope_id().with("roster-background"),
                egui::Sense::click(),
            );
            if bg_res.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.contact_forms.selected = None;
            }
            bg_res.context_menu(|ui| tools(app, ui, true));
            ui.scope_builder(egui::UiBuilder::new().max_rect(scroll), |ui| {
                super::scroll::vertical()
                    .id_salt("nmessenger-roster")
                    .auto_shrink([false, false])
                    .show(ui, |ui| list(app, ui));
            });
        });
}

fn online(presence: Presence) -> bool {
    !matches!(presence, Presence::Offline | Presence::Invisible)
}

fn list(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(data) = &app.data else {
        return;
    };
    let mut contacts = data.contacts.clone();
    let mut categories = data.categories.clone();
    let groups: Vec<_> = data
        .conversations
        .iter()
        .filter(|c| c.kind == ConversationKind::Group)
        .cloned()
        .collect();
    let incoming_contacts: Vec<_> = data
        .contacts
        .iter()
        .filter(|c| c.state == ContactState::PendingIncoming && !c.blocked)
        .cloned()
        .collect();
    let incoming = incoming_contacts.len();
    let query = app.search.to_lowercase();
    contacts.retain(|c| {
        (c.name().to_lowercase().contains(&query)
            || c.user.email.to_lowercase().contains(&query)
            || c.user.display_name.to_lowercase().contains(&query))
            && match app.contact_filter {
                ContactFilter::All => !c.blocked && c.state == ContactState::Accepted,
                ContactFilter::Online => {
                    !c.blocked && c.state == ContactState::Accepted && online(c.user.presence)
                }
                ContactFilter::Requests => !c.blocked && c.state != ContactState::Accepted,
                ContactFilter::Blocked => c.blocked,
            }
    });
    contacts.sort_by_key(|c| (!online(c.user.presence), c.name().to_lowercase()));
    categories.sort_by_key(|category| category.position);
    if incoming > 0 && app.contact_filter != ContactFilter::Requests {
        egui::Frame::new()
            .fill(Color32::from_rgb(244, 248, 255))
            .stroke(Stroke::new(1.0, Color32::from_rgb(180, 215, 248)))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                    nmessenger::paint(ui.ctx(), ui.painter(), "icons/add-contact.svg", icon_rect);

                    let text = if incoming == 1 {
                        let name = &incoming_contacts[0].user.display_name;
                        format!("{}: {}", tr("Nova solicitação", "New request"), name)
                    } else {
                        format!("{} {}", incoming, tr("solicitações de amizade", "friend requests"))
                    };
                    ui.label(
                        RichText::new(text)
                            .font(FontId::new(11.0, egui::FontFamily::Name("Segoe UI Bold".into())))
                            .color(nmessenger::HEADING),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let view_btn = ui
                            .add(
                                egui::Button::new(
                                    RichText::new(tr("Exibir", "View"))
                                        .size(10.5)
                                        .color(nmessenger::BLUE)
                                        .strong(),
                                )
                                .fill(Color32::from_rgb(228, 240, 255))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(160, 205, 245)))
                                .corner_radius(3)
                                .min_size(Vec2::new(48.0, 20.0)),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if view_btn.clicked() {
                            app.contact_filter = ContactFilter::Requests;
                        }
                    });
                });
            });
        ui.add_space(4.0);
    }
    let pending_group_invites = app
        .data
        .as_ref()
        .map(|d| d.conversation_invites.clone())
        .unwrap_or_default();
    if !pending_group_invites.is_empty() {
        for invite in &pending_group_invites {
            egui::Frame::new()
                .fill(Color32::from_rgb(240, 246, 255))
                .stroke(Stroke::new(1.0, Color32::from_rgb(170, 210, 245)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let inviter_name = invite.inviter.display_name.clone();
                        let group_name = invite
                            .conversation_title
                            .clone()
                            .unwrap_or_else(|| tr("Group", "Grupo").into());
                        ui.label(
                            RichText::new(format!(
                                "📬 {} convidou você para '{}'",
                                inviter_name, group_name
                            ))
                            .size(11.0)
                            .color(nmessenger::HEADING),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(
                                    RichText::new(tr("Decline", "Recusar"))
                                        .size(10.5)
                                        .color(Color32::from_rgb(150, 40, 40)),
                                )
                                .clicked()
                            {
                                app.send(Action::DeclineGroupInvite(invite.conversation_id));
                            }
                            if ui
                                .button(
                                    RichText::new(tr("Accept", "Aceitar"))
                                        .size(10.5)
                                        .color(nmessenger::BLUE)
                                        .strong(),
                                )
                                .clicked()
                            {
                                app.send(Action::AcceptGroupInvite(invite.conversation_id));
                            }
                        });
                    });
                });
            ui.add_space(4.0);
        }
    }
    if app.contact_filter == ContactFilter::Requests {
        friend_requests_view(app, ui, &contacts);
        return;
    }
    if app.contact_filter == ContactFilter::Blocked {
        blocked_contacts_view(app, ui, &contacts);
        return;
    }
    let favorites: Vec<_> = contacts.iter().filter(|c| c.favorite).cloned().collect();
    section(
        app,
        ui,
        "favorites",
        tr("Favorites", "Favoritos"),
        &favorites,
        Some(DropTarget::Favorite),
    );
    ui.add_space(3.0);
    let group_online = groups
        .iter()
        .filter(|g| g.members.iter().any(|m| online(m.presence)))
        .count();
    let expanded = heading(
        app,
        ui,
        "groups",
        tr("Groups", "Grupos"),
        (group_online, groups.len()),
        None,
        true,
    );
    if expanded {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(10, 0))
            .show(ui, |ui| {
                if groups.is_empty() {
                    placeholder(ui, tr("No group conversations", "Nenhum grupo de chat"));
                }
                let groups_size = app.section_size("groups");
                for group in &groups {
                    group_row(app, ui, group, groups_size);
                }
            });
    }
    ui.add_space(4.0);
    for category in categories {
        let members: Vec<_> = contacts
            .iter()
            .filter(|c| !c.favorite && c.category_ids.contains(&category.id))
            .cloned()
            .collect();
        section(
            app,
            ui,
            &category.id.to_string(),
            &category.name,
            &members,
            Some(DropTarget::Category(category.id)),
        );
        ui.add_space(4.0);
    }
    let available: Vec<_> = contacts
        .iter()
        .filter(|c| !c.favorite && c.category_ids.is_empty() && online(c.user.presence))
        .cloned()
        .collect();
    let offline: Vec<_> = contacts
        .iter()
        .filter(|c| !c.favorite && c.category_ids.is_empty() && !online(c.user.presence))
        .cloned()
        .collect();
    section(
        app,
        ui,
        "available",
        tr("Available", "Disponíveis"),
        &available,
        Some(DropTarget::General),
    );
    ui.add_space(4.0);
    section(
        app,
        ui,
        "offline",
        "Offline",
        &offline,
        Some(DropTarget::General),
    );
}

#[derive(Clone, Copy)]
enum DropTarget {
    Favorite,
    Category(Uuid),
    General,
}

fn heading(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    counts: (usize, usize),
    drop: Option<DropTarget>,
    create_group: bool,
) -> bool {
    let (count, total) = counts;
    let closed = app.collapsed_sections.contains(key);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4, Color32::from_rgb(225, 240, 253));
    }
    let center = Pos2::new(rect.left() + 11.0, rect.center().y);
    let arrow_rect = Rect::from_center_size(center, Vec2::splat(13.0));
    let angle = if closed {
        -std::f32::consts::FRAC_PI_2
    } else {
        0.0
    };
    nmessenger::paint_rotated(ui.ctx(), ui.painter(), "icons/arrow.svg", arrow_rect, angle);
    let font = FontId::new(11.0, egui::FontFamily::Name("Segoe UI Bold".into()));
    let mut x_offset = rect.left() + 21.0;
    if key == "favorites" || title == "Favoritos" {
        let star_size = 13.0_f32;
        let star_rect = Rect::from_center_size(
            Pos2::new(x_offset + star_size * 0.5, rect.center().y),
            Vec2::splat(star_size),
        );
        nmessenger::paint(ui.ctx(), ui.painter(), "emojis/star.png", star_rect);
        x_offset += star_size + 4.0;
    }
    let width = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(title.into(), font.clone(), nmessenger::HEADING)
            .size()
            .x
    });
    ui.painter().text(
        Pos2::new(x_offset, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        font,
        nmessenger::HEADING,
    );
    ui.painter().text(
        Pos2::new(x_offset + width + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("({count}/{total})"),
        FontId::proportional(10.0),
        nmessenger::MUTED,
    );
    if response.clicked() {
        if closed {
            app.collapsed_sections.remove(key);
        } else {
            app.collapsed_sections.insert(key.into());
        }
    }
    if let Some(target) = drop
        && let Some(user) = response.dnd_release_payload::<Uuid>()
    {
        let request = match target {
            DropTarget::Favorite => UpdateContactRequest {
                favorite: Some(true),
                ..Default::default()
            },
            DropTarget::Category(category) => UpdateContactRequest {
                favorite: Some(false),
                category_ids: Some(vec![category]),
                ..Default::default()
            },
            DropTarget::General => UpdateContactRequest {
                favorite: Some(false),
                category_ids: Some(Vec::new()),
                ..Default::default()
            },
        };
        app.send(Action::Contact(*user, request));
    }
    response.context_menu(|ui| {
        if let Some(DropTarget::Category(id)) = drop {
            if ui.button(tr("Rename category...", "Renomear categoria...")).clicked() {
                app.contact_forms.category_id = Some(id);
                app.contact_forms.category_name = title.into();
                app.contact_forms.category_open = true;
                ui.close();
            }
            if ui.button(tr("Delete category", "Excluir categoria")).clicked() {
                app.send(Action::RemoveCategory(id));
                ui.close();
            }
        }
        if ui
            .button(if closed {
                tr("Expand this section", "Expandir esta seção")
            } else {
                tr("Collapse this section", "Recolher esta seção")
            })
            .clicked()
        {
            if closed {
                app.collapsed_sections.remove(key);
            } else {
                app.collapsed_sections.insert(key.into());
            }
            ui.close();
        }
        if ui.button(tr("Expand all sections", "Expandir todas as seções")).clicked() {
            app.collapsed_sections.clear();
            ui.close();
        }
        ui.separator();
        ui.menu_button(tr("Display size", "Tamanho de exibição"), |ui| {
            let current = app.section_size(key);
            if key != "favorites"
                && ui
                    .selectable_label(current == ContactDisplaySize::Small, tr("Small", "Pequeno"))
                    .clicked()
            {
                app.set_section_size(key, ContactDisplaySize::Small);
                ui.close();
            }
            if ui
                .selectable_label(current == ContactDisplaySize::Medium, tr("Medium", "Médio"))
                .clicked()
            {
                app.set_section_size(key, ContactDisplaySize::Medium);
                ui.close();
            }
            if key != "available"
                && key != "offline"
                && ui
                    .selectable_label(current == ContactDisplaySize::Large, tr("Large", "Grande"))
                    .clicked()
            {
                app.set_section_size(key, ContactDisplaySize::Large);
                ui.close();
            }
        });
        ui.separator();
        tools(app, ui, false);
    });
    if create_group {
        let button = Rect::from_min_size(
            Pos2::new(rect.right() - 79.0, rect.top() + 2.0),
            Vec2::new(77.0, 18.0),
        );
        if ui
            .put(
                button,
                egui::Link::new(RichText::new("+ Novo Grupo").size(10.0)),
            )
            .clicked()
        {
            app.contact_forms.group_open = true;
        }
    }
    !app.collapsed_sections.contains(key)
}

fn section(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    contacts: &[Contact],
    target: Option<DropTarget>,
) {
    if heading(
        app,
        ui,
        key,
        title,
        (
            contacts.iter().filter(|c| online(c.user.presence)).count(),
            contacts.len(),
        ),
        target,
        false,
    ) {
        let size = app.section_size(key);
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(10, 0))
            .show(ui, |ui| {
                if contacts.is_empty() {
                    placeholder(
                        ui,
                        if key == "favorites" {
                            tr("No favorite contacts", "Nenhum contato favorito")
                        } else {
                            tr("No contacts", "Nenhum contato")
                        },
                    );
                }
                for contact in contacts {
                    row(app, ui, contact, size);
                }
            });
    }
}

fn placeholder(ui: &mut egui::Ui, text: &str) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), egui::Sense::hover());
    ui.painter().text(
        rect.min + Vec2::new(25.0, 8.0),
        egui::Align2::LEFT_CENTER,
        text,
        FontId::proportional(10.0),
        Color32::from_rgb(153, 163, 175),
    );
}

fn row(app: &mut MessengerApp, ui: &mut egui::Ui, contact: &Contact, size: ContactDisplaySize) {
    let is_mobile = crate::platform::is_mobile();
    let row_height = match size {
        ContactDisplaySize::Small if is_mobile => 34.0,
        ContactDisplaySize::Small => 22.0,
        ContactDisplaySize::Medium => 40.0,
        ContactDisplaySize::Large => 54.0,
    };
    ui.push_id(contact.user.id, |ui| {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            egui::Sense::click_and_drag(),
        );
        let selected = app.contact_forms.selected == Some(contact.user.id);
        super::contact_preview::show(app, ui, contact, &response);
        if response.container_hovered() || selected {
            ui.painter().rect_filled(
                rect,
                4,
                Color32::from_rgba_unmultiplied(219, 238, 255, if selected { 210 } else { 140 }),
            );
        }
        let name = contact
            .nickname
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|nick| format!("{nick} ({})", contact.user.display_name))
            .unwrap_or_else(|| contact.user.display_name.clone());
        match size {
            ContactDisplaySize::Large => {
                nmessenger::avatar_at(
                    app,
                    ui,
                    &contact.user,
                    Rect::from_min_size(rect.min + Vec2::new(5.0, 2.0), Vec2::splat(48.0)),
                    "perfil",
                );
                ui.painter().text(
                    rect.min + Vec2::new(62.0, 18.0),
                    egui::Align2::LEFT_CENTER,
                    &name,
                    FontId::proportional(12.0),
                    nmessenger::HEADING,
                );
                nmessenger::left(
                    ui,
                    Rect::from_min_size(
                        rect.min + Vec2::new(62.0, 27.0),
                        Vec2::new((rect.width() - 68.0).max(5.0), 17.0),
                    ),
                    egui::Label::new(
                        RichText::new(&contact.user.personal_message)
                            .size(10.0)
                            .color(nmessenger::MUTED),
                    )
                    .truncate(),
                );
            }
            ContactDisplaySize::Medium => {
                let avatar_rect =
                    Rect::from_min_size(rect.min + Vec2::new(5.0, 4.0), Vec2::splat(30.0));
                nmessenger::avatar_at(app, ui, &contact.user, avatar_rect, "perfil");
                nmessenger::paint(
                    ui.ctx(),
                    ui.painter(),
                    &format!(
                        "status/{}_icone.png",
                        nmessenger::status_name(contact.user.presence)
                    ),
                    Rect::from_center_size(
                        avatar_rect.right_bottom() - Vec2::splat(2.0),
                        Vec2::splat(9.0),
                    ),
                );
                ui.painter().text(
                    rect.min + Vec2::new(42.0, 12.0),
                    egui::Align2::LEFT_CENTER,
                    &name,
                    FontId::proportional(11.0),
                    nmessenger::HEADING,
                );
                let msg = if !contact.user.personal_message.is_empty() {
                    contact.user.personal_message.as_str()
                } else {
                    nmessenger::status_label(contact.user.presence)
                };
                nmessenger::left(
                    ui,
                    Rect::from_min_size(
                        rect.min + Vec2::new(42.0, 20.0),
                        Vec2::new((rect.width() - 48.0).max(5.0), 16.0),
                    ),
                    egui::Label::new(RichText::new(msg).size(9.5).color(nmessenger::MUTED))
                        .truncate(),
                );
            }
            ContactDisplaySize::Small => {
                nmessenger::paint(
                    ui.ctx(),
                    ui.painter(),
                    &format!(
                        "status/{}_icone.png",
                        nmessenger::status_name(contact.user.presence)
                    ),
                    Rect::from_center_size(rect.min + Vec2::new(10.0, 11.0), Vec2::splat(9.0)),
                );
                let font = FontId::proportional(10.0);
                let width = ui
                    .fonts_mut(|fonts| {
                        fonts
                            .layout_no_wrap(name.clone(), font.clone(), nmessenger::HEADING)
                            .size()
                            .x
                    })
                    .min(rect.width() * 0.62);
                nmessenger::left(
                    ui,
                    Rect::from_min_size(
                        rect.min + Vec2::new(21.0, 2.0),
                        Vec2::new(width + 2.0, 18.0),
                    ),
                    egui::Label::new(RichText::new(&name).font(font).color(nmessenger::HEADING))
                        .truncate(),
                );
                if !contact.user.personal_message.is_empty() && rect.width() - width > 60.0 {
                    nmessenger::left(
                        ui,
                        Rect::from_min_size(
                            rect.min + Vec2::new(width + 29.0, 2.0),
                            Vec2::new((rect.width() - width - 34.0).max(5.0), 18.0),
                        ),
                        egui::Label::new(
                            RichText::new(format!("— {}", contact.user.personal_message))
                                .size(10.0)
                                .color(nmessenger::MUTED),
                        )
                        .truncate(),
                    );
                }
            }
        }
        let unread: i64 = app
            .data
            .as_ref()
            .map(|data| {
                data.conversations
                    .iter()
                    .filter(|c| {
                        c.kind == ConversationKind::Direct
                            && c.members.iter().any(|m| m.id == contact.user.id)
                    })
                    .map(|c| c.unread)
                    .sum()
            })
            .unwrap_or(0);
        if unread > 0 {
            let badge = Rect::from_center_size(
                Pos2::new(rect.right() - 12.0, rect.center().y),
                Vec2::new(20.0, 15.0),
            );
            ui.painter()
                .rect_filled(badge, 8, Color32::from_rgb(239, 68, 68));
            ui.painter().text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                unread.to_string(),
                FontId::proportional(9.0),
                Color32::WHITE,
            );
        }
        if response.clicked() {
            if app.contact_forms.selected == Some(contact.user.id) {
                app.contact_forms.selected = None;
            } else {
                app.contact_forms.selected = Some(contact.user.id);
            }
        }
        if response.drag_started() {
            response.dnd_set_drag_payload(contact.user.id);
        }
        if (response.double_clicked() || (is_mobile && response.clicked())) && !contact.blocked {
            app.send(Action::CreateConversation(CreateConversationRequest {
                member_ids: vec![contact.user.id],
                title: None,
                members_can_invite: true,
            }));
        }
        response.context_menu(|ui| {
            if ui.button(tr("Send message", "Enviar mensagem")).clicked() {
                app.send(Action::CreateConversation(CreateConversationRequest {
                    member_ids: vec![contact.user.id],
                    title: None,
                    members_can_invite: true,
                }));
                ui.close();
            }
            if ui.button(tr("View profile...", "Ver perfil...")).clicked() {
                app.contact_forms.view_profile = Some(contact.user.clone());
                ui.close();
            }
            if ui
                .button(if contact.favorite {
                    tr("Remove from favorites", "Remover dos favoritos")
                } else {
                    tr("Add to favorites", "Adicionar aos favoritos")
                })
                .clicked()
            {
                app.send(Action::Contact(
                    contact.user.id,
                    UpdateContactRequest {
                        favorite: Some(!contact.favorite),
                        ..Default::default()
                    },
                ));
                ui.close();
            }
            if ui
                .button(if contact.blocked {
                    tr("Unblock", "Desbloquear")
                } else {
                    tr("Block", "Bloquear")
                })
                .clicked()
            {
                app.send(Action::Contact(
                    contact.user.id,
                    UpdateContactRequest {
                        blocked: Some(!contact.blocked),
                        ..Default::default()
                    },
                ));
                ui.close();
            }
            if ui.button(tr("Remove contact", "Remover contato")).clicked() {
                app.send(Action::RemoveContact(contact.user.id));
                ui.close();
            }
        });
    });
}

fn group_presence(group: &Conversation) -> Presence {
    let mut any_busy = false;
    let mut any_away = false;
    for m in &group.members {
        match m.presence {
            Presence::Available => return Presence::Available,
            Presence::Busy => any_busy = true,
            Presence::Away => any_away = true,
            _ => {}
        }
    }
    if any_busy {
        Presence::Busy
    } else if any_away {
        Presence::Away
    } else {
        Presence::Offline
    }
}

fn group_avatar(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    group: &Conversation,
    rect: Rect,
    presence: Presence,
    role: &str,
) {
    let inner = nmessenger::frame(ui.ctx(), ui.painter(), rect, presence, role);
    let me_id = app.data.as_ref().map(|d| d.me.id);
    let mut member_avatars: Vec<Uuid> = group
        .members
        .iter()
        .filter(|m| Some(m.id) != me_id)
        .filter_map(|m| {
            m.avatar_id
                .and_then(|id| {
                    app.data
                        .as_ref()
                        .and_then(|data| data.assets.iter().find(|a| a.id == id).map(|a| a.file_id))
                })
                .or_else(|| Scene::parse(&m.scene).avatar_file)
        })
        .collect();

    if member_avatars.is_empty()
        && let Some(my_avatar) = group
            .members
            .iter()
            .find(|m| Some(m.id) == me_id)
            .and_then(|m| {
                m.avatar_id
                    .and_then(|id| {
                        app.data.as_ref().and_then(|data| {
                            data.assets.iter().find(|a| a.id == id).map(|a| a.file_id)
                        })
                    })
                    .or_else(|| Scene::parse(&m.scene).avatar_file)
            })
    {
        member_avatars.push(my_avatar);
    }

    let radius = nmessenger::frame_radius(inner);
    if member_avatars.len() >= 2 {
        let f1 = member_avatars[0];
        let f2 = member_avatars[1];
        let half_w = inner.width() * 0.5;
        let r1 = Rect::from_min_size(inner.min, Vec2::new(half_w, inner.height()));
        let r2 = Rect::from_min_size(
            inner.min + Vec2::new(half_w, 0.0),
            Vec2::new(half_w, inner.height()),
        );
        let t1 = app.texture(ui.ctx(), ImageSource::File(f1));
        let t2 = app.texture(ui.ctx(), ImageSource::File(f2));
        if let Some((tex1, size1)) = t1 {
            ui.put(
                r1,
                egui::Image::new((tex1, r1.size()))
                    .uv(nmessenger::cover_uv(size1, r1.size()))
                    .corner_radius(egui::CornerRadius {
                        nw: radius,
                        sw: radius,
                        ne: 0,
                        se: 0,
                    }),
            );
        }
        if let Some((tex2, size2)) = t2 {
            ui.put(
                r2,
                egui::Image::new((tex2, r2.size()))
                    .uv(nmessenger::cover_uv(size2, r2.size()))
                    .corner_radius(egui::CornerRadius {
                        ne: radius,
                        se: radius,
                        nw: 0,
                        sw: 0,
                    }),
            );
        }
        ui.painter().line_segment(
            [
                Pos2::new(inner.center().x, inner.top()),
                Pos2::new(inner.center().x, inner.bottom()),
            ],
            Stroke::new(1.0, Color32::from_white_alpha(180)),
        );
    } else if member_avatars.len() == 1 {
        let file = member_avatars[0];
        if let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::File(file)) {
            ui.put(
                inner,
                egui::Image::new((texture, inner.size()))
                    .uv(nmessenger::cover_uv(size, inner.size()))
                    .corner_radius(radius),
            );
        } else {
            draw_default_group_photo(ui, inner, presence);
        }
    } else {
        draw_default_group_photo(ui, inner, presence);
    }
    nmessenger::paint_avatar_rim(ui.painter(), inner, presence);
}

fn draw_default_group_photo(ui: &mut egui::Ui, inner: Rect, presence: Presence) {
    ui.painter().rect_filled(
        inner,
        nmessenger::frame_radius(inner),
        Color32::from_rgb(238, 245, 253),
    );
    let icon_size = (inner.width() * 0.72).clamp(16.0, 32.0);
    nmessenger::paint(
        ui.ctx(),
        ui.painter(),
        &format!("status/{}_grupo.svg", nmessenger::status_name(presence)),
        Rect::from_center_size(inner.center(), Vec2::splat(icon_size)),
    );
}

fn group_row(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    group: &Conversation,
    size: ContactDisplaySize,
) {
    let is_mobile = crate::platform::is_mobile();
    let row_height = match size {
        ContactDisplaySize::Small if is_mobile => 34.0,
        ContactDisplaySize::Small => 22.0,
        ContactDisplaySize::Medium => 40.0,
        ContactDisplaySize::Large => 54.0,
    };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4, Color32::from_rgb(219, 238, 255));
    }
    let name = app.conversation_title(group);
    let presence = group_presence(group);
    let online_count = group.members.iter().filter(|m| online(m.presence)).count();
    match size {
        ContactDisplaySize::Large => {
            let avatar_rect =
                Rect::from_min_size(rect.min + Vec2::new(5.0, 2.0), Vec2::splat(48.0));
            group_avatar(app, ui, group, avatar_rect, presence, "perfil");
            nmessenger::paint(
                ui.ctx(),
                ui.painter(),
                &format!("status/{}_grupo.svg", nmessenger::status_name(presence)),
                Rect::from_center_size(
                    avatar_rect.right_bottom() - Vec2::splat(3.0),
                    Vec2::splat(13.0),
                ),
            );
            ui.painter().text(
                rect.min + Vec2::new(62.0, 18.0),
                egui::Align2::LEFT_CENTER,
                &name,
                FontId::proportional(12.0),
                nmessenger::HEADING,
            );
            ui.painter().text(
                rect.min + Vec2::new(62.0, 32.0),
                egui::Align2::LEFT_CENTER,
                format!("({online_count}/{} Disponível)", group.members.len()),
                FontId::proportional(10.0),
                nmessenger::MUTED,
            );
        }
        ContactDisplaySize::Medium => {
            let avatar_rect =
                Rect::from_min_size(rect.min + Vec2::new(5.0, 4.0), Vec2::splat(30.0));
            group_avatar(app, ui, group, avatar_rect, presence, "perfil");
            nmessenger::paint(
                ui.ctx(),
                ui.painter(),
                &format!("status/{}_grupo.svg", nmessenger::status_name(presence)),
                Rect::from_center_size(
                    avatar_rect.right_bottom() - Vec2::splat(2.0),
                    Vec2::splat(11.0),
                ),
            );
            ui.painter().text(
                rect.min + Vec2::new(42.0, 12.0),
                egui::Align2::LEFT_CENTER,
                &name,
                FontId::proportional(11.0),
                nmessenger::HEADING,
            );
            ui.painter().text(
                rect.min + Vec2::new(42.0, 24.0),
                egui::Align2::LEFT_CENTER,
                format!("({online_count}/{} Disponível)", group.members.len()),
                FontId::proportional(9.5),
                nmessenger::MUTED,
            );
        }
        ContactDisplaySize::Small => {
            nmessenger::paint(
                ui.ctx(),
                ui.painter(),
                &format!("status/{}_grupo.svg", nmessenger::status_name(presence)),
                Rect::from_center_size(rect.min + Vec2::new(12.0, 11.0), Vec2::splat(15.0)),
            );
            let font = FontId::proportional(10.0);
            let width = ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(name.clone(), font.clone(), nmessenger::HEADING)
                    .size()
                    .x
            });
            ui.painter().text(
                rect.min + Vec2::new(25.0, 11.0),
                egui::Align2::LEFT_CENTER,
                &name,
                font,
                nmessenger::HEADING,
            );
            ui.painter().text(
                rect.min + Vec2::new(31.0 + width, 11.0),
                egui::Align2::LEFT_CENTER,
                format!("({online_count}/{} Disponível)", group.members.len()),
                FontId::proportional(10.0),
                nmessenger::MUTED,
            );
        }
    }
    if response.double_clicked() || (is_mobile && response.clicked()) {
        app.open_conversation(group.id);
    }
    response.context_menu(|ui| {
        if ui.button(tr("Open conversation", "Abrir conversa")).clicked() {
            app.open_conversation(group.id);
            ui.close();
        }
        if ui.button(tr("Invite / Manage members...", "Convidar / Gerenciar participantes...")).clicked() {
            app.open_conversation(group.id);
            app.conversation_forms.conversation = Some(group.id);
            app.conversation_forms.viewport = Some(if is_mobile {
                egui::ViewportId::ROOT
            } else {
                egui::ViewportId::from_hash_of(("nm-conversation", group.id))
            });
            app.conversation_forms.title = group.title.clone().unwrap_or_default();
            app.conversation_forms.members_open = true;
            ui.close();
        }
        if ui.button(tr("Leave group", "Sair do grupo")).clicked()
            && let Some(me) = app.data.as_ref().map(|data| data.me.id)
        {
            app.send(Action::RemoveMember(group.id, me));
            ui.close();
        }
    });
}

fn tools(app: &mut MessengerApp, ui: &mut egui::Ui, include_import_export: bool) {
    if ui.button(tr("Add contact", "Adicionar contato")).clicked() {
        app.contact_forms.add_open = true;
        ui.close();
    }
    if ui.button(tr("Create category", "Criar categoria")).clicked() {
        app.contact_forms.category_open = true;
        app.contact_forms.category_id = None;
        app.contact_forms.category_name.clear();
        ui.close();
    }
    if ui.button(tr("Create group conversation", "Criar conversa em grupo")).clicked() {
        app.contact_forms.group_open = true;
        ui.close();
    }
    if include_import_export {
        ui.separator();
        if ui.button(tr("Import .ctt / JSON", "Importar .ctt / JSON")).clicked() {
            app.send(Action::ImportContacts);
            ui.close();
        }
        for format in ["ctt", "json"] {
            if ui.button(format!("{} .{format}", tr("Export", "Exportar"))).clicked() {
                app.send(Action::ExportContacts(format.into()));
                ui.close();
            }
        }
    }
}

fn friend_requests_view(app: &mut MessengerApp, ui: &mut egui::Ui, contacts: &[Contact]) {
    let ctx = ui.ctx().clone();
    let ctx = &ctx;

    // 1. Navigation toolbar (clean MSN-style breadcrumb bar)
    egui::Frame::new()
        .fill(Color32::from_rgb(240, 246, 254))
        .stroke(Stroke::new(1.0, Color32::from_rgb(205, 225, 245)))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let back_clicked = ui
                    .add(
                        egui::Button::new(
                            RichText::new(tr("< Voltar para contatos", "< Back to contacts"))
                                .size(11.0)
                                .color(nmessenger::BLUE)
                                .strong(),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(tr(
                        "Retornar à lista principal de contatos (Esc)",
                        "Return to main contact list (Esc)",
                    ))
                    .clicked();

                if back_clicked || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    app.contact_filter = ContactFilter::All;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !contacts.is_empty() {
                        let badge_text =
                            format!("{} {}", contacts.len(), tr("pendente(s)", "pending"));
                        ui.label(
                            RichText::new(badge_text)
                                .size(10.5)
                                .color(nmessenger::MUTED),
                        );
                    }
                });
            });
        });

    ui.add_space(6.0);

    // 2. Section Title
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
        nmessenger::paint(ctx, ui.painter(), "icons/add-contact.svg", icon_rect);
        ui.label(
            RichText::new(tr("Solicitações de amizade", "Friend requests"))
                .font(FontId::new(12.0, egui::FontFamily::Name("Segoe UI Bold".into())))
                .color(nmessenger::HEADING),
        );
    });

    ui.add_space(3.0);
    let sep_rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            Pos2::new(sep_rect.left(), sep_rect.top()),
            Pos2::new(sep_rect.right(), sep_rect.top()),
        ],
        Stroke::new(1.0, Color32::from_rgb(225, 235, 245)),
    );
    ui.add_space(6.0);

    // 3. Empty state
    if contacts.is_empty() {
        ui.add_space(36.0);
        ui.vertical_centered(|ui| {
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
            nmessenger::paint(ctx, ui.painter(), "icons/person.svg", icon_rect);
            ui.add_space(10.0);
            ui.label(
                RichText::new(tr(
                    "Nenhuma solicitação pendente",
                    "No pending friend requests",
                ))
                .font(FontId::new(12.0, egui::FontFamily::Name("Segoe UI Bold".into())))
                .color(nmessenger::HEADING),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr(
                    "Novas solicitações de amizade aparecerão aqui.",
                    "New friend requests will appear here.",
                ))
                .size(10.5)
                .color(nmessenger::MUTED),
            );
            ui.add_space(16.0);
            let return_btn = ui
                .add(
                    egui::Button::new(
                        RichText::new(tr("Voltar para os contatos", "Back to contacts"))
                            .size(11.0)
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                    )
                    .fill(Color32::from_rgb(230, 242, 253))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(175, 210, 242)))
                    .corner_radius(4)
                    .min_size(Vec2::new(160.0, 26.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if return_btn.clicked() {
                app.contact_filter = ContactFilter::All;
            }
        });
        return;
    }

    // 4. Request Cards
    for contact in contacts {
        let is_incoming = contact.state == ContactState::PendingIncoming;

        egui::Frame::new()
            .fill(Color32::from_rgb(255, 255, 255))
            .stroke(Stroke::new(1.0, Color32::from_rgb(212, 226, 240)))
            .corner_radius(4)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (avatar_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(42.0), egui::Sense::hover());
                    nmessenger::avatar_at(app, ui, &contact.user, avatar_rect, "perfil");

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&contact.user.display_name)
                                    .font(FontId::new(
                                        11.5,
                                        egui::FontFamily::Name("Segoe UI Bold".into()),
                                    ))
                                    .color(nmessenger::HEADING),
                            );

                            let (status_text, bg_color, fg_color, stroke_color) = if is_incoming {
                                (
                                    tr("Recebido", "Incoming"),
                                    Color32::from_rgb(235, 248, 238),
                                    Color32::from_rgb(30, 115, 45),
                                    Color32::from_rgb(180, 225, 190),
                                )
                            } else {
                                (
                                    tr("Enviado", "Outgoing"),
                                    Color32::from_rgb(243, 245, 248),
                                    nmessenger::MUTED,
                                    Color32::from_rgb(220, 225, 232),
                                )
                            };
                            let (badge_rect, _) =
                                ui.allocate_exact_size(Vec2::new(54.0, 15.0), egui::Sense::hover());
                            ui.painter().rect_filled(badge_rect, 3, bg_color);
                            ui.painter().rect_stroke(
                                badge_rect,
                                3,
                                Stroke::new(1.0, stroke_color),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                status_text,
                                FontId::proportional(9.0),
                                fg_color,
                            );
                        });

                        ui.label(
                            RichText::new(&contact.user.email)
                                .size(10.0)
                                .color(nmessenger::MUTED),
                        );

                        if !contact.user.personal_message.is_empty() {
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(format!("\"{}\"", contact.user.personal_message))
                                    .size(10.0)
                                    .italics()
                                    .color(Color32::from_rgb(90, 105, 120)),
                            );
                        }
                    });
                });

                ui.add_space(6.0);

                if is_incoming {
                    ui.horizontal(|ui| {
                        let accept_btn = theme::primary_button(
                            ui,
                            RichText::new(tr("Aceitar", "Accept")).size(10.5).strong(),
                            Vec2::new(70.0, 22.0),
                        );
                        if accept_btn.clicked() {
                            app.send(Action::Contact(
                                contact.user.id,
                                UpdateContactRequest {
                                    accepted: Some(true),
                                    ..Default::default()
                                },
                            ));
                        }

                        let decline_btn = theme::danger_button(
                            ui,
                            RichText::new(tr("Recusar", "Decline")).size(10.5),
                            Vec2::new(70.0, 22.0),
                        );
                        if decline_btn.clicked() {
                            app.send(Action::RemoveContact(contact.user.id));
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.menu_button("...", |ui| {
                                if ui.button(tr("Ver perfil...", "View profile...")).clicked() {
                                    app.contact_forms.view_profile = Some(contact.user.clone());
                                    ui.close();
                                }
                                if ui.button(tr("Bloquear contato", "Block contact")).clicked() {
                                    app.send(Action::Contact(
                                        contact.user.id,
                                        UpdateContactRequest {
                                            blocked: Some(true),
                                            ..Default::default()
                                        },
                                    ));
                                    ui.close();
                                }
                            });
                            if ui.link(tr("Perfil", "Profile")).clicked() {
                                app.contact_forms.view_profile = Some(contact.user.clone());
                            }
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(tr("Aguardando resposta...", "Awaiting response..."))
                                .size(10.0)
                                .italics()
                                .color(nmessenger::MUTED),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let cancel_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(tr("Cancelar", "Cancel"))
                                        .size(10.0)
                                        .color(nmessenger::MUTED),
                                )
                                .fill(Color32::from_rgb(248, 249, 251))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(220, 225, 230)))
                                .corner_radius(3)
                                .min_size(Vec2::new(60.0, 20.0)),
                            );
                            if cancel_btn.clicked() {
                                app.send(Action::RemoveContact(contact.user.id));
                            }
                        });
                    });
                }
            });

        ui.add_space(5.0);
    }
}

fn blocked_contacts_view(app: &mut MessengerApp, ui: &mut egui::Ui, contacts: &[Contact]) {
    // 1. Navigation toolbar
    egui::Frame::new()
        .fill(Color32::from_rgb(240, 246, 254))
        .stroke(Stroke::new(1.0, Color32::from_rgb(205, 225, 245)))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let back_clicked = ui
                    .add(
                        egui::Button::new(
                            RichText::new(tr("< Voltar para contatos", "< Back to contacts"))
                                .size(11.0)
                                .color(nmessenger::BLUE)
                                .strong(),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(tr(
                        "Retornar à lista principal de contatos (Esc)",
                        "Return to main contact list (Esc)",
                    ))
                    .clicked();

                if back_clicked || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    app.contact_filter = ContactFilter::All;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !contacts.is_empty() {
                        let badge_text =
                            format!("{} {}", contacts.len(), tr("bloqueado(s)", "blocked"));
                        ui.label(
                            RichText::new(badge_text)
                                .size(10.5)
                                .color(nmessenger::MUTED),
                        );
                    }
                });
            });
        });

    ui.add_space(6.0);

    // 2. Section Title
    ui.label(
        RichText::new(tr("Contatos bloqueados", "Blocked contacts"))
            .font(FontId::new(12.0, egui::FontFamily::Name("Segoe UI Bold".into())))
            .color(nmessenger::HEADING),
    );

    ui.add_space(3.0);
    let sep_rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            Pos2::new(sep_rect.left(), sep_rect.top()),
            Pos2::new(sep_rect.right(), sep_rect.top()),
        ],
        Stroke::new(1.0, Color32::from_rgb(225, 235, 245)),
    );
    ui.add_space(6.0);

    // 3. Empty state
    if contacts.is_empty() {
        ui.add_space(36.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(tr(
                    "Nenhum contato bloqueado",
                    "No blocked contacts",
                ))
                .font(FontId::new(12.0, egui::FontFamily::Name("Segoe UI Bold".into())))
                .color(nmessenger::HEADING),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr(
                    "Usuários bloqueados não podem enviar mensagens para você nem ver seu status.",
                    "Blocked users will not be able to send you messages or view your presence.",
                ))
                .size(10.5)
                .color(nmessenger::MUTED),
            );
            ui.add_space(16.0);
            let return_btn = ui
                .add(
                    egui::Button::new(
                        RichText::new(tr("Voltar para os contatos", "Back to contacts"))
                            .size(11.0)
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                    )
                    .fill(Color32::from_rgb(230, 242, 253))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(175, 210, 242)))
                    .corner_radius(4)
                    .min_size(Vec2::new(160.0, 26.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if return_btn.clicked() {
                app.contact_filter = ContactFilter::All;
            }
        });
        return;
    }

    // 4. Blocked contacts list
    for contact in contacts {
        egui::Frame::new()
            .fill(Color32::from_rgb(255, 255, 255))
            .stroke(Stroke::new(1.0, Color32::from_rgb(225, 230, 238)))
            .corner_radius(4)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (avatar_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                    nmessenger::avatar_at(app, ui, &contact.user, avatar_rect, "perfil");

                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&contact.user.display_name)
                                .font(FontId::new(
                                    11.5,
                                    egui::FontFamily::Name("Segoe UI Bold".into()),
                                ))
                                .color(nmessenger::HEADING),
                        );
                        ui.label(
                            RichText::new(&contact.user.email)
                                .size(10.0)
                                .color(nmessenger::MUTED),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let unblock_btn = theme::primary_button(
                            ui,
                            RichText::new(tr("Desbloquear", "Unblock")).size(10.5),
                            Vec2::new(76.0, 22.0),
                        );
                        if unblock_btn.clicked() {
                            app.send(Action::Contact(
                                contact.user.id,
                                UpdateContactRequest {
                                    blocked: Some(false),
                                    ..Default::default()
                                },
                            ));
                        }
                    });
                });
            });
        ui.add_space(4.0);
    }
}
