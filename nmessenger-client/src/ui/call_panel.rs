use super::nmessenger;
use crate::{app::MessengerApp, i18n::tr, media::{audio, calls::Component}, transport::Action};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use nm_protocol::*;

fn component_name(component: Component) -> &'static str {
    match component {
        Component::Input => tr("Microphone", "Microfone"),
        Component::Output => tr("Speakers", "Saída de áudio"),
        Component::Camera => tr("Camera", "Câmera"),
        Component::Screen => tr("Screen", "Tela"),
        Component::RemoteAudio => tr("Incoming audio", "Áudio recebido"),
        Component::RemoteVideo => tr("Incoming video", "Vídeo recebido"),
    }
}

fn device_help(component: Component) -> &'static str {
    match component {
        Component::Input => tr("Microphone unavailable. Check permissions and choose another device.", "Microfone indisponível. Verifique as permissões ou escolha outro dispositivo."),
        Component::Output => tr("Audio output unavailable. Choose another playback device.", "Saída de áudio indisponível. Escolha outro dispositivo de reprodução."),
        Component::Camera => tr("Camera unavailable. Check that it is connected, not in use, and selected in Devices.", "Câmera indisponível. Confira se está conectada, livre e selecionada em Dispositivos."),
        Component::Screen => tr("Screen sharing was denied or stopped. Try again and grant permission.", "O compartilhamento foi negado ou interrompido. Tente novamente e conceda a permissão."),
        Component::RemoteAudio => tr("Unable to receive audio.", "Não foi possível receber o áudio."),
        Component::RemoteVideo => tr("Unable to receive video.", "Não foi possível receber o vídeo."),
    }
}

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui, activity: &Activity) {
    let current = app.current_call == Some(activity.id);
    let status = app.media_status.as_ref().filter(|status| status.activity == activity.id).cloned();
    let has_errors = status.as_ref().is_some_and(|status| !status.errors.is_empty());
    let connected = status.as_ref().is_some_and(|status| status.ready.contains(&Component::RemoteAudio));
    let state = if !current {
        tr("Available on this device", "Disponível neste dispositivo")
    } else if status.as_ref().is_some_and(|status| status.paused) {
        tr("Reconnecting…", "Reconectando…")
    } else if has_errors {
        tr("Device needs attention", "Verifique os dispositivos")
    } else if connected {
        tr("Connected", "Conectada")
    } else {
        tr("Connecting…", "Conectando…")
    };
    egui::Frame::new()
        .fill(Color32::from_rgb(248, 251, 255))
        .stroke(Stroke::new(1.0, Color32::from_rgb(220, 232, 244)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let compact = ui.available_width() < 820.0;
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), egui::Sense::hover());
                ui.painter().circle_filled(icon_rect.center(), 17.0, Color32::from_rgb(225, 240, 253));
                nmessenger::paint(ui.ctx(), ui.painter(),
                    if activity.kind == ActivityKind::VideoCall { "icons/camera.png" } else { "icons/call.png" },
                    icon_rect.shrink(8.0));
                ui.add_space(4.0);
                let title = if activity.kind == ActivityKind::VideoCall {
                    tr("Video call", "Videochamada")
                } else {
                    tr("Voice call", "Chamada de voz")
                };
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(title).strong().size(12.5).color(nmessenger::HEADING));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(6.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 3.0, if has_errors {
                            Color32::from_rgb(217, 144, 48)
                        } else if connected {
                            Color32::from_rgb(61, 166, 105)
                        } else { Color32::from_rgb(160, 174, 188) });
                        ui.label(RichText::new(state).size(10.0).color(nmessenger::MUTED));
                    });
                });
                if !compact {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        controls(app, ui, activity, current, status.as_ref());
                    });
                }
            });
            if compact {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| controls(app, ui, activity, current, status.as_ref()));
            }
            if let Some(status) = &status && has_errors {
                ui.add_space(5.0);
                let warning = ui.add(egui::Button::new(
                    RichText::new(tr("Device issue · View details", "Problema com dispositivo · Ver detalhes"))
                        .size(10.0).color(Color32::from_rgb(150, 93, 28)),
                ).fill(Color32::from_rgb(255, 246, 229))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(242, 224, 191)))
                    .corner_radius(5));
                egui::Popup::menu(&warning).show(|ui| {
                    ui.set_max_width(300.0);
                    for component in status.errors.keys() {
                        ui.label(RichText::new(component_name(*component)).strong());
                        ui.label(RichText::new(device_help(*component)).small());
                        ui.add_space(4.0);
                    }
                    if ui.button(tr("Open devices", "Abrir dispositivos")).clicked() {
                        app.open_options();
                        app.options.tab = 7;
                        ui.close();
                    }
                    if status.errors.contains_key(&Component::Screen) && ui.button(tr("Retry screen sharing", "Tentar compartilhar novamente")).clicked() {
                        app.set_screen_sharing(true);
                        ui.close();
                    }
                    if (status.paused || status.errors.keys().any(|component| *component != Component::Screen))
                        && ui.button(tr("Retry devices", "Tentar dispositivos novamente")).clicked() {
                        app.send(Action::StartCall { activity: activity.clone(), config: app.media_config(), restart: true });
                        ui.close();
                    }
                    ui.collapsing(tr("Technical details", "Detalhes técnicos"), |ui| {
                        for (component, error) in &status.errors {
                            ui.label(RichText::new(format!("{}: {error}", component_name(*component))).small().color(nmessenger::MUTED));
                        }
                    });
                });
            }
            let me = app.data.as_ref().map(|data| data.me.id);
            let shared = app.screen_frames.iter()
                .find(|((id, user), _)| *id == activity.id && Some(*user) != me)
                .or_else(|| app.screen_frames.iter().find(|((id, _), _)| *id == activity.id))
                .map(|(_, texture)| texture.clone());
            if let Some(texture) = shared {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Shared screen", "Tela compartilhada")).small().strong());
                let size = texture.size_vec2();
                if size.x > 0.0 && size.y > 0.0 {
                    let scale = (ui.available_width() / size.x).min(240.0 / size.y).min(1.0);
                    ui.image((texture.id(), size * scale));
                }
            }
        });
}

fn controls(app: &mut MessengerApp, ui: &mut egui::Ui, activity: &Activity, current: bool, status: Option<&crate::media::calls::CallStatus>) {
    let end = egui::Button::new(RichText::new(tr("End call", "Encerrar")).size(11.0).strong().color(Color32::WHITE))
        .fill(Color32::from_rgb(199, 70, 70)).stroke(Stroke::NONE).corner_radius(5)
        .min_size(Vec2::new(76.0, 28.0));
    if ui.add(end).clicked() {
        if current { app.send(Action::StopCall); }
        app.send(Action::UpdateActivity(activity.id, UpdateActivityRequest { status: ActivityStatus::Ended, payload: None }));
    }
    if current {
        let muted = status.is_some_and(|status| status.muted);
        let label = if muted { tr("Unmute", "Ativar microfone") } else { tr("Mute", "Silenciar") };
        if ui.add(egui::Button::new(RichText::new(label).size(11.0)).min_size(Vec2::new(72.0, 28.0))).clicked() {
            if let Some(status) = &mut app.media_status { status.muted = !muted; }
            app.send(Action::MuteMicrophone(!muted));
        }
        if matches!(activity.kind, ActivityKind::AudioCall | ActivityKind::VideoCall) {
            let enabled = status.map_or(activity.kind == ActivityKind::VideoCall, |status| status.camera_enabled);
            if ui.add(egui::Button::new(RichText::new(if enabled { tr("Camera off", "Desligar câmera") } else { tr("Camera on", "Ligar câmera") }).size(11.0)).min_size(Vec2::new(72.0, 28.0))).clicked() {
                app.set_camera_enabled(!enabled);
            }
            let screen = status.is_some_and(|status| status.screen_sharing);
            if ui.add(egui::Button::new(RichText::new(if screen { tr("Stop sharing", "Parar tela") } else { tr("Share screen", "Compartilhar tela") }).size(11.0)).min_size(Vec2::new(72.0, 28.0))).clicked() {
                app.set_screen_sharing(!screen);
            }
        }
        if ui.add(egui::Button::new(RichText::new(tr("Devices", "Dispositivos")).size(11.0))
            .min_size(Vec2::new(72.0, 28.0))).clicked() {
            app.open_options();
            app.options.tab = 7;
        }
    } else if ui.add_enabled(audio::AVAILABLE && !app.call_busy(Some(activity.id)),
        egui::Button::new(tr("Start media", "Iniciar mídia")).min_size(Vec2::new(80.0, 28.0))).clicked() {
        app.authorized_calls.insert(activity.id);
        app.current_call = Some(activity.id);
        app.send(Action::StartCall { activity: activity.clone(), config: app.media_config(), restart: false });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::calls::CallStatus;
    use std::collections::{BTreeMap, HashSet};

    #[test]
    fn call_bar_stays_compact_on_desktop_and_mobile_with_device_errors() {
        let mut app = MessengerApp::new_for_test();
        let activity = Activity {
            id: uuid::Uuid::new_v4(), conversation_id: uuid::Uuid::new_v4(),
            initiator_id: uuid::Uuid::new_v4(), kind: ActivityKind::VideoCall,
            status: ActivityStatus::Accepted, payload: serde_json::json!({}), created_at: chrono::Utc::now(),
        };
        app.current_call = Some(activity.id);
        app.media_status = Some(CallStatus {
            activity: activity.id, epoch: 1, ready: HashSet::new(),
            errors: BTreeMap::from([(Component::Input, "audio failure".into()), (Component::Camera, "camera failure".into())]),
            paused: false, muted: false, camera_enabled: true, screen_sharing: false,
        });
        for width in [320.0, 820.0] {
            let ctx = egui::Context::default();
            let mut height = 0.0;
            let mut output = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(width, 600.0))),
                ..Default::default()
            }, |ui| {
                let before = ui.cursor().min.y;
                show(&mut app, ui, &activity);
                height = ui.cursor().min.y - before;
            });
            output.textures_delta.clear();
            assert!(height < 160.0, "call bar too tall at width {width}: {height}");
        }
    }
}
