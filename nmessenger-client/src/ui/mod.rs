mod activities;
mod admin;
mod announcements;
mod assets;
mod audio_player;
mod avatar_picker;
mod call_panel;
mod chat;
mod composer;
mod contact_dialogs;
mod contact_preview;
mod contacts;
mod conversation_sidebar;
mod dialog;
mod emoticons;
mod games;
mod image_viewer;
mod glass;
mod login;
mod language;
pub mod markdown;
mod message_style;
mod messages;
#[cfg(any(target_os = "android", test))]
mod mobile_call;
pub(crate) mod nmessenger;
mod profile;
pub mod profile_edit;
pub mod profile_view;
mod remote;
mod scroll;
mod settings;
mod sounds;
pub mod theme;
#[cfg(any(not(target_os = "android"), test))]
pub mod toast;
pub(crate) mod window;
mod winks;

use crate::app::MessengerApp;
use eframe::egui;

pub fn shell(app: &mut MessengerApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    ctx.options_mut(|options| {
        options.input_options.surrender_focus_on = egui::SurrenderFocusOn::Never;
    });
    window::record_size(&ctx, "main");
    if crate::platform::is_mobile() && ctx.input_mut(|input| {
        input.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)
            || input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
    }) {
        mobile_back(app);
    }
    window::scale(&ctx, app.login.prefs.ui_scale);
    window::configure(&ctx, app.login.prefs.system_titlebars);
    nmessenger::set_theme(
        &ctx,
        if app.options.open {
            app.options.settings.theme.as_str()
        } else {
            app.data.as_ref().map(|data| data.settings.theme.as_str()).unwrap_or("azul")
        },
    );
    let main_title = if let Some(data) = &app.data {
        let name = data.me.display_name.trim();
        let email = data.me.email.trim();
        if !name.is_empty() && !email.is_empty() && name != email {
            format!("{name} ({email}) - NMessenger")
        } else if !name.is_empty() {
            format!("{name} - NMessenger")
        } else if !email.is_empty() {
            format!("{email} - NMessenger")
        } else {
            "NMessenger".to_string()
        }
    } else {
        "NMessenger".to_string()
    };
    let top_inset = crate::platform::safe_area_top();
    if top_inset > 0.0 {
        egui::Panel::top("safe-area-top")
            .exact_size(top_inset)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(0),
            )
            .show(ui, |ui| {
                ui.allocate_space(egui::vec2(ui.available_width(), top_inset));
            });
    }
    let keyboard_inset = crate::platform::keyboard_height(&ctx);
    if keyboard_inset > 0.0 {
        egui::Panel::bottom("safe-area-keyboard")
            .exact_size(keyboard_inset)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(0),
            )
            .show(ui, |ui| {
                ui.allocate_space(egui::vec2(ui.available_width(), keyboard_inset));
            });
    }
    theme::window_bar_with_controls(ui, &main_title, theme::WindowControls::All);
    if app.data.is_none() {
        login::show(app, ui);
        return;
    }
    #[cfg(target_os = "android")]
    {
        if app.active.is_some() {
            chat::mobile(app, ui);
        } else {
            announcements::banner(app, ui);
            profile::show(app, ui);
            contacts::show(app, ui);
        }
        contact_dialogs::show(app, &ctx);
        settings::show(app, &ctx);
        avatar_picker::show(app, &ctx);
        admin::show(app, &ctx);
        if !app.options.open {
            announcements::show(app, &ctx);
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        announcements::banner(app, ui);
        profile::show(app, ui);
        contacts::show(app, ui);
        chat::show(app, &ctx);
        contact_dialogs::show(app, &ctx);
        settings::show(app, &ctx);
        avatar_picker::show(app, &ctx);
        admin::show(app, &ctx);
        if !app.options.open {
            announcements::show(app, &ctx);
        }
    }
    image_viewer::show(app, &ctx);
    errors(app, &ctx);
}

fn mobile_back(app: &mut MessengerApp) {
    use zeroize::Zeroize;
    if app.editing_message.is_some() {
        app.editing_message = None;
        app.edit_message_draft.clear();
        app.edit_message_busy = false;
        app.edit_message_error = None;
    } else if app.forward_attachment.is_some() {
        app.forward_attachment = None;
    } else if app.image_viewer.is_some() {
        app.image_viewer = None;
    } else if app.announcements.open {
        app.announcements.open = false;
    } else if app.admin.open && app.admin.pending.is_some() {
        app.admin.pending = None;
        app.admin.password.zeroize();
        app.admin.new_password.zeroize();
    } else if app.admin.open && app.admin.managing_user_badges.is_some() {
        app.admin.managing_user_badges = None;
    } else if app.admin.open {
        app.admin.open = false;
    } else if app.options.avatar_picker {
        app.options.avatar_picker = false;
        app.options.avatar_choice = None;
        app.options.avatar_error = None;
    } else if app.options.open {
        app.options.open = false;
    } else if app.contact_forms.edit_profile_open {
        app.contact_forms.edit_profile_open = false;
    } else if app.contact_forms.view_profile.is_some() {
        app.contact_forms.view_profile = None;
    } else if app.contact_forms.group_open {
        app.contact_forms.group_open = false;
        app.contact_forms.group_members.clear();
        app.contact_forms.group_title.clear();
        app.contact_forms.group_search.clear();
        app.contact_forms.group_filter = crate::app::GroupContactFilter::All;
    } else if app.contact_forms.category_open {
        app.contact_forms.category_open = false;
    } else if app.contact_forms.add_open {
        app.contact_forms.add_open = false;
    } else if app.options.game_browser.is_some() {
        app.options.game_browser = None;
    } else if app.conversation_forms.clear_confirm {
        app.conversation_forms.clear_confirm = false;
    } else if app.conversation_forms.members_open {
        app.conversation_forms.members_open = false;
    } else if let Some(history) = app.active.and_then(|id| app.histories.get_mut(&id))
        && history.show_media_gallery {
        history.show_media_gallery = false;
    } else if app.mobile_call_view.is_some() {
        app.mobile_call_view = None;
    } else if app.active.is_some() {
        app.active = None;
    } else if app.contact_filter != crate::app::ContactFilter::All {
        app.contact_filter = crate::app::ContactFilter::All;
    } else if app.login.connection_open {
        app.login.connection_open = false;
    } else if app.login.register {
        app.login.register = false;
    }
}

fn errors(app: &mut MessengerApp, ctx: &egui::Context) {
    let Some(error) = app.errors.back().cloned() else {
        return;
    };
    // Warnings fade out on their own instead of covering the interface.
    if app.error_at.elapsed() > std::time::Duration::from_secs(9) {
        return;
    }
    egui::Area::new(egui::Id::unique("nmessenger-error"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-14.0, -14.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(egui::Color32::from_white_alpha(246))
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgb(225, 206, 206),
                ))
                .corner_radius(9)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_width(288.0);
                    ui.horizontal(|ui| {
                        let (rect, _) = ui
                            .allocate_exact_size(egui::Vec2::new(3.0, 40.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(rect, 2, egui::Color32::from_rgb(198, 92, 92));
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Aviso")
                                        .size(11.0)
                                        .strong()
                                        .color(crate::ui::nmessenger::HEADING),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(egui::Button::new("×").frame(false))
                                            .on_hover_text(crate::i18n::tr("Close", "Fechar"))
                                            .clicked()
                                        {
                                            app.errors.pop_back();
                                        }
                                    },
                                );
                            });
                            ui.add(egui::Label::new(egui::RichText::new(&error).size(10.5)).wrap());
                        });
                    });
                });
            crate::ui::glass::rim(ui.painter(), frame.response.rect, 9);
        });
}
