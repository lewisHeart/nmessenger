use crate::{
    app::MessengerApp,
    media::remote::{Input, Key},
    transport::Action,
};
use eframe::egui::{self, RichText, Vec2};
use nm_protocol::{Activity, ClientEvent};

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, activity: &Activity, me: uuid::Uuid) {
    if activity.initiator_id == me {
        let status = app.media_status.as_ref().filter(|status| status.activity == activity.id);
        let failure = status.and_then(|status| status.errors.get(&crate::media::calls::Component::Camera)).cloned();
        let ready = status.is_some_and(|status| status.ready.contains(&crate::media::calls::Component::Camera));
        if let Some(error) = failure {
            ui.colored_label(egui::Color32::from_rgb(180, 70, 40), crate::i18n::tr("Screen sharing could not start or was stopped.", "Não foi possível iniciar o compartilhamento ou ele foi interrompido."));
            ui.collapsing(crate::i18n::tr("Details", "Detalhes"), |ui| { ui.label(&error); });
            if ui.button(crate::i18n::tr("Try again", "Tentar novamente")).clicked() {
                app.send(Action::StartCall { activity: activity.clone(), config: app.media_config(), restart: true });
            }
        } else {
            ui.label(RichText::new(if ready { "Sua tela está sendo compartilhada · 5 quadros/s" } else { "Aguardando permissão para compartilhar a tela…" })
                .color(if ready { super::theme::GREEN } else { super::nmessenger::MUTED }));
        }
        let mut enabled = app.remote_controls.contains(&activity.id);
        if crate::media::remote::control_available() && ui
            .checkbox(&mut enabled, "Permitir controle de mouse e teclado")
            .changed()
        {
            if enabled {
                app.remote_controls.insert(activity.id);
            } else {
                app.remote_controls.remove(&activity.id);
            }
            app.send(Action::RemoteControl {
                activity: activity.id,
                enabled,
            });
        }
    } else {
        let textures: Vec<_> = app
            .screen_frames
            .iter()
            .filter(|((id, user), _)| *id == activity.id && *user == activity.initiator_id)
            .map(|(_, texture)| (texture.id(), texture.size_vec2()))
            .collect();
        if textures.is_empty() {
            ui.label("Aguardando o compartilhamento do seu contato…");
        }
        for (texture, size) in textures {
            let scale = (ui.available_width().min(480.0) / size.x).min(240.0 / size.y);
            let response =
                ui.add(egui::Image::new((texture, size * scale)).sense(egui::Sense::click()));
            if activity.payload["control"].as_bool() != Some(false)
                && let Some(position) = response.interact_pointer_pos()
                && (response.clicked() || response.secondary_clicked())
            {
                signal(
                    app,
                    activity.id,
                    Input::Click {
                        x: ((position.x - response.rect.left()) / response.rect.width())
                            .clamp(0.0, 1.0),
                        y: ((position.y - response.rect.top()) / response.rect.height())
                            .clamp(0.0, 1.0),
                        button: if response.secondary_clicked() { 3 } else { 1 },
                    },
                );
            }
        }
        if activity.payload["control"].as_bool() == Some(false) {
            ui.label(RichText::new("Somente visualização: o controle remoto não está disponível neste dispositivo.").small());
            return;
        }
        ui.label(
            RichText::new("Cliques e texto são aplicados quando o contato permite controle.")
                .small(),
        );
        let mut text = app.remote_text.remove(&activity.id).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut text)
                    .char_limit(256)
                    .desired_width(180.0)
                    .hint_text("Texto remoto"),
            );
            if ui.button("Digitar").clicked() && !text.is_empty() {
                signal(
                    app,
                    activity.id,
                    Input::Text {
                        text: std::mem::take(&mut text),
                    },
                );
            }
            if ui.button("Enter").clicked() {
                signal(app, activity.id, Input::Key { key: Key::Enter });
            }
        });
        app.remote_text.insert(activity.id, text);
        ui.horizontal_wrapped(|ui| {
            for (label, key) in [
                ("Tab", Key::Tab),
                ("Esc", Key::Escape),
                ("⌫", Key::Backspace),
                ("←", Key::Left),
                ("↑", Key::Up),
                ("↓", Key::Down),
                ("→", Key::Right),
            ] {
                if ui
                    .add(egui::Button::new(label).min_size(Vec2::splat(22.0)))
                    .clicked()
                {
                    signal(app, activity.id, Input::Key { key });
                }
            }
            if ui.button("Rolar ↑").clicked() {
                signal(app, activity.id, Input::Scroll { up: true });
            }
            if ui.button("Rolar ↓").clicked() {
                signal(app, activity.id, Input::Scroll { up: false });
            }
        });
    }
}

fn signal(app: &MessengerApp, activity_id: uuid::Uuid, input: Input) {
    app.send(Action::Realtime(ClientEvent::Signal {
        activity_id,
        data: serde_json::json!({"remote_input": input}),
    }));
}
