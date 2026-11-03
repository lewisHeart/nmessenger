use crate::{app::MessengerApp, transport::Action};
use crate::i18n::tr;
use eframe::egui;
use nm_protocol::{AssetKind, UserAsset};

pub fn show(app: &mut MessengerApp, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(tr("Custom sounds", "Sons personalizados")).strong());
    let assets = app
        .data
        .as_ref()
        .map(|d| d.assets.clone())
        .unwrap_or_default();
    for (event, label) in [
        ("message", tr("New message", "Nova mensagem")),
        ("nudge", tr("Nudge", "Chamar atenção")),
        ("online", tr("Contact online", "Contato online")),
        ("call", tr("Call or activity invitation", "Convite de chamada/atividade")),
    ] {
        let name = format!("event:{event}");
        let custom: Option<UserAsset> = assets
            .iter()
            .rev()
            .find(|a| a.kind == AssetKind::Sound && a.name == name)
            .cloned();
        ui.horizontal(|ui| {
            ui.label(label);
            if ui.button(tr("Play", "Ouvir")).clicked() {
                app.send(Action::Notification {
                    file: custom.as_ref().map(|a| a.file_id),
                    event: event.into(),
                    output: app.options.local.audio_output.clone(),
                });
            }
            if ui.button(tr("Choose WAV…", "Escolher WAV…")).clicked() {
                app.send(Action::UploadAsset {
                    kind: AssetKind::Sound,
                    name,
                    shortcut: None,
                    local: None,
                });
            }
            if let Some(asset) = custom
                && ui.button(tr("Restore default", "Restaurar padrão")).clicked()
            {
                app.send(Action::DeleteAsset(asset.id));
            }
        });
    }
    ui.add(
        egui::Label::new(
            egui::RichText::new(tr("WAV PCM/float, up to 60 seconds; channels and sample rate are converted to mono 16 kHz.", "WAV PCM/float, até 60 segundos; canais e taxa são convertidos para mono 16 kHz."))
                .small()
                .color(crate::ui::nmessenger::MUTED),
        )
        .wrap(),
    );
    if !app.options.sound_status.is_empty() {
        ui.label(&app.options.sound_status);
    }
}
