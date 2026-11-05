use super::theme;
use eframe::egui::{self, Color32, Vec2};

pub fn show_deferred(
    shared: std::sync::Arc<std::sync::Mutex<crate::app::MessengerApp>>,
    ctx: &egui::Context,
    identity: &'static str,
    title: &str,
    size: [f32; 2],
    render: impl Fn(&mut crate::app::MessengerApp, &mut egui::Ui) -> bool + Send + Sync + 'static,
    on_close: impl Fn(&mut crate::app::MessengerApp) + Send + Sync + 'static,
) {
    let mut builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_decorations(super::window::system_titlebars(ctx))
        .with_transparent(true);
    if !crate::platform::is_mobile() {
        builder = builder.with_min_inner_size([size[0].min(460.0), size[1].min(320.0)]);
        if !super::window::has_viewport_initialized(ctx, identity) {
            builder = builder.with_inner_size(super::window::initial_size(ctx, identity, size));
            super::window::mark_viewport_initialized(ctx, identity);
        }
    }

    let viewport_id = egui::ViewportId::from_hash_of(identity);
    let title_str = title.to_string();

    ctx.show_viewport_deferred(
        viewport_id,
        builder,
        move |ui, class| {
            let mut guard = shared.lock().unwrap();
            let app = &mut *guard;
            if class != egui::ViewportClass::EmbeddedWindow {
                super::window::record_size(ui.ctx(), identity);
            }
            if !crate::platform::is_mobile() {
                super::window::minimum(ui.ctx(), Vec2::new(size[0].min(460.0), size[1].min(320.0)));
            }

            let mut close = ui
                .ctx()
                .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape));

            theme::window_bar_with_controls(ui, &title_str, theme::WindowControls::CloseOnly);
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(16),
                )
                .show(ui, |ui| {
                    if matches!(identity, "user-profile-view" | "user-profile-edit") {
                        super::scroll::vertical()
                            .id_salt((identity, "viewport-scroll"))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                close |= ui.push_id(identity, |ui| render(app, ui)).inner;
                            });
                    } else {
                        close |= ui.push_id(identity, |ui| render(app, ui)).inner;
                    }
                });

            if close {
                super::window::forget_initial_size(ui.ctx(), identity);
                on_close(app);
            }
        },
    );
}

/// A dialog owns a viewport instead of being clipped to the compact contact list.
/// `content` returns true when the operation is submitted or cancelled.
pub fn show(
    ctx: &egui::Context,
    identity: &'static str,
    title: &str,
    size: [f32; 2],
    mut content: impl FnMut(&mut egui::Ui) -> bool,
) -> bool {
    let mut open = true;
    let is_mobile = crate::platform::is_mobile();
    if is_mobile {
        return mobile_page(ctx, identity, title, |ui| {
            if matches!(identity, "user-profile-view" | "user-profile-edit" | "nmessenger-games") {
                super::scroll::vertical().id_salt((identity, "mobile-scroll"))
                    .auto_shrink([false, false]).show(ui, |ui| content(ui)).inner
            } else {
                content(ui)
            }
        });
    }
    let mut builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_decorations(super::window::system_titlebars(ctx))
        .with_transparent(true);
    if !is_mobile {
        builder = builder.with_min_inner_size([size[0].min(460.0), size[1].min(320.0)]);
        if !super::window::has_viewport_initialized(ctx, identity) {
            builder = builder.with_inner_size(super::window::initial_size(ctx, identity, size));
            super::window::mark_viewport_initialized(ctx, identity);
        }
    }
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of(identity),
        builder,
        |ui, class| {
            if class != egui::ViewportClass::EmbeddedWindow { super::window::record_size(ui.ctx(), identity); }
            if !is_mobile {
                super::window::minimum(ui.ctx(), Vec2::new(size[0].min(460.0), size[1].min(320.0)));
            }
            if ui
                .ctx()
                .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
            {
                open = false;
                return;
            }
            if class == egui::ViewportClass::EmbeddedWindow {
                let mut done = false;
                let screen = ui.ctx().content_rect();
                let win_w = (screen.width() - 16.0).min(size[0]);
                let win_h = (screen.height() - 24.0).min(size[1]);
                let mut window = egui::Window::new(title)
                    .title_bar(false)
                    .collapsible(false)
                    .vscroll(false)
                    .hscroll(false)
                    .frame(frame())
                    .id(egui::Id::unique(identity))
                    .open(&mut open);
                if is_mobile {
                    window = window
                        .default_size([win_w, win_h])
                        .max_size([screen.width() - 8.0, screen.height() - 16.0]);
                } else {
                    window = window
                        .default_size([win_w, win_h])
                        .max_size([win_w, win_h]);
                }
                window.show(ui.ctx(), |ui| {
                    done |= embedded_title(ui, title);
                    if matches!(identity, "nmessenger-avatar-picker" | "nmessenger-admin" | "announcements-dialog") {
                        done |= content(ui);
                    } else {
                        super::scroll::vertical()
                            .id_salt((identity, "dialog-scroll"))
                            .show(ui, |ui| {
                                done |= content(ui);
                            });
                    }
                });
                open &= !done;
            } else {
                theme::window_bar_with_controls(ui, title, theme::WindowControls::CloseOnly);
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(Color32::TRANSPARENT)
                            .inner_margin(16),
                    )
                    .show(ui, |ui| {
                        if matches!(identity, "user-profile-view" | "user-profile-edit") {
                            super::scroll::vertical().id_salt((identity, "viewport-scroll"))
                                .auto_shrink([false, false]).show(ui, |ui| {
                                    open &= !ui.push_id(identity, |ui| content(ui)).inner;
                                });
                        } else {
                            open &= !ui.push_id(identity, |ui| content(ui)).inner;
                        }
                    });
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        },
    );
    if !open { super::window::forget_initial_size(ctx, identity); }
    open
}

/// Full-screen in-app page, sized to the usable viewport (including IME resize).
pub fn mobile_page(
    ctx: &egui::Context,
    identity: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    content: impl FnOnce(&mut egui::Ui) -> bool,
) -> bool {
    let screen = ctx.content_rect();
    let mut done = false;
    let top_inset = crate::platform::safe_area_top();
    let pad_top = (8.0 + top_inset).round() as i8;
    let keyboard_inset = crate::platform::keyboard_height(ctx);
    let pad_bottom = (8.0 + keyboard_inset).round() as i8;
    egui::Area::new(egui::Id::unique(("mobile-page", identity)))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .movable(false)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            ui.set_max_size(screen.size());
            egui::Frame::new()
                .fill(Color32::from_rgb(250, 253, 255))
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: pad_top,
                    bottom: pad_bottom,
                })
                .show(ui, |ui| {
                    let avail = (screen.size() - Vec2::new(24.0, (16.0 + top_inset + keyboard_inset).round())).max(Vec2::ZERO);
                    ui.set_min_size(avail);
                    ui.set_max_size(avail);
                    done |= embedded_title(ui, title);
                    done |= content(ui);
                });
        });
    !done
}

pub fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(252, 254, 255))
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(195, 218, 238)))
        .corner_radius(10)
        .inner_margin(14)
}

pub fn embedded_title(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    let is_mobile = crate::platform::is_mobile();
    ui.horizontal(|ui| {
        if is_mobile {
            close = ui.add(egui::Button::new(egui::RichText::new("‹").size(26.0))
                .fill(Color32::TRANSPARENT).stroke(egui::Stroke::NONE)
                .min_size(Vec2::new(44.0, 44.0))).on_hover_text(crate::i18n::tr("Back", "Voltar")).clicked();
        }
        let title = if is_mobile { title.strip_suffix(" - NMessenger").unwrap_or(title) } else { title };
        ui.add(egui::Label::new(
            egui::RichText::new(title)
                .size(if is_mobile { 16.0 } else { 15.0 })
                .strong()
                .color(super::nmessenger::HEADING),
        ).truncate());
        if !is_mobile { ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = super::theme::close_button(ui, 18.0)
                .on_hover_text(crate::i18n::tr("Close", "Fechar"))
                .clicked();
        }); }
    });
    ui.add_space(8.0);
    close
}

/// Returns Some(true) on submit, Some(false) on cancel, and None otherwise.
pub fn buttons(ui: &mut egui::Ui, submit: &str, enabled: bool) -> Option<bool> {
    let mut response = None;
    let is_mobile = crate::platform::is_mobile();
    let panel_h = if is_mobile { 52.0 } else { 42.0 };
    let btn_h = if is_mobile { 34.0 } else { 24.0 };
    egui::Panel::bottom(ui.scope_id().with("dialog-buttons"))
        .exact_size(panel_h)
        .show_separator_line(false)
        .frame(egui::Frame::new().inner_margin(4))
        .show(ui, |ui| {
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized([if is_mobile { 88.0 } else { 78.0 }, btn_h], egui::Button::new(crate::i18n::tr("Cancel", "Cancelar")))
                    .clicked()
                {
                    response = Some(false);
                }
                let submit_btn = if enabled {
                    theme::primary_button(
                        ui,
                        egui::RichText::new(submit)
                            .color(Color32::from_rgb(1, 45, 147))
                            .size(if is_mobile { 12.0 } else { 11.0 })
                            .strong(),
                        Vec2::new(if is_mobile { 110.0 } else { 90.0 }, btn_h),
                    )
                } else {
                    ui.add_enabled(
                        false,
                        egui::Button::new(submit).min_size(Vec2::new(if is_mobile { 110.0 } else { 90.0 }, btn_h)),
                    )
                };
                if submit_btn.clicked() {
                    response = Some(true);
                }
            });
        });
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_page_uses_available_screen_and_resizes_with_keyboard() {
        let ctx = egui::Context::default();
        for height in [640.0, 360.0] {
            let mut content_rect = None;
            let mut output = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(360.0, height))),
                ..Default::default()
            }, |ui| {
                assert!(mobile_page(ui.ctx(), "mobile-test", "Perfil", |ui| {
                    content_rect = Some(ui.available_rect_before_wrap());
                    false
                }));
            });
            output.textures_delta.clear();
            let rect = content_rect.unwrap();
            assert!(rect.width() >= 330.0 && rect.width() <= 360.0, "{rect:?}");
            assert!(rect.bottom() <= height && rect.height() > height - 100.0, "{rect:?}");
        }

        unsafe { std::env::set_var("NM_KEYBOARD_HEIGHT", "250"); }
        let mut content_rect_kb = None;
        let mut output = ctx.run_ui(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(360.0, 640.0))),
            ..Default::default()
        }, |ui| {
            mobile_page(ui.ctx(), "mobile-test-kb", "Perfil", |ui| {
                content_rect_kb = Some(ui.available_rect_before_wrap());
                false
            });
        });
        output.textures_delta.clear();
        unsafe { std::env::remove_var("NM_KEYBOARD_HEIGHT"); }
        let rect_kb = content_rect_kb.unwrap();
        assert!(rect_kb.height() <= 640.0 - 250.0, "{rect_kb:?}");
    }

}
