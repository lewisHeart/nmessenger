use super::{nmessenger, theme};
use crate::{app::MessengerApp, transport::Action};
use crate::i18n::tr;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2};

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
            let has_error = app.errors.back().is_some();
            let content_height = (if has_error { 355.0 } else { 330.0 })
                + if app.login.identity_recovery { 48.0 } else { 0.0 };
            let avail = ui.available_rect_before_wrap();
            let needs_scroll = avail.height() < content_height + 20.0;

            if !needs_scroll {
                let scroll_id = egui::Id::unique("login-scroll");
                ui.ctx().data_mut(|d| {
                    d.remove::<egui::scroll_area::State>(scroll_id);
                });
                let top = avail.top() + ((avail.height() - content_height) * 0.5).max(12.0);
                draw_login_form(app, ui, ctx, avail, top);
                ui.allocate_space(avail.size());
            } else {
                super::scroll::vertical()
                    .id_salt("login-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let inner_r = ui.available_rect_before_wrap();
                        let top = inner_r.top() + 10.0;
                        draw_login_form(app, ui, ctx, inner_r, top);
                        ui.allocate_space(Vec2::new(inner_r.width(), content_height + 20.0));
                    });
            }
        });
    connection(app, ctx);
    registration(app, ctx);
}

fn draw_login_form(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    r: Rect,
    top: f32,
) {
    let width = (if crate::platform::is_mobile() { 360.0_f32 } else { 248.0_f32 }).min(r.width() - 32.0);
    let left = r.center().x - width / 2.0;

    let avatar_size = 88.0_f32;
    let avatar = Rect::from_min_size(
        Pos2::new(r.center().x - avatar_size / 2.0, top),
        Vec2::splat(avatar_size),
    );
    let inner = nmessenger::frame(
        ctx,
        ui.painter(),
        avatar,
        app.login.prefs.presence,
        "login",
    );
    let cached_path = {
        crate::transport::normalize_server(&app.login.prefs.server).ok().and_then(|server| {
            let path = crate::storage::cached_avatar_path(&server, &app.login.prefs.email);
            (!app.login.prefs.email.trim().is_empty() && path.exists()).then_some(path)
        })
    };
    if let Some(path) = cached_path
        && let Some((texture, size)) =
            app.texture(ctx, crate::transport::ImageSource::Local(path))
    {
        ui.put(
            inner,
            egui::Image::new((texture, inner.size()))
                .uv(nmessenger::cover_uv(size, inner.size()))
                .corner_radius(nmessenger::frame_radius(inner)),
        );
    }
    nmessenger::paint_avatar_rim(ui.painter(), inner, app.login.prefs.presence);
    if app.login.busy {
        let angle = ctx.input(|i| i.time) as f32 * 3.0;
        let center = avatar.center();
        for index in 0..8 {
            let a = angle + index as f32 * std::f32::consts::TAU / 8.0;
            let point = center + Vec2::new(a.cos(), a.sin()) * 16.0;
            ui.painter().circle_filled(
                point,
                2.5,
                Color32::from_rgba_unmultiplied(46, 131, 237, 35 + index * 27),
            );
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }

    let mut curr_y = top + avatar_size + 10.0;

    if let Some(error) = app.errors.back() {
        let rect = Rect::from_min_size(
            Pos2::new(left, curr_y),
            Vec2::new(width, 22.0),
        );
        ui.painter()
            .rect_filled(rect, 4, Color32::from_rgb(254, 242, 242));
        ui.painter().rect_stroke(
            rect,
            4,
            Stroke::new(1.0, Color32::from_rgb(254, 202, 202)),
            egui::StrokeKind::Inside,
        );
        let response = ui.put(
            rect.shrink2(Vec2::new(8.0, 2.0)),
            egui::Label::new(
                RichText::new(error)
                    .size(10.0)
                    .color(Color32::from_rgb(185, 28, 28)),
            )
            .truncate(),
        );
        response.on_hover_text(error);
        curr_y += 28.0;
    }

    let email = nmessenger::input(
        ui,
        Rect::from_min_size(Pos2::new(left, curr_y), Vec2::new(width, 26.0)),
        "nmessenger-email",
        &mut app.login.prefs.email,
        "exemplo@mail.com",
        false,
    );
    curr_y += 32.0;

    let password = nmessenger::input(
        ui,
        Rect::from_min_size(Pos2::new(left, curr_y), Vec2::new(width, 26.0)),
        "nmessenger-password",
        &mut app.login.password,
        tr("Password", "Senha"),
        true,
    );
    curr_y += 28.0;

    if app.login.identity_recovery {
        ui.painter().text(
            Pos2::new(left, curr_y + 7.0),
            egui::Align2::LEFT_CENTER,
            tr("Previous identity password on this device:", "Senha anterior da identidade neste dispositivo:"),
            FontId::proportional(9.5),
            nmessenger::INK,
        );
        curr_y += 17.0;
        nmessenger::input(
            ui,
            Rect::from_min_size(Pos2::new(left, curr_y), Vec2::new(width, 26.0)),
            "nmessenger-identity-password",
            &mut app.login.identity_password,
            tr("Previous password", "Senha anterior"),
            true,
        );
        curr_y += 31.0;
    }

    if email.changed() || password.changed() {
        app.errors.clear();
    }

    if ui
        .put(
            Rect::from_min_size(
                Pos2::new(left, curr_y),
                Vec2::new(100.0, 16.0),
            ),
            egui::Link::new(RichText::new(tr("Forgot password?", "Esqueci a senha?")).size(10.0)),
        )
        .clicked()
    {
        app.error(
            tr("Contact the server administrator to recover your password.", "Entre em contato com o administrador do servidor para recuperar sua senha."),
        );
    }
    curr_y += 20.0;

    ui.painter().text(
        Pos2::new(left, curr_y + 11.0),
        egui::Align2::LEFT_CENTER,
        tr("Sign in as:", "Logar como:"),
        FontId::proportional(10.0),
        nmessenger::INK,
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(
            Pos2::new(left + 60.0, curr_y),
            Vec2::new(width - 60.0, 22.0),
        )),
        |ui| {
            nmessenger::presence_menu(
                ui,
                "login-presence",
                &mut app.login.prefs.presence,
                true,
            );
        },
    );
    curr_y += 26.0;

    nmessenger::checkbox(
        ui,
        Rect::from_min_size(Pos2::new(left, curr_y), Vec2::new(width, 18.0)),
        &mut app.login.prefs.remember,
        tr("Remember my email", "Lembrar meu email"),
        true,
    );
    if !app.login.prefs.remember {
        app.login.prefs.auto_login = false;
    }
    curr_y += 22.0;

    nmessenger::checkbox(
        ui,
        Rect::from_min_size(Pos2::new(left, curr_y), Vec2::new(135.0, 18.0)),
        &mut app.login.prefs.auto_login,
        tr("Sign in automatically", "Logar automaticamente"),
        true,
    );
    if app.login.prefs.auto_login {
        app.login.prefs.remember = true;
    }
    if ui
        .put(
            Rect::from_min_size(
                Pos2::new(left + width - 45.0, curr_y),
                Vec2::new(45.0, 18.0),
            ),
            egui::Link::new(RichText::new(tr("Options", "Opções")).size(10.0)),
        )
        .clicked()
    {
        app.login.connection_open = true;
        app.login.connection_draft = Some(app.login.prefs.clone());
    }
    curr_y += 28.0;

    // Centralized "Entrar" button
    let btn_width = 150.0_f32.min(width);
    let btn_left = r.center().x - btn_width / 2.0;
    let button_rect =
        Rect::from_min_size(Pos2::new(btn_left, curr_y), Vec2::new(btn_width, 32.0));
    let button = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(button_rect)
                .layout(egui::Layout::centered_and_justified(
                    egui::Direction::LeftToRight,
                )),
            |ui| {
                theme::primary_button(
                    ui,
                    RichText::new(if app.login.busy {
                        tr("Signing in...", "Entrando...")
                    } else {
                        tr("Sign in", "Entrar")
                    })
                    .size(11.0)
                    .family(egui::FontFamily::Name("Segoe UI Bold".into()))
                    .color(Color32::from_rgb(1, 45, 147)),
                    Vec2::new(btn_width, 32.0),
                )
            },
        )
        .inner;
    curr_y += 38.0;

    let enter =
        (email.has_focus() || password.has_focus() || password.lost_focus())
            && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if !app.login.busy && (button.clicked() || enter) {
        app.sign_in();
    }

    // Centralized "Criar conta" link
    let register_width = 100.0_f32;
    let register_rect = Rect::from_min_size(
        Pos2::new(r.center().x - register_width / 2.0, curr_y),
        Vec2::new(register_width, 18.0),
    );
    let register_clicked = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(register_rect)
                .layout(egui::Layout::centered_and_justified(
                    egui::Direction::LeftToRight,
                )),
            |ui| {
                ui.add(egui::Link::new(
                    RichText::new(tr("Create account", "Criar conta"))
                        .size(10.5)
                        .color(nmessenger::BLUE),
                ))
            },
        )
        .inner
        .clicked();
    if register_clicked {
        app.login.register = true;
    }
}

fn registration(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.login.register {
        return;
    }
    if crate::platform::is_mobile() {
        let open = super::dialog::mobile_page(ctx, "mobile-register", tr("Create account", "Criar conta"), |ui| {
            super::scroll::vertical()
                .id_salt("mobile-register-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(tr("Display name:", "Nome de exibição:"));
                    let r1 = ui.add(
                        egui::TextEdit::singleline(&mut app.login.name)
                            .id(egui::Id::unique("register-name-input"))
                            .desired_width(f32::INFINITY),
                    );
                    if r1.clicked() || r1.gained_focus() { crate::platform::show_soft_input_type("text"); }
                    r1.context_menu(|ui| {
                        if ui.button(tr("Paste", "Colar")).clicked() {
                            if let Some(clip) = crate::platform::get_clipboard_text() { app.login.name = clip.trim().to_string(); }
                            ui.close();
                        }
                    });
                    ui.label("Email:");
                    let r2 = ui.add(
                        egui::TextEdit::singleline(&mut app.login.prefs.email)
                            .id(egui::Id::unique("register-email-input"))
                            .desired_width(f32::INFINITY),
                    );
                    if r2.clicked() || r2.gained_focus() { crate::platform::show_soft_input_type("email"); }
                    r2.context_menu(|ui| {
                        if ui.button(tr("Paste", "Colar")).clicked() {
                            if let Some(clip) = crate::platform::get_clipboard_text() { app.login.prefs.email = clip.trim().to_string(); }
                            ui.close();
                        }
                    });
                    ui.label(tr("Password:", "Senha:"));
                    let r3 = ui.add(
                        egui::TextEdit::singleline(&mut app.login.password)
                            .id(egui::Id::unique("register-password-input"))
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    if r3.clicked() || r3.gained_focus() { crate::platform::show_soft_input_type("password"); }
                    r3.context_menu(|ui| {
                        if ui.button(tr("Paste", "Colar")).clicked() {
                            if let Some(clip) = crate::platform::get_clipboard_text() { app.login.password = clip.trim().to_string(); }
                            ui.close();
                        }
                    });
                    ui.add_space(12.0);
                    if theme::primary(ui, tr("Create account", "Criar conta")).clicked() {
                        crate::platform::hide_soft_input();
                        app.sign_in();
                        return app.login.busy;
                    }
                    false
                })
                .inner
        });
        app.login.register = open;
        return;
    }
    let mut open = true;
    let mut submitted = false;
    let win_w = (ctx.content_rect().width() - 16.0).min(280.0);
    egui::Window::new(tr("Create account", "Criar conta"))
        .title_bar(false)
        .collapsible(false)
        .frame(super::dialog::frame())
        .open(&mut open)
        .resizable(false)
        .default_width(win_w)
        .max_width(win_w)
        .show(ctx, |ui| {
            super::scroll::vertical()
                .id_salt("register-scroll")
                .show(ui, |ui| {
                    submitted |= super::dialog::embedded_title(ui, tr("Create account", "Criar conta"));
                    ui.label(tr("Display name:", "Nome de exibição:"));
                    ui.text_edit_singleline(&mut app.login.name);
                    ui.label("Email:");
                    ui.text_edit_singleline(&mut app.login.prefs.email);
                    ui.label(tr("Password:", "Senha:"));
                    ui.add(egui::TextEdit::singleline(&mut app.login.password).password(true));
                    ui.add_space(8.0);
                    if theme::primary(ui, tr("Create account", "Criar conta")).clicked() {
                        app.sign_in();
                        submitted = true;
                    }
                });
        });
    if !open || submitted {
        app.login.register = false;
    }
}

fn connection(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.login.connection_open {
        return;
    }
    let mut draft = app.login.connection_draft.take().unwrap_or_else(|| app.login.prefs.clone());
    let default_env_url = crate::env::default_server_url();
    let open = super::dialog::show(ctx, "login-options", tr("Options", "Opções"), [460.0, 480.0], |ui| {
        let mut close = false;
        super::scroll::vertical().id_salt("login-options-scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.heading(tr("Connection", "Conexão"));
            ui.label(RichText::new(tr("Choose the server and identify this device.", "Escolha o servidor e identifique este dispositivo.")).small().color(nmessenger::MUTED));
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Server address", "Endereço do servidor")).strong());

            if ui.checkbox(&mut draft.custom_server, tr("Edit connection IP", "Editar IP da conexão")).changed() {
                if !draft.custom_server {
                    draft.server = default_env_url.clone();
                }
            }

            if draft.custom_server {
                ui.add_space(2.0);
                let s_res = ui.add(
                    egui::TextEdit::singleline(&mut draft.server)
                        .id(egui::Id::unique("login-options-server"))
                        .hint_text(&default_env_url)
                        .desired_width(f32::INFINITY),
                );
                if s_res.clicked() || s_res.gained_focus() { crate::platform::show_soft_input(); }
                s_res.context_menu(|ui| {
                    if ui.button(tr("Paste", "Colar")).clicked() {
                        if let Some(clip) = crate::platform::get_clipboard_text() { draft.server = clip.trim().to_string(); }
                        ui.close();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.small_button(tr("Reset to default (env)", "Restaurar padrão")).clicked() {
                        draft.server = default_env_url.clone();
                        draft.custom_server = false;
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

            ui.add_space(6.0);
            ui.label(tr("Device name", "Nome do dispositivo"));
            let d_res = ui.add(
                egui::TextEdit::singleline(&mut draft.device)
                    .id(egui::Id::unique("login-options-device"))
                    .desired_width(f32::INFINITY),
            );
            if d_res.clicked() || d_res.gained_focus() { crate::platform::show_soft_input(); }
            d_res.context_menu(|ui| {
                if ui.button(tr("Paste", "Colar")).clicked() {
                    if let Some(clip) = crate::platform::get_clipboard_text() { draft.device = clip.trim().to_string(); }
                    ui.close();
                }
            });
            ui.add_space(8.0);
            if ui.button(tr("Test connection", "Testar conexão")).clicked() {
                let target = if draft.custom_server && !draft.server.trim().is_empty() {
                    crate::env::format_server_url(&draft.server)
                } else {
                    default_env_url.clone()
                };
                app.send(Action::Health(target));
            }
            if !app.notice.is_empty() {
                ui.label(RichText::new(&app.notice).size(11.0));
            }
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            super::language::selector(ui, "login-options-language", &mut draft.language);
            ui.add_space(18.0);
            ui.horizontal_wrapped(|ui| {
                if theme::primary(ui, tr("Apply", "Aplicar")).clicked() {
                    if !draft.custom_server || draft.server.trim().is_empty() {
                        draft.server = default_env_url.clone();
                        draft.custom_server = false;
                    } else {
                        draft.server = crate::env::format_server_url(&draft.server);
                    }
                    crate::i18n::set_preference(draft.language);
                    app.login.prefs = draft.clone();
                    app.send(Action::SavePreferences(draft.clone()));
                    ctx.request_repaint();
                    close = true;
                }
                if ui.button(tr("Cancel", "Cancelar")).clicked() {
                    close = true;
                }
            });
        });
        close
    });
    app.login.connection_open = open;
    if open {
        app.login.connection_draft = Some(draft);
    }
}
