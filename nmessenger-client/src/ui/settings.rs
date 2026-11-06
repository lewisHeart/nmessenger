use super::{nmessenger, theme};
use crate::{app::MessengerApp, transport::Action};
use crate::i18n::tr;
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use nm_protocol::*;

const PAGES: &[(usize, &str)] = &[
    (0, "Conta"),
    (3, "Interface"),
    (1, "Conversas"),
    (6, "Sons e alertas"),
    (7, "Áudio e vídeo"),
    (2, "Privacidade"),
    (4, "Sessões"),
    (5, "Conexão"),
];

fn page_label(page: usize, portuguese: &'static str) -> &'static str {
    let english = match page {
        0 => "Account", 1 => "Conversations", 2 => "Privacy", 3 => "Interface",
        4 => "Sessions", 5 => "Connection", 6 => "Sounds and alerts", 7 => "Audio and video", 8 => "Interface",
        _ => portuguese,
    };
    tr(english, portuguese)
}

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.options.open {
        use zeroize::Zeroize;
        app.options.history_backup_password.zeroize();
        return;
    }
    let is_mobile = crate::platform::is_mobile();
    if is_mobile {
        if !super::dialog::mobile_page(ctx, "mobile-settings", tr("Settings", "Configurações"), |ui| {
            content(app, ui);
            false
        }) {
            app.options.open = false;
        }
        super::announcements::show(app, ctx);
        return;
    }
    let mut builder = egui::ViewportBuilder::default()
        .with_title(tr("Settings - NMessenger", "Configurações - NMessenger"))
        .with_decorations(super::window::system_titlebars(ctx))
        .with_transparent(true);
    if !is_mobile {
        builder = builder.with_min_inner_size([680.0, 520.0]);
        if !super::window::has_viewport_initialized(ctx, "settings") {
            builder = builder.with_inner_size(super::window::initial_size(ctx, "settings", [780.0, 590.0]));
            super::window::mark_viewport_initialized(ctx, "settings");
        }
    }
    let viewport_id = egui::ViewportId::from_hash_of("nmessenger-settings");

    if let Some(shared) = app.shared.clone() {
        if !ctx.embed_viewports() {
            ctx.show_viewport_deferred(
                viewport_id,
                builder,
                move |ui, class| {
                    let mut guard = shared.lock().unwrap();
                    render_settings_window(&mut *guard, ui, class, is_mobile);
                },
            );
            return;
        }
    }

    ctx.show_viewport_immediate(
        viewport_id,
        builder,
        |ui, class| {
            render_settings_window(app, ui, class, is_mobile);
        },
    );
    if !app.options.open { super::window::forget_initial_size(ctx, "settings"); }
}

fn render_settings_window(app: &mut MessengerApp, ui: &mut egui::Ui, class: egui::ViewportClass, is_mobile: bool) {
    if class != egui::ViewportClass::EmbeddedWindow { super::window::record_size(ui.ctx(), "settings"); }
    if !is_mobile {
        super::window::minimum(ui.ctx(), Vec2::new(680.0, 520.0));
    }
    if ui.ctx().input(|i| i.viewport().close_requested()) {
        app.options.open = false;
        super::window::forget_initial_size(ui.ctx(), "settings");
    }
    if class == egui::ViewportClass::EmbeddedWindow {
        let mut open = app.options.open;
        let mut close = false;
        let screen = ui.ctx().content_rect();
        let win_w = if is_mobile {
            screen.width()
        } else {
            (screen.width() - 16.0).min(750.0)
        };
        let win_h = if is_mobile {
            screen.height()
        } else {
            (screen.height() - 24.0).min(560.0)
        };
        let mut window = egui::Window::new(tr("Settings", "Configurações"))
            .title_bar(false)
            .collapsible(false)
            .vscroll(false)
            .hscroll(false)
            .frame(if is_mobile {
                egui::Frame::new()
                    .fill(Color32::from_rgb(245, 248, 252))
                    .inner_margin(egui::Margin::symmetric(10, 8))
            } else {
                super::dialog::frame()
            })
            .open(&mut open);
        if is_mobile {
            window = window.fixed_pos(screen.min).fixed_size([win_w, win_h]);
        } else {
            window = window.default_size([win_w, win_h]).max_size([win_w, win_h]);
        }
        window.show(ui.ctx(), |ui| {
            close |= super::dialog::embedded_title(ui, tr("Settings", "Configurações"));
            content(app, ui);
        });
        app.options.open = open && !close;
    } else {
        theme::window_bar_with_controls(
            ui,
            tr("Settings - NMessenger", "Configurações - NMessenger"),
            theme::WindowControls::CloseOnly,
        );
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(12),
            )
            .show(ui, |ui| content(app, ui));
    }
    super::announcements::show(app, ui.ctx());
}

fn content(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let is_mob = crate::platform::is_mobile();
    egui::Panel::bottom("settings-footer")
        .exact_size(if is_mob { 54.0 } else { 56.0 })
        .show_separator_line(true)
        .frame(
            egui::Frame::new()
                .fill(if is_mob { Color32::from_rgb(250, 253, 255) } else { Color32::from_white_alpha(240) })
                .inner_margin(egui::Margin::symmetric(10, 8)),
        )
        .show(ui, |ui| {
            if is_mob {
                let btn_w = ((ui.available_width() - 8.0) / 2.0).max(90.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !app.options.saving,
                            egui::Button::new(tr("Cancel", "Cancelar")).min_size(Vec2::new(btn_w, 34.0)),
                        )
                        .clicked()
                    {
                        app.options.open = false;
                    }
                    let save_button = if !app.options.saving && !app.options.loading {
                        ui.add(
                            egui::Button::new(
                                RichText::new(tr("Save changes", "Salvar alterações"))
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(nmessenger::BLUE)
                            .min_size(Vec2::new(btn_w, 34.0)),
                        )
                    } else {
                        ui.add_enabled(
                            false,
                            egui::Button::new(
                                RichText::new(if app.options.saving {
                                    tr("Saving...", "Salvando...")
                                } else {
                                    tr("Loading...", "Carregando...")
                                })
                                .strong(),
                            )
                            .min_size(Vec2::new(btn_w, 34.0)),
                        )
                    };
                    if save_button.clicked() {
                        save(app);
                    }
                    if app.options.saving || app.options.loading {
                        ui.spinner();
                    }
                });
            } else {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save_button = if !app.options.saving && !app.options.loading {
                        theme::primary_button(
                            ui,
                            RichText::new(tr("Save changes", "Salvar alterações"))
                                .color(Color32::from_rgb(1, 45, 147))
                                .strong(),
                            Vec2::new(145.0, 32.0),
                        )
                    } else {
                        ui.add_enabled(
                            false,
                            egui::Button::new(
                                RichText::new(if app.options.saving {
                                    tr("Saving...", "Salvando...")
                                } else {
                                    tr("Loading...", "Carregando...")
                                })
                                .strong(),
                            )
                            .min_size(Vec2::new(145.0, 32.0)),
                        )
                    };
                    if save_button.clicked() {
                        save(app);
                    }
                    if ui
                        .add_enabled(
                            !app.options.saving,
                            egui::Button::new(tr("Cancel", "Cancelar")).min_size(Vec2::new(100.0, 32.0)),
                        )
                        .clicked()
                    {
                        app.options.open = false;
                    }
                    if app.options.saving || app.options.loading {
                        ui.spinner();
                    }
                });
            }
        });
    if crate::platform::is_mobile() {
        egui::Panel::top("settings-mobile-navigation")
            .exact_size(48.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_white_alpha(220))
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ui, |ui| {
                super::scroll::horizontal().id_salt("settings-mobile-pages").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for (page, label) in PAGES {
                            let selected = app.options.tab == *page;
                            let text = page_label(*page, label);
                            let font = FontId::proportional(12.0);
                            let text_w = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE).size().x);
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(text_w + 22.0, 30.0), egui::Sense::click());
                            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                            let held = resp.is_pointer_button_down_on();
                            let hovered = resp.hovered();

                            let (bg, stroke, text_color) = if selected {
                                if held {
                                    (Color32::from_rgb(0, 95, 190), Stroke::NONE, Color32::WHITE)
                                } else {
                                    (nmessenger::BLUE, Stroke::NONE, Color32::WHITE)
                                }
                            } else if held {
                                (
                                    Color32::from_rgb(195, 225, 252),
                                    Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                                    Color32::from_rgb(10, 50, 110),
                                )
                            } else if hovered {
                                (
                                    Color32::from_rgba_unmultiplied(230, 242, 253, 230),
                                    Stroke::new(1.0, Color32::from_rgb(160, 205, 245)),
                                    nmessenger::HEADING,
                                )
                            } else {
                                (
                                    Color32::from_white_alpha(180),
                                    Stroke::new(1.0, Color32::from_rgb(212, 226, 240)),
                                    nmessenger::HEADING,
                                )
                            };

                            ui.painter().rect_filled(rect, 16, bg);
                            if stroke != Stroke::NONE {
                                ui.painter().rect_stroke(rect, 16, stroke, egui::StrokeKind::Inside);
                            }
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                text,
                                font,
                                text_color,
                            );
                            if resp.clicked() {
                                app.options.tab = *page;
                            }
                        }
                    });
                });
            });
    } else { egui::Panel::left("settings-navigation")
        .exact_size(156.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(Color32::from_white_alpha(170))
                .corner_radius(8)
                .inner_margin(8),
        )
        .show(ui, |ui| {
            for (page, label) in PAGES {
                let selected = app.options.tab == *page;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(138.0, 35.0), egui::Sense::click());
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                let held = resp.is_pointer_button_down_on();
                let hovered = resp.hovered();

                let (bg, stroke) = if held {
                    (
                        Color32::from_rgb(188, 220, 248),
                        Stroke::new(1.0, Color32::from_rgb(70, 145, 230)),
                    )
                } else if selected {
                    if hovered {
                        (
                            Color32::from_rgb(214, 233, 249),
                            Stroke::new(1.0, Color32::from_rgb(150, 198, 245)),
                        )
                    } else {
                        (
                            Color32::from_rgb(221, 237, 250),
                            Stroke::new(1.0, Color32::from_rgb(195, 220, 245)),
                        )
                    }
                } else if hovered {
                    (
                        Color32::from_rgba_unmultiplied(232, 243, 253, 220),
                        Stroke::new(1.0, Color32::from_rgb(185, 215, 245)),
                    )
                } else {
                    (Color32::TRANSPARENT, Stroke::NONE)
                };

                if bg != Color32::TRANSPARENT {
                    ui.painter().rect_filled(rect, 6, bg);
                }
                if stroke != Stroke::NONE {
                    ui.painter().rect_stroke(rect, 6, stroke, egui::StrokeKind::Inside);
                }

                let text_color = if selected {
                    Color32::from_rgb(16, 68, 128)
                } else if held {
                    Color32::from_rgb(10, 48, 100)
                } else if hovered {
                    Color32::from_rgb(20, 75, 140)
                } else {
                    nmessenger::HEADING
                };

                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    page_label(*page, label),
                    FontId::proportional(12.0),
                    text_color,
                );

                if resp.clicked() {
                    app.options.tab = *page;
                }
                ui.add_space(3.0);
            }
        }); }
    ui.add_space(6.0);
    super::scroll::vertical()
        .id_salt("settings-content")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let inner_w = ui.available_width();
            ui.set_width(inner_w);
            if let Some(error) = &app.options.error {
                ui.colored_label(Color32::from_rgb(185, 28, 28), error);
                ui.add_space(8.0);
            }
            ui.add_enabled_ui(!app.options.saving && !app.options.loading, |ui| match app
                .options
                .tab
            {
                0 => account(app, ui),
                1 => conversations(app, ui),
                2 => privacy(app, ui),
                3 | 8 => interface(app, ui),
                4 => sessions(app, ui),
                6 => notifications(app, ui),
                7 => media(app, ui),
                _ => connection(app, ui),
            });
            if is_mob {
                ui.add_space(32.0);
            }
        });
}

fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let is_mob = crate::platform::is_mobile();
    ui.label(
        RichText::new(title)
            .size(if is_mob { 18.0 } else { 20.0 })
            .strong()
            .color(nmessenger::HEADING),
    );
    ui.label(
        RichText::new(subtitle)
            .size(if is_mob { 10.0 } else { 11.0 })
            .color(nmessenger::MUTED),
    );
    ui.add_space(if is_mob { 6.0 } else { 10.0 });
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    let is_mob = crate::platform::is_mobile();
    let pad_h = if is_mob { 12 } else { 14 };
    let pad_v = if is_mob { 10 } else { 14 };
    egui::Frame::new()
        .fill(Color32::from_white_alpha(226))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(pad_h, pad_v))
        .stroke(egui::Stroke::new(0.8_f32, Color32::from_rgb(217, 230, 242)))
        .show(ui, |ui| {
            let inner_w = ui.available_width();
            ui.set_max_width(inner_w);
            ui.set_width(inner_w);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            super::glass::gloss_line(ui);
            contents(ui);
        });
    ui.add_space(if is_mob { 8.0 } else { 10.0 });
}

fn account(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Your account", "Sua conta"), tr("How you appear to your contacts.", "Como você aparece para seus contatos."));
    card(ui, |ui| {
        if let Some(me) = app.data.as_ref().map(|data| data.me.clone()) {
            let is_mob = crate::platform::is_mobile();
            ui.horizontal(|ui| {
                theme::avatar(app, ui, &me, if is_mob { 64.0 } else { 88.0 });
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(tr("SIGNED-IN ACCOUNT", "CONTA CONECTADA"))
                            .size(9.0)
                            .color(Color32::from_rgb(120, 145, 170))
                            .strong(),
                    );
                    ui.label(
                        RichText::new(&me.email)
                            .size(if is_mob { 13.5 } else { 15.0 })
                            .strong()
                            .color(nmessenger::HEADING),
                    );
                    ui.add_space(6.0);
                    let camera_icon = nmessenger::texture(ui.ctx(), "icons/camera.png")
                        .map(|t| egui::load::SizedTexture::new(t.id(), egui::vec2(16.0, 16.0)));
                    let change_avatar = theme::primary_button_with_icon(
                        ui,
                        camera_icon,
                        RichText::new(tr("Change profile picture...", "Alterar imagem de perfil..."))
                            .size(11.0)
                            .color(Color32::from_rgb(1, 45, 147))
                            .strong(),
                        Vec2::new(if is_mob { (ui.available_width() - 8.0).min(185.0) } else { 195.0 }, 28.0),
                    );
                    if change_avatar.clicked() {
                        app.options.avatar_picker = true;
                    }
                });
            });
            ui.add_space(14.0);
            ui.separator();
            ui.add_space(12.0);
        }

        ui.label(
            RichText::new(tr("Display name", "Nome de exibição"))
                .strong()
                .size(11.5)
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(tr("The name your contacts see in their list and messages.", "O nome que seus contatos verão na lista e nas mensagens."))
                .size(10.0)
                .color(nmessenger::MUTED),
        );
        ui.add_space(3.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.options.display_name)
                .margin(egui::Margin::symmetric(8, 6))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(12.0);

        ui.label(
            RichText::new(tr("Personal message", "Mensagem pessoal"))
                .strong()
                .size(11.5)
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(tr("A status message shown next to your display name.", "Frase de status exibida ao lado do seu nome de exibição."))
                .size(10.0)
                .color(nmessenger::MUTED),
        );
        ui.add_space(3.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.options.personal_message)
                .margin(egui::Margin::symmetric(8, 6))
                .desired_width(f32::INFINITY)
                .hint_text(tr("What's on your mind today?", "O que você está pensando hoje?")),
        );
        ui.add_space(12.0);

        ui.label(
            RichText::new(tr("Music or activity", "Música ou atividade"))
                .strong()
                .size(11.5)
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(tr("Shown with a music note in your presence.", "Exibido com o ícone de nota musical na sua presença."))
                .size(10.0)
                .color(nmessenger::MUTED),
        );
        ui.add_space(3.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.options.music)
                .margin(egui::Margin::symmetric(8, 6))
                .desired_width(f32::INFINITY)
                .hint_text(tr("Artist — Song title", "Artista — Nome da Música")),
        );
        ui.add_space(6.0);
        ui.checkbox(
            &mut app.options.settings.music_status,
            tr("Detect and share what I'm listening to (Rich Presence / Spotify)", "Detectar e compartilhar o que estou ouvindo (Rich Presence / Spotify)"),
        );
        ui.label(
            RichText::new(tr("A newly playing track replaces the current value.", "Uma música nova em reprodução substitui o valor atual."))
                .size(10.0)
                .color(nmessenger::MUTED),
        );
        music_access(ui, app.options.settings.music_status);
    });
    card(ui, |ui| {
        ui.label(
            RichText::new(tr("Status and presence", "Status e presença"))
                .strong()
                .size(13.0)
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(tr(
                "Change your status to Away when you are idle.",
                "Altera automaticamente seu status para Ausente quando você estiver inativo.",
            ))
            .size(10.0)
            .color(nmessenger::MUTED),
        );
        ui.add_space(8.0);

        let mut enabled = app.options.settings.auto_away_minutes > 0;
        let mut minutes = if enabled {
            app.options.settings.auto_away_minutes
        } else {
            10
        };
        ui.horizontal(|ui| {
            if ui
                .checkbox(
                    &mut enabled,
                    tr(
                        "Show as \"Away\" when inactive for:",
                        "Mostrar como \"Ausente\" quando inativo por:",
                    ),
                )
                .changed()
            {
                app.options.settings.auto_away_minutes = if enabled { minutes } else { 0 };
            }
            if enabled {
                let drag = ui.add(
                    egui::DragValue::new(&mut minutes)
                        .range(1..=1440)
                        .suffix(tr(" min", " min")),
                );
                if drag.changed() {
                    app.options.settings.auto_away_minutes = minutes.max(1);
                }
            }
        });
        if enabled {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(tr("Presets:", "Predefinições:"))
                        .small()
                        .color(nmessenger::MUTED),
                );
                for preset in [5, 10, 15, 30, 60] {
                    let is_cur = app.options.settings.auto_away_minutes == preset;
                    let text = format!("{preset} min");
                    let btn = if is_cur {
                        ui.add(
                            egui::Button::new(
                                RichText::new(&text)
                                    .size(11.0)
                                    .strong()
                                    .color(Color32::from_rgb(1, 45, 147)),
                            )
                            .fill(Color32::from_rgb(205, 228, 252))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(150, 195, 235)))
                            .corner_radius(4),
                        )
                    } else {
                        ui.add(
                            egui::Button::new(
                                RichText::new(&text).size(11.0).color(nmessenger::HEADING),
                            )
                            .fill(Color32::from_white_alpha(200))
                            .stroke(egui::Stroke::new(0.8, Color32::from_rgb(217, 230, 242)))
                            .corner_radius(4),
                        )
                    };
                    if btn.clicked() {
                        app.options.settings.auto_away_minutes = preset;
                    }
                }
            });
        }
    });
}

fn interface(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Interface", "Interface"), tr("Language, scale, colors and themes.", "Idioma, escala, cores e temas."));
    card(ui, |ui| {
        super::language::selector(ui, "settings-language", &mut app.options.local.language);
    });
    card(ui, |ui| {
        ui.label(
            RichText::new(tr("Interface scale", "Escala da interface"))
                .size(13.0)
                .strong()
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(
                tr("Adjusts text, buttons and windows across the application.", "Ajusta proporcionalmente os textos, botões e janelas de todo o aplicativo."),
            )
            .size(10.0)
            .color(nmessenger::MUTED),
        );
        ui.add_space(10.0);

        let current_pct = (app.options.local.ui_scale * 100.0).round() as i32;
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 85.0).clamp(110.0, 240.0);
            ui.add(
                egui::Slider::new(&mut app.options.local.ui_scale, 0.75..=2.0)
                    .trailing_fill(true)
                    .show_value(false),
            );
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(230, 242, 253))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(180, 210, 238)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{current_pct}%"))
                            .size(12.0)
                            .strong()
                            .color(Color32::from_rgb(1, 45, 147)),
                    );
                });
        });
        ui.add_space(8.0);

        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(tr("Presets:", "Predefinições:"))
                    .small()
                    .color(nmessenger::MUTED),
            );
            let is_mob = crate::platform::is_mobile();
            let default_lbl = if is_mob {
                "125%"
            } else {
                tr("125% (Default)", "125% (Padrão)")
            };
            for (scale, label) in [
                (0.80, "80%"),
                (1.0, "100%"),
                (1.25, default_lbl),
                (1.50, "150%"),
            ] {
                let active = (app.options.local.ui_scale - scale).abs() < 0.03;
                let btn_w = if scale == 1.25 && !is_mob { 95.0 } else { 52.0 };
                let btn = ui.add(
                    egui::Button::new(
                        RichText::new(label)
                            .size(10.0)
                            .color(if active {
                                Color32::from_rgb(1, 45, 147)
                            } else {
                                nmessenger::INK
                            })
                            .strong(),
                    )
                    .fill(if active {
                        theme::SKY
                    } else {
                        Color32::from_white_alpha(200)
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if active {
                            Color32::from_rgb(150, 195, 235)
                        } else {
                            Color32::from_rgb(215, 225, 235)
                        },
                    ))
                    .corner_radius(4)
                    .min_size(Vec2::new(btn_w, 22.0)),
                );
                if btn.clicked() {
                    app.options.local.ui_scale = scale;
                }
            }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new(tr("Message fonts have their own size and style settings in the composer.", "A fonte das mensagens possui tamanho e estilo próprios configuráveis no compositor."))
                .small()
                .color(nmessenger::MUTED),
        );
    });

    card(ui, |ui| {
        ui.label(
            RichText::new(tr("Interface color (Skin)", "Cor da interface (Skin)"))
                .size(13.0)
                .strong()
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(tr("Choose the color of the theme and title bar.", "Escolha o tom do tema e da barra superior da janela."))
                .size(10.0)
                .color(nmessenger::MUTED),
        );
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            let is_mob = crate::platform::is_mobile();
            let num_cols = if is_mob || ui.available_width() < 340.0 { 2 } else { 3 };
            let spacing_x = 6.0;
            let btn_w = ((ui.available_width() - spacing_x * (num_cols - 1) as f32) / num_cols as f32).clamp(72.0, 110.0);
            for (value, label, top_col) in [
                ("azul", tr("Aero Blue", "Azul Aero"), Color32::from_rgb(194, 221, 244)),
                ("verde", tr("Green", "Verde"), Color32::from_rgb(200, 230, 201)),
                ("rosa", tr("Pink", "Rosa"), Color32::from_rgb(252, 213, 206)),
                ("prata", tr("Silver", "Prata"), Color32::from_rgb(229, 229, 234)),
                ("purple", tr("Purple", "Roxo"), Color32::from_rgb(193, 170, 231)),
                ("orange", tr("Orange", "Laranja"), Color32::from_rgb(255, 199, 151)),
            ] {
                let selected = app.options.settings.theme.eq_ignore_ascii_case(value)
                    || (value == "purple"
                        && app.options.settings.theme.eq_ignore_ascii_case("roxo"));
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(btn_w, 48.0), egui::Sense::click_and_drag());
                if response.hovered() || selected {
                    ui.painter().rect_filled(
                        rect,
                        6,
                        if selected {
                            Color32::from_rgb(225, 240, 254)
                        } else {
                            Color32::from_rgb(243, 248, 253)
                        },
                    );
                    ui.painter().rect_stroke(
                        rect,
                        6,
                        egui::Stroke::new(
                            if selected { 1.5 } else { 1.0 },
                            if selected {
                                Color32::from_rgb(46, 131, 237)
                            } else {
                                Color32::from_rgb(200, 220, 240)
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                } else {
                    ui.painter().rect_stroke(
                        rect,
                        6,
                        egui::Stroke::new(1.0, Color32::from_rgb(225, 232, 240)),
                        egui::StrokeKind::Inside,
                    );
                }
                let swatch_rect = egui::Rect::from_min_size(
                    rect.min + Vec2::new(8.0, 7.0),
                    Vec2::new(rect.width() - 16.0, 16.0),
                );
                ui.painter().rect_filled(swatch_rect, 3, top_col);
                ui.painter().rect_stroke(
                    swatch_rect,
                    3,
                    egui::Stroke::new(1.0, Color32::from_black_alpha(25)),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    rect.center_bottom() - Vec2::new(0.0, 7.0),
                    egui::Align2::CENTER_BOTTOM,
                    label,
                    egui::FontId::proportional(10.5),
                    if selected {
                        nmessenger::HEADING
                    } else {
                        Color32::from_rgb(70, 70, 70)
                    },
                );
                if response.clicked() {
                    app.options.settings.theme = value.into();
                    nmessenger::set_theme(ui.ctx(), value);
                    ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
                }
            }
        });
    });

    card(ui, |ui| {
        ui.label(
            RichText::new(tr("Windows and decorations", "Janelas e Decorações"))
                .size(13.0)
                .strong()
                .color(nmessenger::HEADING),
        );
        ui.add_space(4.0);
        ui.checkbox(
            &mut app.options.local.system_titlebars,
            tr("Use system title bars", "Usar barras de título do sistema operacional"),
        );
        ui.label(
            RichText::new(
                tr("Turn off to use the classic NMessenger style and rounded corners.", "Desative para utilizar o visual clássico e cantos arredondados nativos do NMessenger."),
            )
            .small()
            .color(nmessenger::MUTED),
        );
    });
    card(ui, |ui| {
        ui.label(RichText::new(tr("Custom emoticons", "Emoticons personalizados")).strong());
        ui.label(RichText::new(tr(
            "Choose a name and shortcut, then select an image. Send it from the emoji menu or type its shortcut alone.",
            "Escolha um nome e um atalho e selecione uma imagem. Envie pelo menu de emojis ou digite o atalho sozinho.",
        )).small().color(nmessenger::MUTED));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.options.asset_name)
                    .hint_text(tr("Name", "Nome"))
                    .desired_width(145.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.options.shortcut)
                    .hint_text(tr("Shortcut, e.g. :cat:", "Atalho, ex. :gato:"))
                    .desired_width(115.0),
            );
            if ui.add_enabled(!app.options.emoticon_busy, egui::Button::new(tr("Choose image...", "Escolher imagem..."))).clicked() {
                let name = app.options.asset_name.trim();
                let shortcut = app.options.shortcut.trim();
                let duplicate = app.data.as_ref().is_some_and(|data| data.assets.iter().any(|asset|
                    asset.kind == AssetKind::Emoticon && asset.shortcut.as_deref() == Some(shortcut)));
                let error = if name.is_empty() || shortcut.is_empty() {
                    Some(tr("Enter a name and shortcut first.", "Preencha o nome e o atalho primeiro."))
                } else if name.chars().count() > 100 || shortcut.chars().count() > 32 {
                    Some(tr("The name or shortcut is too long.", "O nome ou o atalho é longo demais."))
                } else if duplicate {
                    Some(tr("This shortcut is already in use.", "Este atalho já está em uso."))
                } else { None };
                if let Some(error) = error {
                    app.options.emoticon_status = Some(Err(error.into()));
                } else {
                    app.options.emoticon_busy = true;
                    app.options.emoticon_status = None;
                    app.send(Action::UploadAsset {
                        kind: AssetKind::Emoticon,
                        name: name.to_owned(),
                        shortcut: Some(shortcut.to_owned()),
                        local: None,
                    });
                }
            }
        });
        if app.options.emoticon_busy {
            ui.spinner();
        }
        if let Some(status) = &app.options.emoticon_status {
            match status {
                Ok(true) => { ui.label(tr("Emoticon added.", "Emoticon adicionado.")); }
                Ok(false) => { ui.label(tr("Image selection cancelled.", "Seleção de imagem cancelada.")); }
                Err(error) => { ui.colored_label(Color32::from_rgb(185, 28, 28), error); }
            }
        }
        let emoticons: Vec<_> = app.data.as_ref().map(|data| data.assets.iter()
            .filter(|asset| asset.kind == AssetKind::Emoticon).cloned().collect()).unwrap_or_default();
        for asset in emoticons {
            ui.horizontal_wrapped(|ui| {
                if let Some((texture, _)) = app.texture(ui.ctx(), crate::media::images::ImageSource::File(asset.file_id)) {
                    ui.image(egui::load::SizedTexture::new(texture, Vec2::splat(28.0)));
                }
                ui.label(format!("{}  {}", asset.name, asset.shortcut.as_deref().unwrap_or("")));
                if ui.small_button(tr("Remove", "Remover")).clicked() {
                    app.send(Action::DeleteAsset(asset.id));
                }
            });
        }
    });
}

fn conversations(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Conversations", "Conversas"), tr("Reading and composing messages.", "Leitura e composição das mensagens."));
    card(ui, |ui| {
        let settings = &mut app.options.settings;
        ui.checkbox(
            &mut settings.show_timestamps,
            tr("Show message timestamps", "Mostrar horário das mensagens"),
        );
        ui.checkbox(
            &mut settings.group_messages,
            tr("Group consecutive messages", "Agrupar mensagens consecutivas"),
        );
        ui.checkbox(&mut settings.emoticons, tr("Show graphical emoticons", "Exibir emoticons gráficos"));
        ui.checkbox(
            &mut settings.nudges,
            tr("Allow nudges and window shaking", "Permitir chamar atenção e agitar a janela"),
        );
        ui.add_space(10.0);
        ui.label(tr("Default formatting", "Formatação padrão"));
        super::composer::format_toolbar(ui, &mut settings.font);
    });
    card(ui, |ui| {
        ui.label(
            RichText::new(tr("Suggested downloads folder", "Pasta sugerida para downloads"))
                .strong()
                .size(13.0)
                .color(nmessenger::HEADING),
        );
        ui.label(
            RichText::new(tr(
                "Files and media received from your contacts will be saved here.",
                "Arquivos e mídias recebidos de seus contatos serão salvos nesta pasta.",
            ))
            .size(10.0)
            .color(nmessenger::MUTED),
        );
        ui.add_space(8.0);

        let is_mob = crate::platform::is_mobile();
        if is_mob {
            ui.add(
                egui::TextEdit::singleline(&mut app.options.settings.receive_directory)
                    .margin(egui::Margin::symmetric(8, 6))
                    .desired_width(f32::INFINITY)
                    .hint_text(tr("Default downloads folder", "Pasta padrão de downloads")),
            );
        } else {
            ui.horizontal(|ui| {
                let avail = (ui.available_width() - 116.0).max(120.0);
                ui.add(
                    egui::TextEdit::singleline(&mut app.options.settings.receive_directory)
                        .margin(egui::Margin::symmetric(8, 6))
                        .desired_width(avail)
                        .hint_text(tr("Choose a folder...", "Escolha uma pasta...")),
                );
                let btn = ui.add(
                    egui::Button::new(
                        RichText::new(tr("Browse...", "Procurar..."))
                            .size(11.5)
                            .strong()
                            .color(Color32::from_rgb(1, 45, 147)),
                    )
                    .fill(Color32::from_rgb(230, 242, 253))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(180, 210, 238)))
                    .corner_radius(4)
                    .min_size(Vec2::new(104.0, 28.0)),
                );
                if btn.clicked() {
                    let cur = (!app.options.settings.receive_directory.is_empty())
                        .then(|| app.options.settings.receive_directory.clone());
                    app.send(Action::PickReceiveDirectory(cur));
                }
            });
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(tr(
                "When you save received files, this directory will be suggested as default.",
                "Ao salvar arquivos recebidos, esta pasta será sugerida por padrão.",
            ))
            .size(10.0)
            .color(nmessenger::MUTED),
        );
    });
}

fn notifications(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Sounds and alerts", "Sons e alertas"), tr("Choose when to receive notifications.", "Escolha quando receber notificações."));
    card(ui, |ui| {
        ui.checkbox(&mut app.options.settings.sounds, tr("Play sounds", "Reproduzir sons"));
        ui.checkbox(
            &mut app.options.settings.message_alerts,
            tr("Notify me about new messages", "Avisar sobre novas mensagens"),
        );
        ui.checkbox(
            &mut app.options.settings.online_alerts,
            tr("Notify me when a contact signs in", "Avisar quando um contato entra"),
        );
    });
    card(ui, |ui| super::sounds::show(app, ui));
}

fn music_access(ui: &mut egui::Ui, enabled: bool) {
    #[cfg(target_os = "android")]
    if enabled {
        if crate::android::music_access_granted() {
            ui.label(RichText::new(tr("Music access enabled", "Acesso à música ativado")).small().color(nmessenger::MUTED));
        } else {
            ui.label(RichText::new(tr("Allow notification access to detect active music players.", "Permita o acesso às notificações para detectar músicas em reprodução.")).small().color(nmessenger::MUTED));
            if ui.button(tr("Open notification access", "Abrir acesso às notificações")).clicked() {
                crate::android::request_music_access();
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (ui, enabled);
}

fn device_menu(ui: &mut egui::Ui, id: &str, value: &mut String, values: &[String]) {
    let combo_w = (ui.available_width() - 12.0).clamp(160.0, 330.0);
    egui::ComboBox::from_id_salt(id)
        .selected_text(if value.is_empty() {
            tr("System default", "Padrão do sistema")
        } else {
            value.as_str()
        })
        .width(combo_w)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), tr("System default", "Padrão do sistema"));
            for name in values {
                ui.selectable_value(value, name.clone(), name);
            }
        });
}

fn media(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(
        ui,
        tr("Audio and video", "Áudio e vídeo"),
        tr("Devices used for recordings and calls.", "Dispositivos usados nas gravações e chamadas."),
    );
    card(ui, |ui| {
        ui.label(tr("Microphone", "Microfone"));
        device_menu(
            ui,
            "audio-input",
            &mut app.options.local.audio_input,
            &app.options.devices.inputs,
        );
        ui.add_space(8.0);
        ui.label(tr("Speakers / headphones", "Alto-falantes / fones"));
        device_menu(
            ui,
            "audio-output",
            &mut app.options.local.audio_output,
            &app.options.devices.outputs,
        );
        ui.add_space(8.0);
        ui.label(tr("Camera", "Câmera"));
        if cfg!(target_os = "android") {
            ui.label(RichText::new(tr("The front camera is selected automatically. Android will request permission when needed.", "A câmera frontal é selecionada automaticamente. O Android pedirá permissão quando necessário.")).small().color(nmessenger::MUTED));
        } else {
            let cam_combo_w = (ui.available_width() - 12.0).clamp(160.0, 330.0);
            egui::ComboBox::from_id_salt("camera-input")
                .selected_text(&app.options.local.camera_device)
                .width(cam_combo_w)
                .show_ui(ui, |ui| {
                    for name in &app.options.devices.cameras {
                        ui.selectable_value(&mut app.options.local.camera_device, name.clone(), name);
                    }
                });
            ui.label(
                RichText::new(tr("Camera device", "Dispositivo da câmera"))
                    .size(10.0).color(nmessenger::MUTED),
            );
            ui.text_edit_singleline(&mut app.options.local.camera_device);
        }
        ui.add_space(10.0);
        if ui.button(tr("Refresh devices", "Atualizar dispositivos")).clicked() {
            app.send(Action::DiscoverDevices);
        }
        if !cfg!(target_os = "android") { ui.label(
            RichText::new(if app.options.devices.ffmpeg {
                tr("Video capture available", "Captura de vídeo disponível")
            } else {
                tr("ffmpeg not found for video capture", "ffmpeg não encontrado para captura de vídeo")
            })
            .color(if app.options.devices.ffmpeg {
                theme::GREEN
            } else {
                Color32::from_rgb(180, 70, 40)
            }),
        ); }
        if let Some(error) = &app.options.devices.error {
            ui.colored_label(Color32::from_rgb(180, 70, 40), error);
        }
    });
}

fn privacy(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Privacy", "Privacidade"), tr("Control who can contact you.", "Controle quem pode falar com você."));
    card(ui, |ui| {
        ui.label(RichText::new(tr("Recover older messages", "Recuperar mensagens antigas")).strong().color(nmessenger::HEADING));
        ui.label(tr(
            "On the original device, export its encrypted identity. Import that file here using the original device's password to read messages addressed to it.",
            "No dispositivo original, exporte a identidade criptografada. Importe o arquivo aqui usando a senha original para ler as mensagens destinadas a ele.",
        ));
        ui.add_space(6.0);
        if ui.add_enabled(!app.options.history_backup_busy, egui::Button::new(tr("Export this device's identity...", "Exportar identidade deste dispositivo..."))).clicked() {
            app.options.history_backup_busy = true;
            app.options.history_backup_status = None;
            app.send(Action::ExportIdentityBackup);
        }
        ui.add_space(6.0);
        ui.add(egui::TextEdit::singleline(&mut app.options.history_backup_password).password(true)
            .hint_text(tr("Original device's password", "Senha do dispositivo original")).desired_width(ui.available_width().min(340.0)));
        if ui.add_enabled(!app.options.history_backup_busy && !app.options.history_backup_password.is_empty(),
            egui::Button::new(tr("Import identity...", "Importar identidade..."))).clicked() {
            app.options.history_backup_busy = true;
            app.options.history_backup_status = None;
            let password = std::mem::take(&mut app.options.history_backup_password);
            app.send(Action::ImportIdentityBackup { password });
        }
        if app.options.history_backup_busy { ui.spinner(); }
        if let Some(result) = &app.options.history_backup_status {
            match result {
                Ok(true) => { ui.label(tr("Completed successfully.", "Operação concluída.")); }
                Ok(false) => { ui.label(tr("Cancelled.", "Operação cancelada.")); }
                Err(error) => { ui.colored_label(Color32::from_rgb(185, 28, 28), error); }
            }
        }
    });
    card(ui, |ui| {
        ui.collapsing(tr("End-to-end encryption · verify devices", "Criptografia ponta a ponta · verificar dispositivos"), |ui| {
            ui.label(tr("Compare each device's code with your contact through another trusted channel. A new device or changed identity blocks communication until verified.", "Compare o código de cada dispositivo com seu contato por outro canal confiável. Um dispositivo novo ou uma identidade alterada bloqueia a comunicação até a verificação."));
            ui.add_space(8.0);
            let selected = app.data.as_ref().and_then(|data| data.conversations.iter()
                .find(|conversation| Some(conversation.id) == app.options.encryption_selected))
                .map(|conversation| app.conversation_title(conversation))
                .unwrap_or_else(|| tr("Choose a conversation", "Escolha uma conversa").into());
            egui::ComboBox::from_id_salt("encryption-conversation-selector")
                .selected_text(selected).width(ui.available_width().min(420.0))
                .show_ui(ui, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut app.options.encryption_search)
                        .hint_text(tr("Search conversations or contacts...", "Buscar conversas ou contatos..."))
                        .desired_width(ui.available_width()));
                    let query = app.options.encryption_search.trim().to_lowercase();
                    let mut conversations: Vec<_> = app.data.as_ref().map(|data| data.conversations.iter()
                        .filter_map(|conversation| {
                            let title = app.conversation_title(conversation);
                            let matches = query.is_empty() || title.to_lowercase().contains(&query)
                                || conversation.members.iter().any(|member| member.display_name.to_lowercase().contains(&query) || member.email.to_lowercase().contains(&query));
                            matches.then_some((conversation.id, title))
                        }).collect()).unwrap_or_default();
                    conversations.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.0.cmp(&b.0)));
                    if conversations.is_empty() { ui.label(tr("No matching conversations.", "Nenhuma conversa encontrada.")); }
                    else {
                        super::scroll::vertical().id_salt("encryption-conversation-list").max_height(180.0)
                            .show_rows(ui, 28.0, conversations.len(), |ui, range| {
                                for index in range {
                                    let (id, title) = &conversations[index];
                                    if ui.selectable_label(app.options.encryption_selected == Some(*id), title).clicked() {
                                        app.options.encryption_selected = Some(*id);
                                        app.options.encryption_keys = None;
                                        app.options.verification_codes.clear();
                                        app.options.roster_code.clear();
                                        app.send(Action::EncryptionKeys(*id));
                                        ui.close();
                                    }
                                }
                            });
                    }
                });
            if let Some((conversation, keys)) = app.options.encryption_keys.clone() {
                ui.add_space(8.0);
                ui.collapsing(tr("Verify conversation participants", "Verificar participantes da conversa"), |ui| {
                    ui.label(tr("Compare this code through another trusted channel before approving membership changes.", "Compare este código por outro canal confiável antes de autorizar mudanças nos participantes."));
                    let code = crate::encryption::Crypto::roster_fingerprint(conversation, &keys);
                    ui.monospace(&code);
                    if ui.small_button(tr("Copy code", "Copiar código")).clicked() { ui.ctx().copy_text(code); }
                    ui.add(egui::TextEdit::singleline(&mut app.options.roster_code).hint_text(tr("Code received through a trusted channel", "Código recebido por canal confiável")));
                    if ui.button(tr("Verify participants", "Verificar participantes")).clicked() { app.send(Action::VerifyConversationRoster { conversation, fingerprint: app.options.roster_code.clone() }); }
                });
                ui.collapsing(format!("{} ({})", tr("Devices", "Dispositivos"), keys.len()), |ui| {
                    for key in keys {
                      ui.push_id(key.device_id, |ui| {
                        let label = format!("{} · {}{}", app.name(key.user_id), key.device_id, if key.active { "" } else { tr(" · session ended", " · sessão encerrada") });
                        ui.collapsing(label, |ui| {
                        let code = crate::encryption::Crypto::fingerprint(&key);
                        ui.monospace(&code);
                        if ui.small_button(tr("Copy code", "Copiar código")).clicked() { ui.ctx().copy_text(code); }
                        let input = app.options.verification_codes.entry(key.device_id).or_default();
                        ui.add(egui::TextEdit::singleline(input).hint_text(tr("Code received through a trusted channel", "Código recebido pelo canal confiável")));
                        let supplied = input.clone();
                        if ui.button(tr("Verify this device", "Verificar este dispositivo")).clicked() {
                            app.send(Action::VerifyEncryptionKey { conversation, device: key.device_id, fingerprint: supplied });
                        }
                        });
                      });
                    }
                });
            }
        });
    });
    card(ui, |ui| {
        ui.checkbox(
            &mut app.options.settings.read_receipts,
            tr("Share read receipts", "Compartilhar confirmações de leitura"),
        );
        ui.checkbox(
            &mut app.options.settings.typing_indicators,
            tr("Share typing indicators", "Compartilhar quando estou digitando"),
        );
        ui.checkbox(
            &mut app.options.settings.notification_previews,
            tr("Show message contents in notifications", "Mostrar conteúdo das mensagens nas notificações"),
        );
        ui.checkbox(
            &mut app.options.settings.music_status,
            tr("Share music detected from a player", "Compartilhar música detectada em um player"),
        );
        ui.label(RichText::new(tr("A newly playing track replaces the current value.", "Uma música nova em reprodução substitui o valor atual.")).small().color(nmessenger::HEADING));
        music_access(ui, app.options.settings.music_status);
        ui.label(RichText::new(tr("Read receipts and typing settings are also enforced by the server. Disabling read receipts still updates your own unread counts.", "As opções de leitura e digitação também são aplicadas pelo servidor. Desativar leitura mantém seus próprios contadores de mensagens atualizados.")).small().color(nmessenger::HEADING));
    });
    card(ui, |ui| {
        ui.checkbox(
            &mut app.options.settings.allow_strangers,
            tr("Allow messages from people outside my contact list", "Permitir mensagens de pessoas fora da minha lista"),
        );
        ui.checkbox(
            &mut app.options.settings.require_group_invite,
            tr("Only join groups via invitation", "Só entrar em grupo via convite"),
        );
    });
    card(ui, |ui| {
        ui.label(RichText::new(tr("Blocked contacts", "Contatos bloqueados")).strong());
        let blocked: Vec<_> = app
            .data
            .as_ref()
            .map(|data| {
                data.contacts
                    .iter()
                    .filter(|c| c.blocked)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if blocked.is_empty() {
            ui.label(tr("No blocked contacts.", "Nenhum contato bloqueado."));
        }
        for contact in blocked {
            ui.horizontal(|ui| {
                ui.label(contact.name());
                if ui.small_button(tr("Unblock", "Desbloquear")).clicked() {
                    app.send(Action::Contact(
                        contact.user.id,
                        UpdateContactRequest {
                            blocked: Some(false),
                            ..Default::default()
                        },
                    ));
                }
            });
        }
    });
}

fn sessions(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Sessions", "Sessões"), tr("Devices signed in to your account.", "Computadores conectados à sua conta."));
    if ui.button(tr("Refresh sessions", "Atualizar sessões")).clicked() {
        app.send(Action::Sessions);
    }
    let sessions = app
        .data
        .as_ref()
        .map(|data| data.sessions.clone())
        .unwrap_or_default();
    for session in sessions {
        card(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{}{}",
                    session.device_name,
                    if session.current {
                        tr(" · this device", " · este computador")
                    } else {
                        ""
                    }
                ))
                .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "{}: {}", tr("Last activity", "Última atividade"),
                    session
                        .last_seen_at
                        .with_timezone(&chrono::Local)
                        .format("%d/%m/%Y %H:%M")
                ))
                .size(11.0)
                .color(nmessenger::MUTED),
            );
            if ui.button(tr("End session", "Encerrar sessão")).clicked() {
                app.send(Action::RevokeSession(session.id));
            }
        });
    }
}

fn connection(app: &mut MessengerApp, ui: &mut egui::Ui) {
    heading(ui, tr("Connection", "Conexão"), tr("Preferences for this device.", "Preferências deste computador."));
    card(ui, |ui| {
        let default_env_url = crate::env::default_server_url();
        ui.label(RichText::new(tr("Server address", "Endereço do servidor")).strong());

        if ui.checkbox(&mut app.options.local.custom_server, tr("Edit connection IP", "Editar IP da conexão")).changed() {
            if !app.options.local.custom_server {
                app.options.local.server = default_env_url.clone();
            }
        }

        if app.options.local.custom_server {
            ui.add_space(2.0);
            let s_res = ui.add(
                egui::TextEdit::singleline(&mut app.options.local.server)
                    .hint_text(&default_env_url)
                    .desired_width(f32::INFINITY),
            );
            if s_res.clicked() || s_res.gained_focus() {
                crate::platform::show_soft_input();
            }
            s_res.context_menu(|ui| {
                if ui.button(tr("Paste", "Colar")).clicked() {
                    if let Some(clip) = crate::platform::get_clipboard_text() {
                        app.options.local.server = clip.trim().to_string();
                    }
                    ui.close();
                }
            });
            ui.horizontal(|ui| {
                if ui.small_button(tr("Reset to default", "Restaurar padrão")).clicked() {
                    app.options.local.server = default_env_url.clone();
                    app.options.local.custom_server = false;
                }
            });
        } else {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&default_env_url)
                        .monospace()
                        .color(Color32::from_rgb(46, 131, 237)),
                );
                ui.label(
                    RichText::new(tr("(Default)", "(Padrão)"))
                        .small()
                        .color(nmessenger::MUTED),
                );
            });
        }

        ui.add_space(8.0);
        ui.label(tr("Device name", "Nome deste computador"));
        let d_res = ui.add(
            egui::TextEdit::singleline(&mut app.options.local.device).desired_width(f32::INFINITY),
        );
        if d_res.clicked() || d_res.gained_focus() {
            crate::platform::show_soft_input();
        }
        ui.checkbox(&mut app.options.local.remember, tr("Remember my email", "Lembrar meu email"));
        ui.add_space(10.0);
        let target_server = if app.options.local.custom_server && !app.options.local.server.trim().is_empty() {
            crate::env::format_server_url(&app.options.local.server)
        } else {
            default_env_url
        };
        if ui.button(tr("Test server", "Testar servidor")).clicked() {
            app.send(Action::Health(target_server));
        }
        ui.label(format!("{}: {}", tr("Current status", "Estado atual"), app.connection));
        if !app.notice.is_empty() {
            ui.label(&app.notice);
        }
    });
    card(ui, |ui| {
        ui.label(tr("Additional local library", "Biblioteca local adicional"));
        let l_res = ui.add(
            egui::TextEdit::singleline(&mut app.options.local.content_directory)
                .desired_width(f32::INFINITY),
        );
        if l_res.clicked() || l_res.gained_focus() {
            crate::platform::show_soft_input();
        }
        if ui.button(tr("Refresh library", "Atualizar biblioteca")).clicked() {
            app.send(Action::DiscoverAssets(
                app.options.local.content_directory.clone(),
            ));
        }
    });
}

fn save(app: &mut MessengerApp) {
    if app.options.display_name.trim().is_empty() {
        app.options.error = Some(tr("Enter a display name.", "Informe o nome de exibição.").into());
        return;
    }
    app.options.error = None;
    app.options.saving = true;
    if !app.options.local.custom_server || app.options.local.server.trim().is_empty() {
        app.options.local.server = crate::env::default_server_url();
    } else {
        app.options.local.server = crate::env::format_server_url(&app.options.local.server);
    }
    let now_playing = if app.options.music.trim().is_empty() {
        if !app.options.settings.music_status {
            Some(String::new())
        } else {
            None
        }
    } else {
        Some(app.options.music.clone())
    };
    app.send(Action::ApplySettings {
        profile: UpdateProfileRequest {
            display_name: Some(app.options.display_name.clone()),
            personal_message: Some(app.options.personal_message.clone()),
            now_playing,
            ..Default::default()
        },
        settings: app.options.settings.clone(),
        local: app.options.local.clone(),
    });
}
