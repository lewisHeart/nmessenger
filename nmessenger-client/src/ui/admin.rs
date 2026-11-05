use super::{dialog, glass, nmessenger, scroll, theme};
use crate::{
    app::{admin::Mutation, MessengerApp},
    transport::Action,
};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Vec2};
use nm_protocol::*;
use zeroize::Zeroize;

pub fn refresh(app: &mut MessengerApp) {
    app.admin.error = None;
    if app.admin.tab == 0 {
        app.admin.overview_refreshed = Some(std::time::Instant::now());
    }
    if app.admin.tab == 3 {
        app.admin.busy = false;
        app.send(Action::Announcements);
    } else {
        app.admin.busy = true;
        app.send(Action::AdminLoad {
            tab: app.admin.tab,
            search: app.admin.search.clone(),
            offset: app.admin.offset,
        });
    }
}

pub fn open(app: &mut MessengerApp) {
    app.admin.open = true;
    app.admin.tab = 0;
    app.admin.offset = 0;
    app.admin.search.clear();
    refresh(app);
}

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.admin.open || !app.announcements.feed.can_manage {
        return;
    }
    if app.admin.tab == 0 && !app.admin.busy
        && app.admin.overview_refreshed.is_some_and(|time| time.elapsed() >= std::time::Duration::from_secs(30)) {
        refresh(app);
    }
    let alive = dialog::show(
        ctx,
        "app-admin-panel",
        "Painel do Administrador - NMessenger",
        [940.0, 680.0],
        |ui| {
            header_banner(app, ui);
            ui.add_space(8.0);

            tab_navigation(app, ui);
            ui.add_space(10.0);

            if let Some(error) = &app.admin.error {
                egui::Frame::new()
                    .fill(Color32::from_rgb(255, 235, 235))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(230, 160, 160)))
                    .corner_radius(6)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⚠️").size(14.0));
                            ui.colored_label(Color32::from_rgb(175, 45, 45), error);
                        });
                    });
                ui.add_space(8.0);
            }
            if !app.admin.notice.is_empty() {
                egui::Frame::new()
                    .fill(Color32::from_rgb(235, 250, 238))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(160, 220, 180)))
                    .corner_radius(6)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("OK")
                                    .strong()
                                    .color(Color32::from_rgb(40, 140, 60)),
                            );
                            ui.colored_label(Color32::from_rgb(30, 120, 50), &app.admin.notice);
                        });
                    });
                ui.add_space(8.0);
            }

            if app.admin.tab == 3 {
                super::announcements::content(app, ui);
            } else {
            scroll::vertical().id_salt("admin-data").auto_shrink([false, false]).show(ui, |ui| {
                confirm_card(app, ui);
                user_badges_modal(app, ui);
                if matches!(app.admin.tab, 1 | 2) {
                    search_bar(app, ui);
                    ui.add_space(10.0);
                }
                match app.admin.tab {
                        0 => overview(app, ui),
                        1 => users(app, ui),
                        2 => sessions(app, ui),
                        4 => audit(app, ui),
                        _ => badges(app, ui),
                    }
            });
            }
            false
        },
    );
    if !alive {
        app.admin.open = false;
        app.admin.password.zeroize();
        app.admin.new_password.zeroize();
        app.admin.pending = None;
    }
}

fn header_banner(app: &mut MessengerApp, ui: &mut egui::Ui) {
    if crate::platform::is_mobile() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Administração").size(16.0).strong().color(nmessenger::HEADING));
            if ui.add_enabled(!app.admin.busy, egui::Button::new("Atualizar")).clicked() { refresh(app); }
            if app.admin.busy { ui.spinner(); }
        });
        return;
    }
    let header_frame = egui::Frame::new()
        .fill(Color32::from_rgb(243, 248, 255))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(205, 224, 245)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 10));

    if crate::platform::is_mobile() {
        header_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(icon_rect, 6, Color32::from_rgb(222, 238, 255));
                nmessenger::paint(
                    ui.ctx(),
                    ui.painter(),
                    "icons/settings.png",
                    icon_rect.shrink(5.0),
                );

                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Administração")
                            .size(15.0)
                            .strong()
                            .color(nmessenger::HEADING),
                    );
                    ui.label(
                        RichText::new("Servidor e Contas")
                            .size(10.0)
                            .color(nmessenger::MUTED),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let refresh_btn = theme::primary_button(
                        ui,
                        RichText::new("🔄")
                            .size(13.0)
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                        Vec2::new(36.0, 32.0),
                    );
                    if refresh_btn.clicked() && !app.admin.busy {
                        refresh(app);
                    }
                    if app.admin.busy {
                        ui.spinner();
                    }
                });
            });
        });
        return;
    }

    header_frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(38.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(icon_rect, 7, Color32::from_rgb(222, 238, 255));
            ui.painter().rect_stroke(
                icon_rect,
                7,
                egui::Stroke::new(1.0, Color32::from_rgb(160, 205, 248)),
                egui::StrokeKind::Inside,
            );
            nmessenger::paint(
                ui.ctx(),
                ui.painter(),
                "icons/settings.png",
                icon_rect.shrink(6.0),
            );

            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Painel do Administrador")
                            .size(18.0)
                            .strong()
                            .color(nmessenger::HEADING),
                    );
                });
                ui.label(
                    RichText::new(
                        "Indicadores do servidor, gerenciamento de contas, sessões ativas e anúncios.",
                    )
                    .size(11.0)
                    .color(nmessenger::MUTED),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let refresh_btn = theme::primary_button(
                    ui,
                    RichText::new("🔄 Atualizar")
                        .size(11.5)
                        .color(Color32::from_rgb(1, 45, 147))
                        .strong(),
                    Vec2::new(95.0, 30.0),
                );
                if refresh_btn.clicked() && !app.admin.busy {
                    refresh(app);
                }
                if app.admin.busy {
                    ui.spinner();
                    ui.add_space(6.0);
                }
            });
        });
    });
}

fn badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32, border: Color32) {
    let font = egui::FontId::proportional(10.5);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, fg);
    let size = Vec2::new(galley.size().x + 14.0, 19.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 9, bg);
    ui.painter().rect_stroke(
        rect,
        9,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        egui::pos2(rect.min.x + 7.0, rect.center().y - galley.size().y / 2.0),
        galley,
        Color32::TRANSPARENT,
    );
}

fn tab_navigation(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let nav_frame = egui::Frame::new()
        .fill(Color32::from_rgb(236, 243, 250))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(212, 226, 240)))
        .corner_radius(7)
        .inner_margin(4);

    if crate::platform::is_mobile() {
        scroll::horizontal().id_salt("admin-mobile-tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (index, title) in [
                    (0, "📊 Visão geral"),
                    (1, "👥 Usuários"),
                    (2, "💻 Sessões"),
                    (3, "📢 Anúncios"),
                    (4, "⚖️ Auditoria"),
                    (5, "🏆 Selos"),
                ] {
                    let is_selected = app.admin.tab == index;
                    let text = RichText::new(title)
                        .size(12.0)
                        .strong()
                        .color(if is_selected {
                            Color32::WHITE
                        } else {
                            nmessenger::HEADING
                        });
                    let btn = egui::Button::new(text)
                        .fill(if is_selected {
                            nmessenger::BLUE
                        } else {
                            Color32::from_rgb(235, 244, 252)
                        })
                        .stroke(if is_selected {
                            egui::Stroke::NONE
                        } else {
                            egui::Stroke::new(1.0, Color32::from_rgb(205, 224, 242))
                        })
                        .corner_radius(16)
                        .min_size(Vec2::new(0.0, 32.0));
                    let resp = ui.add(btn);
                    if resp.clicked() {
                        app.admin.tab = index;
                        app.admin.offset = 0;
                        app.admin.pending = None;
                        app.admin.password.zeroize();
                        app.admin.new_password.zeroize();
                        app.admin.search.clear();
                        refresh(app);
                    }
                }
            });
        });
        return;
    }
    nav_frame.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (index, icon_path, title) in [
                (0, "icons/settings.png", "Visão geral"),
                (1, "icons/person.png", "Usuários"),
                (2, "emojis/door.png", "Sessões"),
                (3, "icons/information.png", "Gerenciar anúncios"),
                (4, "emojis/hammer.png", "Auditoria"),
                (5, "emojis/trophy.png", "Selos e conquistas"),
            ] {
                let is_selected = app.admin.tab == index;
                let text = RichText::new(title)
                    .size(11.5)
                    .strong()
                    .color(if is_selected {
                        Color32::from_rgb(1, 45, 147)
                    } else {
                        Color32::from_rgb(70, 90, 115)
                    });
                let icon_tex = nmessenger::texture(ui.ctx(), icon_path)
                    .map(|t| egui::load::SizedTexture::new(t.id(), Vec2::splat(15.0)));
                let btn = if let Some(st) = icon_tex {
                    egui::Button::image_and_text(st, text)
                } else {
                    egui::Button::new(text)
                }
                .fill(if is_selected {
                    Color32::WHITE
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(if is_selected {
                    egui::Stroke::new(1.0, Color32::from_rgb(130, 185, 240))
                } else {
                    egui::Stroke::NONE
                })
                .corner_radius(5)
                .min_size(Vec2::new(0.0, 28.0));

                if ui.add(btn).clicked() {
                    app.admin.tab = index;
                    app.admin.offset = 0;
                    app.admin.pending = None;
                    app.admin.password.zeroize();
                    app.admin.new_password.zeroize();
                    app.admin.search.clear();
                    refresh(app);
                }
            }
        });
    });
}

fn search_bar(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let is_mob = crate::platform::is_mobile();
    ui.horizontal(|ui| {
        let frame = egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(195, 215, 235)))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 5));
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let (search_icon_rect, _) =
                    ui.allocate_exact_size(Vec2::splat(15.0), egui::Sense::hover());
                nmessenger::paint(ui.ctx(), ui.painter(), "icons/search.png", search_icon_rect);
                ui.add_space(4.0);
                let edit_w = if is_mob {
                    (ui.available_width() - 85.0).clamp(50.0, 240.0)
                } else {
                    (ui.available_width() - 110.0).clamp(80.0, 320.0)
                };
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut app.admin.search)
                        .hint_text("Buscar...")
                        .desired_width(edit_w)
                        .frame(egui::Frame::NONE),
                );
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    app.admin.offset = 0;
                    refresh(app);
                }
            });
        });

        ui.add_space(4.0);
        let btn = theme::primary_button(
            ui,
            RichText::new("Buscar")
                .size(11.5)
                .color(Color32::from_rgb(1, 45, 147))
                .strong(),
            Vec2::new(if is_mob { 60.0 } else { 72.0 }, 28.0),
        );
        if btn.clicked() && !app.admin.busy {
            app.admin.offset = 0;
            refresh(app);
        }

        if !app.admin.search.is_empty() && ui.button("Limpar").clicked() {
            app.admin.search.clear();
            app.admin.offset = 0;
            refresh(app);
        }
    });
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    let is_mob = crate::platform::is_mobile();
    let margin = if is_mob { 10 } else { 12 };
    let frame = egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(220, 233, 245)))
        .corner_radius(8)
        .inner_margin(margin);
    frame.show(ui, |ui| {
        let w = ui.available_width();
        ui.set_max_width(w);
        ui.set_width(w);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        glass::gloss_line(ui);
        content(ui);
    });
    ui.add_space(8.0);
}

fn kpi_card(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    subtitle: &str,
    icon: &str,
    accent: Color32,
    outer_width: f32,
) {
    let margin = 10;
    let frame = egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(218, 232, 245)))
        .corner_radius(8)
        .inner_margin(margin);

    let inner_width = (outer_width - (margin as f32 * 2.0)).max(60.0);
    frame.show(ui, |ui| {
        ui.set_max_width(inner_width);
        ui.set_width(inner_width);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(14.0));
                ui.label(RichText::new(title).size(10.5).color(nmessenger::MUTED));
            });
            ui.add_space(4.0);
            ui.label(RichText::new(value).size(22.0).strong().color(accent));
            ui.add_space(2.0);
            ui.label(
                RichText::new(subtitle)
                    .size(10.0)
                    .color(nmessenger::HEADING),
            );
        });
    });
}

fn overview(app: &MessengerApp, ui: &mut egui::Ui) {
    let Some(stats) = &app.admin.overview else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.label(RichText::new("Carregando indicadores do servidor…").color(nmessenger::MUTED));
        });
        return;
    };

    ui.add_space(4.0);
    let is_mob = crate::platform::is_mobile();
    let spacing_x = ui.spacing().item_spacing.x;
    let card_width = if is_mob {
        ((ui.available_width() - spacing_x - 4.0) / 2.0).floor().max(90.0)
    } else {
        ((ui.available_width() - spacing_x * 2.0 - 4.0) / 3.0).floor().max(140.0)
    };

    // Row 1 of KPI cards
    ui.horizontal_wrapped(|ui| {
        kpi_card(
            ui,
            "Usuários cadastrados",
            &stats.users.to_string(),
            "Total de contas registradas",
            "👥",
            Color32::from_rgb(1, 45, 147),
            card_width,
        );
        kpi_card(
            ui,
            "Sessões ativas",
            &stats.active_sessions.to_string(),
            "Sessões não expiradas",
            "⚡",
            Color32::from_rgb(30, 130, 45),
            card_width,
        );
        kpi_card(
            ui,
            "Conversas criadas",
            &stats.conversations.to_string(),
            "Canais diretos e grupos",
            "💬",
            Color32::from_rgb(0, 110, 200),
            card_width,
        );
    });

    ui.add_space(8.0);

    // Row 2 of KPI cards
    ui.horizontal_wrapped(|ui| {
        kpi_card(
            ui,
            "Mensagens trocadas",
            &stats.messages.to_string(),
            "Histórico completo mantido",
            "✉️",
            Color32::from_rgb(70, 40, 160),
            card_width,
        );
        kpi_card(
            ui,
            "Arquivos e mídias",
            &stats.files.to_string(),
            &format!(
                "{:.1} MiB armazenados",
                stats.stored_bytes as f64 / 1048576.0
            ),
            "📁",
            Color32::from_rgb(200, 100, 20),
            card_width,
        );
        kpi_card(
            ui,
            "Contas suspensas",
            &stats.suspended_users.to_string(),
            if stats.suspended_users == 0 {
                "Nenhuma restrição ativa"
            } else {
                "Contas sob bloqueio"
            },
            "⛔",
            if stats.suspended_users > 0 {
                Color32::from_rgb(190, 40, 40)
            } else {
                Color32::from_rgb(100, 110, 125)
            },
            card_width,
        );
    });

    ui.add_space(14.0);

    // System Information & Storage Policy
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Armazenamento e Políticas do Servidor")
                    .size(13.5)
                    .strong()
                    .color(nmessenger::HEADING),
            );
        });
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Espaço em disco utilizado:")
                    .size(11.5)
                    .color(nmessenger::MUTED),
            );
            ui.label(
                RichText::new(format!(
                    "{:.2} MiB",
                    stats.stored_bytes as f64 / 1048576.0
                ))
                .size(11.5)
                .strong()
                .color(nmessenger::INK),
            );
            ui.label(RichText::new("· Cota por usuário:").size(11.5).color(nmessenger::MUTED));
            ui.label(
                RichText::new(format!(
                    "{:.0} MiB",
                    stats.user_upload_quota as f64 / 1048576.0
                ))
                .size(11.5)
                .strong()
                .color(nmessenger::INK),
            );
            ui.label(RichText::new("· Duração da sessão:").size(11.5).color(nmessenger::MUTED));
            ui.label(
                RichText::new(format!("{} dias", stats.session_ttl_secs / 86400))
                    .size(11.5)
                    .strong()
                    .color(nmessenger::INK),
            );
        });

        ui.add_space(6.0);
        ui.label(format!("Anúncios publicados: {}", stats.published_announcements));
        ui.label(
            RichText::new(
                "Todas as ações de criação, alteração de permissões e suspensão são registradas na aba Auditoria.",
            )
            .size(10.5)
            .color(nmessenger::MUTED),
        );
    });
}

fn users(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(page) = app.admin.users.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.label(RichText::new("Carregando lista de usuários…").color(nmessenger::MUTED));
        });
        return;
    };
    if page.items.is_empty() {
        card(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("Nenhum usuário encontrado").strong());
                ui.label(
                    RichText::new("Tente alterar o termo de busca.")
                        .small()
                        .color(nmessenger::MUTED),
                );
                ui.add_space(20.0);
            });
        });
        return;
    }
    let me = app.data.as_ref().map(|data| data.me.id);
    for user in &page.items {
        let is_me = Some(user.id) == me;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let (avatar_rect, _) =
                    ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
                let initial = user
                    .display_name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let bg_color = if user.administrator {
                    Color32::from_rgb(238, 228, 255)
                } else if user.disabled_at.is_some() {
                    Color32::from_rgb(255, 230, 230)
                } else {
                    Color32::from_rgb(228, 242, 255)
                };
                ui.painter().circle_filled(avatar_rect.center(), 20.0, bg_color);
                ui.painter().circle_stroke(
                    avatar_rect.center(),
                    20.0,
                    egui::Stroke::new(
                        1.0,
                        if user.administrator {
                            Color32::from_rgb(175, 145, 230)
                        } else if user.disabled_at.is_some() {
                            Color32::from_rgb(230, 140, 140)
                        } else {
                            Color32::from_rgb(160, 205, 245)
                        },
                    ),
                );
                ui.painter().text(
                    avatar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initial,
                    egui::FontId::new(17.0, egui::FontFamily::Name("Segoe UI Bold".into())),
                    if user.administrator {
                        Color32::from_rgb(85, 35, 150)
                    } else if user.disabled_at.is_some() {
                        Color32::from_rgb(180, 40, 40)
                    } else {
                        Color32::from_rgb(20, 80, 160)
                    },
                );

                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(&user.display_name)
                                .size(13.5)
                                .strong()
                                .color(nmessenger::INK),
                        );
                        if user.administrator {
                            badge(
                                ui,
                                "🛡 Administrador",
                                Color32::from_rgb(240, 232, 255),
                                Color32::from_rgb(90, 40, 160),
                                Color32::from_rgb(205, 185, 245),
                            );
                        }
                        if user.banned {
                            badge(ui, "⛔ Banido", Color32::from_rgb(255, 235, 235), Color32::from_rgb(180, 40, 40), Color32::from_rgb(245, 190, 190));
                        } else if user.disabled_at.is_some() {
                            badge(
                                ui,
                                "⛔ Suspenso",
                                Color32::from_rgb(255, 235, 235),
                                Color32::from_rgb(180, 40, 40),
                                Color32::from_rgb(245, 190, 190),
                            );
                        } else {
                            badge(
                                ui,
                                "Ativo",
                                Color32::from_rgb(235, 250, 235),
                                Color32::from_rgb(30, 130, 40),
                                Color32::from_rgb(180, 230, 180),
                            );
                        }
                        if is_me {
                            badge(
                                ui,
                                "Você",
                                Color32::from_rgb(230, 240, 255),
                                Color32::from_rgb(30, 80, 180),
                                Color32::from_rgb(180, 205, 245),
                            );
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&user.email).size(11.0).color(nmessenger::HEADING));
                        ui.label(RichText::new("·").color(nmessenger::MUTED));
                        ui.label(
                            RichText::new(format!("{} sessão(ões)", user.active_sessions))
                                .size(10.5)
                                .color(nmessenger::MUTED),
                        );
                        ui.label(RichText::new("·").color(nmessenger::MUTED));
                        ui.label(
                            RichText::new(format!(
                                "{:.1} MiB",
                                user.stored_bytes as f64 / 1048576.0
                            ))
                            .size(10.5)
                            .color(nmessenger::MUTED),
                        );
                        ui.label(RichText::new("·").color(nmessenger::MUTED));
                        ui.label(
                            RichText::new(format!(
                                "Cadastro: {}",
                                user.created_at.format("%d/%m/%Y")
                            ))
                            .size(10.5)
                            .color(nmessenger::MUTED),
                        );
                    });

                    if !user.badges.is_empty() {
                        ui.add_space(3.0);
                        ui.horizontal_wrapped(|ui| {
                            for b in &user.badges {
                                badge(
                                    ui,
                                    &b.name,
                                    Color32::from_rgb(255, 250, 235),
                                    Color32::from_rgb(160, 95, 10),
                                    Color32::from_rgb(240, 220, 160),
                                );
                            }
                        });
                    }
                });

            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                    let enabled = !is_me && !app.admin.busy;

                    let badge_btn = egui::Button::new(
                        RichText::new("Gerenciar Selos")
                            .size(11.0)
                            .color(Color32::from_rgb(140, 80, 10)),
                    )
                    .fill(Color32::from_rgb(255, 250, 240))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(245, 215, 150)))
                    .corner_radius(4)
                    .min_size(Vec2::new(105.0, 26.0));
                    if ui.add_enabled(!app.admin.busy, badge_btn).clicked() {
                        app.admin.managing_user_badges = Some(user.clone());
                        if app.admin.badges.is_none() {
                            app.send(Action::AdminLoad {
                                tab: 5,
                                search: String::new(),
                                offset: 0,
                            });
                        }
                    }

                    let role_label = if user.administrator {
                        "Remover Admin"
                    } else {
                        "Tornar Admin"
                    };
                    let role_btn = egui::Button::new(RichText::new(role_label).size(11.0))
                        .fill(Color32::from_rgb(248, 245, 255))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(215, 200, 245)))
                        .corner_radius(4)
                        .min_size(Vec2::new(96.0, 26.0));
                    if ui.add_enabled(enabled, role_btn).clicked() {
                        app.admin.pending = Some(Mutation::UserRole {
                            user: user.id,
                            email: user.email.clone(),
                            administrator: !user.administrator,
                        });
                    }

                    let sess_btn = egui::Button::new(RichText::new("Encerrar Sessões").size(11.0))
                        .fill(Color32::from_rgb(245, 248, 252))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(210, 225, 240)))
                        .corner_radius(4)
                        .min_size(Vec2::new(108.0, 26.0));
                    if ui.add_enabled(enabled, sess_btn).clicked() {
                        app.admin.pending = Some(Mutation::RevokeUser {
                            user: user.id,
                            email: user.email.clone(),
                        });
                    }
                    if ui.add_enabled(enabled, egui::Button::new("Redefinir senha")).clicked() {
                        app.admin.pending = Some(Mutation::ResetPassword { user: user.id, email: user.email.clone() });
                    }
                    if ui.add_enabled(enabled, egui::Button::new(if user.banned { "Remover banimento" } else { "Banir usuário" })).clicked() {
                        app.admin.pending = Some(Mutation::ModerateUser { user: user.id, email: user.email.clone(), banned: Some(!user.banned) });
                    }
                    if ui.add_enabled(enabled && !user.banned, egui::Button::new("Excluir conta")).on_hover_text("O histórico compartilhado será preservado").clicked() {
                        app.admin.pending = Some(Mutation::ModerateUser { user: user.id, email: user.email.clone(), banned: None });
                    }

                    let status_label = if user.disabled_at.is_some() {
                        "Reativar"
                    } else {
                        "Suspender"
                    };
                    let status_color = if user.disabled_at.is_some() {
                        Color32::from_rgb(40, 140, 50)
                    } else {
                        Color32::from_rgb(180, 45, 45)
                    };
                    let status_btn = egui::Button::new(
                        RichText::new(status_label).size(11.0).color(status_color),
                    )
                    .fill(if user.disabled_at.is_some() {
                        Color32::from_rgb(238, 252, 240)
                    } else {
                        Color32::from_rgb(255, 242, 242)
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if user.disabled_at.is_some() {
                            Color32::from_rgb(185, 230, 195)
                        } else {
                            Color32::from_rgb(245, 195, 195)
                        },
                    ))
                    .corner_radius(4)
                    .min_size(Vec2::new(76.0, 26.0));
                    if ui.add_enabled(enabled && !user.banned, status_btn).clicked() {
                        app.admin.pending = Some(Mutation::UserStatus {
                            user: user.id,
                            email: user.email.clone(),
                            disabled: user.disabled_at.is_none(),
                        });
                    }
            });
        });
    }
    pagination(app, ui, page.total, page.items.len());
}

fn sessions(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(page) = app.admin.sessions.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.label(RichText::new("Carregando sessões ativas…").color(nmessenger::MUTED));
        });
        return;
    };
    if page.items.is_empty() {
        card(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("Nenhuma sessão encontrada").strong());
                ui.label(
                    RichText::new("Tente alterar o termo de busca.")
                        .small()
                        .color(nmessenger::MUTED),
                );
                ui.add_space(20.0);
            });
        });
        return;
    }
    for session in &page.items {
        card(ui, |ui| {
            if crate::platform::is_mobile() {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let (icon_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(icon_rect, 6, Color32::from_rgb(235, 244, 252));
                        ui.painter().rect_stroke(
                            icon_rect,
                            6,
                            egui::Stroke::new(1.0, Color32::from_rgb(195, 220, 245)),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            icon_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "💻",
                            egui::FontId::proportional(15.0),
                            nmessenger::INK,
                        );

                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(&session.email)
                                        .size(13.0)
                                        .strong()
                                        .color(nmessenger::INK),
                                );
                                if session.current {
                                    badge(
                                        ui,
                                        "Esta Sessão",
                                        Color32::from_rgb(230, 245, 255),
                                        Color32::from_rgb(0, 102, 204),
                                        Color32::from_rgb(175, 215, 250),
                                    );
                                }
                            });
                            ui.label(
                                RichText::new(&session.device_name)
                                    .size(11.0)
                                    .color(nmessenger::HEADING),
                            );
                        });
                    });

                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Última: {}",
                                session
                                    .last_seen_at
                                    .with_timezone(&chrono::Local)
                                    .format("%d/%m/%Y %H:%M")
                            ))
                            .size(10.0)
                            .color(nmessenger::MUTED),
                        );
                        ui.label(RichText::new("·").color(nmessenger::MUTED));
                        ui.label(
                            RichText::new(format!(
                                "Expira: {}",
                                session
                                    .expires_at
                                    .with_timezone(&chrono::Local)
                                    .format("%d/%m/%Y %H:%M")
                            ))
                            .size(10.0)
                            .color(nmessenger::MUTED),
                        );
                    });

                    if !session.current {
                        ui.add_space(6.0);
                        let btn = egui::Button::new(
                            RichText::new("Revogar Sessão")
                                .size(11.0)
                                .color(Color32::from_rgb(180, 45, 45)),
                        )
                        .fill(Color32::from_rgb(255, 242, 242))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(245, 195, 195)))
                        .corner_radius(4)
                        .min_size(Vec2::new(120.0, 30.0));
                        if ui.add_enabled(!app.admin.busy, btn).clicked() {
                            app.admin.pending = Some(Mutation::RevokeSession {
                                session: session.id,
                                email: session.email.clone(),
                            });
                        }
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(icon_rect, 6, Color32::from_rgb(235, 244, 252));
                    ui.painter().rect_stroke(
                        icon_rect,
                        6,
                        egui::Stroke::new(1.0, Color32::from_rgb(195, 220, 245)),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "💻",
                        egui::FontId::proportional(16.0),
                        nmessenger::INK,
                    );

                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&session.email)
                                    .size(13.0)
                                    .strong()
                                    .color(nmessenger::INK),
                            );
                            if session.current {
                                badge(
                                    ui,
                                    "Esta Sessão",
                                    Color32::from_rgb(230, 245, 255),
                                    Color32::from_rgb(0, 102, 204),
                                    Color32::from_rgb(175, 215, 250),
                                );
                            }
                        });
                        ui.label(
                            RichText::new(&session.device_name)
                                .size(11.0)
                                .color(nmessenger::HEADING),
                        );
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Última atividade: {}",
                                    session
                                        .last_seen_at
                                        .with_timezone(&chrono::Local)
                                        .format("%d/%m/%Y %H:%M")
                                ))
                                .size(10.0)
                                .color(nmessenger::MUTED),
                            );
                            ui.label(RichText::new("·").color(nmessenger::MUTED));
                            ui.label(
                                RichText::new(format!(
                                    "Expira em: {}",
                                    session
                                        .expires_at
                                        .with_timezone(&chrono::Local)
                                        .format("%d/%m/%Y %H:%M")
                                ))
                                .size(10.0)
                                .color(nmessenger::MUTED),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !session.current {
                            let btn = egui::Button::new(
                                RichText::new("Revogar Sessão")
                                    .size(11.0)
                                    .color(Color32::from_rgb(180, 45, 45)),
                            )
                            .fill(Color32::from_rgb(255, 242, 242))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(245, 195, 195)))
                            .corner_radius(4)
                            .min_size(Vec2::new(110.0, 28.0));
                            if ui.add_enabled(!app.admin.busy, btn).clicked() {
                                app.admin.pending = Some(Mutation::RevokeSession {
                                    session: session.id,
                                    email: session.email.clone(),
                                });
                            }
                        }
                    });
                });
            }
        });
    }
    pagination(app, ui, page.total, page.items.len());
}

fn audit(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(page) = app.admin.audit.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.label(RichText::new("Carregando registro de auditoria…").color(nmessenger::MUTED));
        });
        return;
    };
    if page.items.is_empty() {
        card(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("Nenhum evento registrado").strong());
                ui.label(
                    RichText::new(
                        "As ações de segurança e administração aparecerão aqui automaticamente.",
                    )
                    .small()
                    .color(nmessenger::MUTED),
                );
                ui.add_space(20.0);
            });
        });
        return;
    }
    for entry in &page.items {
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let (icon, bg, fg, border) = match entry.action.as_str() {
                    a if a.contains("ROLE") || a.contains("ADMIN") => (
                        "🛡",
                        Color32::from_rgb(240, 232, 255),
                        Color32::from_rgb(90, 40, 160),
                        Color32::from_rgb(205, 185, 245),
                    ),
                    a if a.contains("STATUS")
                        || a.contains("DISABLE")
                        || a.contains("SUSPEND") =>
                    {
                        (
                            "⛔",
                            Color32::from_rgb(255, 235, 235),
                            Color32::from_rgb(180, 40, 40),
                            Color32::from_rgb(245, 190, 190),
                        )
                    }
                    a if a.contains("REVOKE") || a.contains("SESSION") => (
                        "🔑",
                        Color32::from_rgb(255, 248, 230),
                        Color32::from_rgb(180, 110, 20),
                        Color32::from_rgb(240, 215, 160),
                    ),
                    _ => (
                        "📝",
                        Color32::from_rgb(235, 245, 255),
                        Color32::from_rgb(30, 90, 170),
                        Color32::from_rgb(190, 220, 245),
                    ),
                };
                badge(ui, &format!("{icon} {}", entry.action), bg, fg, border);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(
                            entry
                                .created_at
                                .with_timezone(&chrono::Local)
                                .format("%d/%m/%Y %H:%M:%S")
                                .to_string(),
                        )
                        .size(10.5)
                        .color(nmessenger::MUTED),
                    );
                });
            });

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("Administrador:")
                        .size(11.0)
                        .color(nmessenger::MUTED),
                );
                ui.label(
                    RichText::new(&entry.actor_email)
                        .size(11.5)
                        .strong()
                        .color(nmessenger::INK),
                );
                if let Some(user) = entry.target_user {
                    ui.label(RichText::new("· Alvo:").size(11.0).color(nmessenger::MUTED));
                    ui.label(
                        RichText::new(user.to_string())
                            .size(10.5)
                            .color(nmessenger::HEADING),
                    );
                }
                if let Some(session) = entry.target_session {
                    ui.label(
                        RichText::new("· Sessão:")
                            .size(11.0)
                            .color(nmessenger::MUTED),
                    );
                    ui.label(
                        RichText::new(session.to_string())
                            .size(10.5)
                            .color(nmessenger::HEADING),
                    );
                }
            });

            if !entry.details.is_null() && entry.details != serde_json::json!({}) {
                ui.add_space(4.0);
                let details_str = serde_json::to_string(&entry.details)
                    .unwrap_or_else(|_| entry.details.to_string());
                egui::Frame::new()
                    .fill(Color32::from_rgb(246, 249, 252))
                    .stroke(egui::Stroke::new(0.5, Color32::from_rgb(225, 235, 245)))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(details_str)
                                .size(10.0)
                                .monospace()
                                .color(nmessenger::HEADING),
                        );
                    });
            }
        });
    }
    pagination(app, ui, page.total, page.items.len());
}

fn pagination(app: &mut MessengerApp, ui: &mut egui::Ui, total: i64, count: usize) {
    ui.add_space(4.0);
    let start = if count == 0 {
        0
    } else {
        app.admin.offset + 1
    };
    let end = (app.admin.offset + count as i64).min(total);
    if crate::platform::is_mobile() {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(format!("Exibindo {start}–{end} de {total} registros"))
                    .size(11.0)
                    .color(nmessenger::MUTED),
            );
            ui.add_space(4.0);
            let btn_w = ((ui.available_width() - 8.0) / 2.0).max(80.0);
            ui.horizontal(|ui| {
                let prev_enabled = app.admin.offset > 0 && !app.admin.busy;
                if ui
                    .add_enabled(
                        prev_enabled,
                        egui::Button::new("‹ Anterior").min_size(Vec2::new(btn_w, 32.0)),
                    )
                    .clicked()
                {
                    app.admin.offset = (app.admin.offset - 25).max(0);
                    refresh(app);
                }
                let next_enabled = app.admin.offset + 25 < total && !app.admin.busy;
                if ui
                    .add_enabled(
                        next_enabled,
                        egui::Button::new("Próxima ›").min_size(Vec2::new(btn_w, 32.0)),
                    )
                    .clicked()
                {
                    app.admin.offset += 25;
                    refresh(app);
                }
            });
        });
    } else {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Exibindo {start}–{end} de {total} registros"))
                    .size(11.5)
                    .color(nmessenger::MUTED),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let next_enabled = app.admin.offset + 25 < total && !app.admin.busy;
                if ui
                    .add_enabled(
                        next_enabled,
                        egui::Button::new("Próxima ›").min_size(Vec2::new(75.0, 26.0)),
                    )
                    .clicked()
                {
                    app.admin.offset += 25;
                    refresh(app);
                }
                let prev_enabled = app.admin.offset > 0 && !app.admin.busy;
                if ui
                    .add_enabled(
                        prev_enabled,
                        egui::Button::new("‹ Anterior").min_size(Vec2::new(75.0, 26.0)),
                    )
                    .clicked()
                {
                    app.admin.offset = (app.admin.offset - 25).max(0);
                    refresh(app);
                }
            });
        });
    }
}

fn confirm_card(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(pending) = app.admin.pending.clone() else {
        return;
    };
    let frame = egui::Frame::new()
        .fill(Color32::from_rgb(255, 251, 235))
        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(235, 185, 70)))
        .corner_radius(8)
        .inner_margin(14);

    frame.show(ui, |ui| {
        let (prompt, action_label, is_danger) = match &pending {
            Mutation::UserStatus {
                email, disabled, ..
            } => (
                format!(
                    "Confirma {} a conta de {}?",
                    if *disabled {
                        "suspender"
                    } else {
                        "reativar"
                    },
                    email
                ),
                if *disabled {
                    "Suspender Conta"
                } else {
                    "Reativar Conta"
                },
                *disabled,
            ),
            Mutation::UserRole {
                email,
                administrator,
                ..
            } => (
                format!(
                    "Confirma {} privilégios de Administrador para {}?",
                    if *administrator {
                        "conceder"
                    } else {
                        "remover"
                    },
                    email
                ),
                if *administrator {
                    "Conceder Administrador"
                } else {
                    "Remover Administrador"
                },
                false,
            ),
            Mutation::RevokeUser { email, .. } => (
                format!("Confirma encerrar todas as sessões ativas de {}?", email),
                "Encerrar Todas as Sessões",
                true,
            ),
            Mutation::ResetPassword { email, .. } => (
                format!("Redefinir a senha de {email} e encerrar suas sessões? Para manter as chaves locais, o usuário precisará da senha anterior neste dispositivo."),
                "Redefinir senha",
                true,
            ),
            Mutation::ModerateUser { email, banned, .. } => match banned {
                Some(true) => (format!("Banir {email} e impedir novos cadastros com este e-mail?"), "Banir usuário", true),
                Some(false) => (format!("Remover o banimento de {email}?"), "Remover banimento", false),
                None => (format!("Excluir a conta de {email}? O histórico compartilhado será preservado com identificação anônima."), "Excluir conta", true),
            },
            Mutation::RevokeSession { email, .. } => (
                format!("Confirma revogar esta sessão ativa de {}?", email),
                "Revogar Sessão",
                true,
            ),
            Mutation::DeleteBadge { name, .. } => (
                format!("Confirma excluir permanentemente o selo '{}'? Ele será removido de todos os usuários.", name),
                "Excluir Selo",
                true,
            ),
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new("⚠️").size(20.0));
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Confirmação de Ação Administrativa")
                        .size(13.5)
                        .strong()
                        .color(Color32::from_rgb(140, 80, 0)),
                );
                ui.label(RichText::new(prompt).size(12.0).color(nmessenger::INK));
            });
        });

        let needs_password = matches!(pending, Mutation::UserRole { .. } | Mutation::ResetPassword { .. } | Mutation::ModerateUser { .. });
        let reset_password = matches!(pending, Mutation::ResetPassword { .. });
        if needs_password {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Sua senha de administrador:")
                        .size(11.5)
                        .strong()
                        .color(nmessenger::INK),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.admin.password)
                        .password(true)
                        .hint_text("Digite sua senha para confirmar")
                        .desired_width(240.0),
                );
            });
        }

        if reset_password {
            ui.horizontal(|ui| {
                ui.label("Nova senha:");
                ui.add(egui::TextEdit::singleline(&mut app.admin.new_password).password(true).desired_width(240.0));
            });
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let can_confirm =
                !app.admin.busy && (!needs_password || !app.admin.password.is_empty()) && (!reset_password || !app.admin.new_password.is_empty());
            let clicked = ui.add_enabled_ui(can_confirm, |ui| {
                if is_danger {
                    theme::danger_button(
                        ui,
                        RichText::new(action_label).size(11.5).strong(),
                        Vec2::new(170.0, 30.0),
                    ).clicked()
                } else {
                    theme::primary_button(
                        ui,
                        RichText::new(action_label)
                            .size(11.5)
                            .strong()
                            .color(Color32::from_rgb(1, 45, 147)),
                        Vec2::new(170.0, 30.0),
                    ).clicked()
                }
            }).inner;
            if clicked {
                let action = match pending {
                    Mutation::UserStatus { user, disabled, .. } => Action::AdminUpdateUser(
                        user,
                        AdminUpdateUser {
                            disabled: Some(disabled),
                            administrator: None,
                            confirmation_password: None,
                        },
                    ),
                    Mutation::UserRole {
                        user,
                        administrator,
                        ..
                    } => Action::AdminUpdateUser(
                        user,
                        AdminUpdateUser {
                            disabled: None,
                            administrator: Some(administrator),
                            confirmation_password: Some(std::mem::take(&mut app.admin.password)),
                        },
                    ),
                    Mutation::RevokeUser { user, .. } => Action::AdminRevokeUser(user),
                    Mutation::ResetPassword { user, .. } => Action::AdminResetPassword(user, std::mem::take(&mut app.admin.new_password), std::mem::take(&mut app.admin.password)),
                    Mutation::ModerateUser { user, banned, .. } => Action::AdminModerateUser(user, banned, std::mem::take(&mut app.admin.password)),
                    Mutation::RevokeSession { session, .. } => Action::AdminRevokeSession(session),
                    Mutation::DeleteBadge { id, .. } => Action::AdminDeleteBadge(id),
                };
                app.admin.busy = true;
                app.admin.error = None;
                app.admin.notice.clear();
                app.send(action);
            }

            ui.add_space(6.0);
            if ui.button("Cancelar").clicked() {
                app.admin.pending = None;
                app.admin.password.zeroize();
                app.admin.new_password.zeroize();
            }
        });
    });
    ui.add_space(8.0);
}

fn user_badges_modal(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let Some(user) = app.admin.managing_user_badges.clone() else {
        return;
    };

    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🏆").size(20.0));
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Gerenciar Selos de: {}", user.display_name))
                            .size(13.5)
                            .strong()
                            .color(Color32::from_rgb(1, 45, 147)),
                    );
                    ui.label(RichText::new(format!("({})", user.email)).size(11.0).color(nmessenger::MUTED));
                });
                ui.label(
                    RichText::new("Conceda ou revogue selos manualmente para este usuário.")
                        .size(11.0)
                        .color(nmessenger::MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if crate::platform::is_mobile() { "Voltar" } else { "Fechar" }).clicked() {
                    app.admin.managing_user_badges = None;
                }
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let Some(all_badges) = app.admin.badges.clone() else {
            ui.label(RichText::new("Carregando selos disponíveis...").color(nmessenger::MUTED));
            return;
        };

        if all_badges.is_empty() {
            ui.label(RichText::new("Nenhum selo cadastrado no sistema.").color(nmessenger::MUTED));
            return;
        }

        for b in &all_badges {
            let has_badge = user.badges.iter().any(|ub| ub.id == b.id || ub.slug == b.slug);

            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
                if let Some(texture) = nmessenger::badge_texture(ui.ctx(), &b.slug, &b.icon) {
                    ui.painter().image(
                        texture.id(),
                        icon_rect,
                        Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    ui.painter().text(icon_rect.center(), egui::Align2::CENTER_CENTER, &b.icon, egui::FontId::proportional(16.0), Color32::BLACK);
                }

                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&b.name).strong().size(12.0).color(nmessenger::HEADING));
                        badge(ui, &format!("+{} pts", b.points), Color32::from_rgb(255, 248, 225), Color32::from_rgb(180, 110, 10), Color32::from_rgb(240, 215, 140));
                        if has_badge {
                            badge(ui, "Conquistado", Color32::from_rgb(235, 250, 235), Color32::from_rgb(30, 130, 40), Color32::from_rgb(180, 230, 180));
                        }
                    });
                    if !b.description.is_empty() {
                        ui.label(RichText::new(&b.description).size(10.0).color(nmessenger::MUTED));
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_badge {
                        let revoke_btn = theme::danger_button(
                            ui,
                            RichText::new("Revogar Selo").size(10.5),
                            Vec2::new(100.0, 26.0),
                        );
                        if revoke_btn.clicked() {
                            app.send(Action::AdminMutateUserBadge {
                                user_id: user.id,
                                mutation: UserBadgeMutation {
                                    badge_id: b.id,
                                    grant: false,
                                },
                            });
                            // Optimistically update local user badges so UI responds instantly
                            if let Some(u) = app.admin.managing_user_badges.as_mut() {
                                u.badges.retain(|ub| ub.id != b.id && ub.slug != b.slug);
                            }
                            if let Some(users_page) = app.admin.users.as_mut() {
                                if let Some(target) = users_page.items.iter_mut().find(|target| target.id == user.id) {
                                    target.badges.retain(|ub| ub.id != b.id && ub.slug != b.slug);
                                }
                            }
                        }
                    } else {
                        let grant_btn = theme::primary_button(
                            ui,
                            RichText::new("Conceder Selo")
                                .size(10.5)
                                .color(Color32::from_rgb(1, 45, 147))
                                .strong(),
                            Vec2::new(100.0, 26.0),
                        );
                        if grant_btn.clicked() {
                            app.send(Action::AdminMutateUserBadge {
                                user_id: user.id,
                                mutation: UserBadgeMutation {
                                    badge_id: b.id,
                                    grant: true,
                                },
                            });
                            let new_badge = nm_protocol::Badge {
                                id: b.id,
                                slug: b.slug.clone(),
                                name: b.name.clone(),
                                description: b.description.clone(),
                                icon: b.icon.clone(),
                                points: b.points,
                                awarded_at: Some(chrono::Utc::now()),
                            };
                            if let Some(u) = app.admin.managing_user_badges.as_mut() {
                                u.badges.push(new_badge.clone());
                            }
                            if let Some(users_page) = app.admin.users.as_mut() {
                                if let Some(target) = users_page.items.iter_mut().find(|target| target.id == user.id) {
                                    target.badges.push(new_badge);
                                }
                            }
                        }
                    }
                });
            });
            ui.add_space(4.0);
        }
    });
    ui.add_space(8.0);
}

fn badges(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let is_mob = crate::platform::is_mobile();
    if is_mob {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Selos e Conquistas")
                    .size(15.0)
                    .strong()
                    .color(nmessenger::HEADING),
            );
            ui.label(
                RichText::new("Cadastre novos selos e configure pontuações.")
                    .size(10.5)
                    .color(nmessenger::MUTED),
            );
            if !app.admin.creating_badge && app.admin.editing_badge.is_none() {
                ui.add_space(4.0);
                let new_btn = theme::primary_button(
                    ui,
                    RichText::new("+ Criar Novo Selo")
                        .size(11.5)
                        .color(Color32::from_rgb(1, 45, 147))
                        .strong(),
                    Vec2::new(140.0, 30.0),
                );
                if new_btn.clicked() {
                    app.admin.creating_badge = true;
                    app.admin.editing_badge = None;
                    app.admin.badge_slug.clear();
                    app.admin.badge_name.clear();
                    app.admin.badge_description.clear();
                    app.admin.badge_icon = "🏆".to_string();
                    app.admin.badge_points = 25;
                }
            }
        });
    } else {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Gerenciador de Selos e Conquistas")
                        .size(15.0)
                        .strong()
                        .color(nmessenger::HEADING),
                );
                ui.label(
                    RichText::new("Cadastre novos selos, configure suas pontuações e gerencie selos de usuários.")
                        .size(11.0)
                        .color(nmessenger::MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !app.admin.creating_badge && app.admin.editing_badge.is_none() {
                    let new_btn = theme::primary_button(
                        ui,
                        RichText::new("+ Criar Novo Selo")
                            .size(11.5)
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                        Vec2::new(140.0, 30.0),
                    );
                    if new_btn.clicked() {
                        app.admin.creating_badge = true;
                        app.admin.editing_badge = None;
                        app.admin.badge_slug.clear();
                        app.admin.badge_name.clear();
                        app.admin.badge_description.clear();
                        app.admin.badge_icon = "🏆".to_string();
                        app.admin.badge_points = 25;
                    }
                }
            });
        });
    }

    ui.add_space(10.0);

    if app.admin.creating_badge || app.admin.editing_badge.is_some() {
        let is_edit = app.admin.editing_badge.is_some();
        card(ui, |ui| {
            ui.label(
                RichText::new(if is_edit { "Editar Selo" } else { "Cadastrar Novo Selo" })
                    .size(13.5)
                    .strong()
                    .color(nmessenger::HEADING),
            );
            ui.add_space(6.0);

            if !is_edit {
                ui.label(RichText::new("Identificador único (slug):").strong().size(10.5));
                ui.add(
                    egui::TextEdit::singleline(&mut app.admin.badge_slug)
                        .hint_text("ex: membro-honrario, gamer-mestre...")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
            }

            ui.label(RichText::new("Nome do selo:").strong().size(10.5));
            ui.add(
                egui::TextEdit::singleline(&mut app.admin.badge_name)
                    .hint_text("ex: Membro Honorário")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(4.0);

            ui.label(RichText::new("Descrição da conquista:").strong().size(10.5));
            ui.add(
                egui::TextEdit::multiline(&mut app.admin.badge_description)
                    .hint_text("O que o usuário fez para conquistar este selo...")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Ícone / Emoji / Asset:").strong().size(10.5));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.admin.badge_icon)
                            .hint_text("ex: 🏆, 🥇, badges/founder.svg")
                            .desired_width(180.0),
                    );
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Pontos do selo:").strong().size(10.5));
                    ui.add(egui::DragValue::new(&mut app.admin.badge_points).range(0..=1000));
                });
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let can_save = !app.admin.busy
                    && !app.admin.badge_name.trim().is_empty()
                    && (is_edit || !app.admin.badge_slug.trim().is_empty());
                let save_btn = ui.add_enabled_ui(can_save, |ui| {
                    theme::primary_button(
                        ui,
                        RichText::new(if is_edit { "Salvar Alterações" } else { "Criar Selo" })
                            .size(11.5)
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                        Vec2::new(140.0, 28.0),
                    ).clicked()
                }).inner;

                if save_btn {
                    if let Some(edit) = &app.admin.editing_badge {
                        app.send(Action::AdminUpdateBadge(
                            edit.id,
                            UpdateBadgeRequest {
                                name: Some(app.admin.badge_name.trim().to_string()),
                                description: Some(app.admin.badge_description.clone()),
                                icon: Some(app.admin.badge_icon.trim().to_string()),
                                points: Some(app.admin.badge_points),
                            },
                        ));
                    } else {
                        app.send(Action::AdminCreateBadge(CreateBadgeRequest {
                            slug: app.admin.badge_slug.trim().to_lowercase(),
                            name: app.admin.badge_name.trim().to_string(),
                            description: app.admin.badge_description.clone(),
                            icon: app.admin.badge_icon.trim().to_string(),
                            points: app.admin.badge_points,
                        }));
                    }
                    app.admin.creating_badge = false;
                    app.admin.editing_badge = None;
                }

                if ui.button("Cancelar").clicked() {
                    app.admin.creating_badge = false;
                    app.admin.editing_badge = None;
                }
            });
        });
        ui.add_space(10.0);
    }

    let Some(badges_list) = &app.admin.badges else {
        if app.admin.busy {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Carregando selos do sistema...").color(nmessenger::MUTED));
            });
        }
        return;
    };

    if badges_list.is_empty() {
        card(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("Nenhum selo cadastrado no sistema.").color(nmessenger::MUTED));
                ui.add_space(12.0);
            });
        });
        return;
    }

    for b in badges_list {
        card(ui, |ui| {
            if is_mob {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                        if let Some(texture) = nmessenger::badge_texture(ui.ctx(), &b.slug, &b.icon) {
                            ui.painter().image(
                                texture.id(),
                                icon_rect,
                                Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            ui.painter().rect_filled(icon_rect, 6, Color32::from_rgb(245, 248, 252));
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &b.icon,
                                egui::FontId::proportional(18.0),
                                Color32::BLACK,
                            );
                        }

                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(&b.name)
                                        .size(13.0)
                                        .strong()
                                        .color(nmessenger::HEADING),
                                );
                                badge(
                                    ui,
                                    &format!("+{} pts", b.points),
                                    Color32::from_rgb(255, 248, 225),
                                    Color32::from_rgb(180, 110, 10),
                                    Color32::from_rgb(240, 215, 140),
                                );
                                badge(
                                    ui,
                                    &format!("{} usuário(s)", b.users_count),
                                    Color32::from_rgb(238, 246, 255),
                                    Color32::from_rgb(20, 80, 160),
                                    Color32::from_rgb(190, 220, 245),
                                );
                            });
                        });
                    });

                    if !b.description.is_empty() {
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(&b.description)
                                .size(10.5)
                                .color(nmessenger::INK),
                        );
                    }

                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("Slug: {}", b.slug))
                            .size(9.5)
                            .monospace()
                            .color(nmessenger::MUTED),
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Editar").size(11.0)).min_size(Vec2::new(75.0, 28.0))).clicked() {
                            app.admin.creating_badge = false;
                            app.admin.editing_badge = Some(b.clone());
                            app.admin.badge_slug = b.slug.clone();
                            app.admin.badge_name = b.name.clone();
                            app.admin.badge_description = b.description.clone();
                            app.admin.badge_icon = b.icon.clone();
                            app.admin.badge_points = b.points;
                        }
                        if theme::danger_button(ui, RichText::new("Excluir").size(11.0), Vec2::new(75.0, 28.0)).clicked() {
                            app.admin.pending = Some(Mutation::DeleteBadge {
                                id: b.id,
                                name: b.name.clone(),
                            });
                        }
                    });
                });
            } else {
                ui.horizontal(|ui| {
                    let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
                    if let Some(texture) = nmessenger::badge_texture(ui.ctx(), &b.slug, &b.icon) {
                        ui.painter().image(
                            texture.id(),
                            icon_rect,
                            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().rect_filled(icon_rect, 6, Color32::from_rgb(245, 248, 252));
                        ui.painter().text(
                            icon_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &b.icon,
                            egui::FontId::proportional(20.0),
                            Color32::BLACK,
                        );
                    }

                    ui.add_space(10.0);

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&b.name)
                                    .size(13.5)
                                    .strong()
                                    .color(nmessenger::HEADING),
                            );
                            badge(
                                ui,
                                &format!("+{} pts", b.points),
                                Color32::from_rgb(255, 248, 225),
                                Color32::from_rgb(180, 110, 10),
                                Color32::from_rgb(240, 215, 140),
                            );
                            badge(
                                ui,
                                &format!("{} usuário(s)", b.users_count),
                                Color32::from_rgb(238, 246, 255),
                                Color32::from_rgb(20, 80, 160),
                                Color32::from_rgb(190, 220, 245),
                            );
                        });

                        if !b.description.is_empty() {
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(&b.description)
                                    .size(11.0)
                                    .color(nmessenger::INK),
                            );
                        }

                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!("Slug: {}", b.slug))
                                .size(9.5)
                                .monospace()
                                .color(nmessenger::MUTED),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::danger_button(ui, RichText::new("Excluir").size(10.5), Vec2::new(65.0, 24.0)).clicked() {
                            app.admin.pending = Some(Mutation::DeleteBadge {
                                id: b.id,
                                name: b.name.clone(),
                            });
                        }

                        if ui.button(RichText::new("Editar").size(10.5)).clicked() {
                            app.admin.creating_badge = false;
                            app.admin.editing_badge = Some(b.clone());
                            app.admin.badge_slug = b.slug.clone();
                            app.admin.badge_name = b.name.clone();
                            app.admin.badge_description = b.description.clone();
                            app.admin.badge_icon = b.icon.clone();
                            app.admin.badge_points = b.points;
                        }
                    });
                });
            }
        });
        ui.add_space(4.0);
    }
}
