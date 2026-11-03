use crate::i18n::{LanguagePreference, tr};
use eframe::egui::{self, RichText};

pub fn selector(ui: &mut egui::Ui, id: &str, language: &mut LanguagePreference) {
    ui.label(
        RichText::new(tr("Language", "Idioma"))
            .size(13.0)
            .strong()
            .color(super::nmessenger::HEADING),
    );
    ui.label(
        RichText::new(tr(
            "Choose a language or follow your device's settings.",
            "Escolha um idioma ou use a configuração do dispositivo.",
        ))
        .size(10.0)
        .color(super::nmessenger::MUTED),
    );
    ui.add_space(8.0);
    let selected = match language {
        LanguagePreference::System => tr("System default", "Padrão do sistema"),
        LanguagePreference::English => "English",
        LanguagePreference::Portuguese => "Português",
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected)
        .width(ui.available_width().min(280.0))
        .show_ui(ui, |ui| {
            ui.selectable_value(language, LanguagePreference::System, tr("System default", "Padrão do sistema"));
            ui.selectable_value(language, LanguagePreference::English, "English");
            ui.selectable_value(language, LanguagePreference::Portuguese, "Português");
        });
}
