use crate::{app::MessengerApp, i18n::tr, media::images::ImageSource, transport::Action};
use eframe::egui::{self, Color32, RichText};
use nm_protocol::MessageKind;

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if let Some(file) = app.image_viewer {
        let open = super::dialog::show(ctx, "image-viewer", tr("Image", "Imagem"), [900.0, 700.0], |ui| {
            let mut close = false;
            egui::Panel::top("image-viewer-actions").show_separator_line(false)
                .frame(egui::Frame::new().fill(Color32::TRANSPARENT).inner_margin(4))
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr("Close", "Fechar")).clicked() { close = true; }
                        if ui.button(tr("Forward...", "Encaminhar...")).clicked() {
                            app.forward_attachment = Some((file, MessageKind::Photo));
                            app.forward_search.clear();
                        }
                        if ui.button(tr("Download", "Baixar")).clicked() { app.send(Action::Download(file)); }
                    });
                });
            egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(30, 39, 51))).show(ui, |ui| {
                let area = ui.available_size();
                if let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::File(file)) {
                    let scale = (area.x / size.x.max(1.0)).min(area.y / size.y.max(1.0)).min(4.0).max(0.0);
                    ui.centered_and_justified(|ui| { ui.image((texture, size * scale)); });
                } else {
                    ui.centered_and_justified(|ui| { ui.label(RichText::new(tr("Loading image...", "Carregando imagem...")).color(Color32::WHITE)); });
                }
            });
            close
        });
        if !open { app.image_viewer = None; }
    }
    if let Some((file, kind)) = app.forward_attachment {
        let open = super::dialog::show(ctx, "forward-attachment", tr("Forward to...", "Encaminhar para..."), [460.0, 500.0], |ui| {
            let mut selected = None;
            ui.add(egui::TextEdit::singleline(&mut app.forward_search)
                .hint_text(tr("Search conversations...", "Buscar conversas..."))
                .desired_width(f32::INFINITY));
            ui.add_space(8.0);
            let query = app.forward_search.trim().to_lowercase();
            let conversations: Vec<_> = app.data.as_ref().map(|data| data.conversations.iter()
                .filter_map(|conversation| {
                    let title = app.conversation_title(conversation);
                    (query.is_empty() || title.to_lowercase().contains(&query)).then_some((conversation.id, title))
                }).collect()).unwrap_or_default();
            if conversations.is_empty() {
                ui.label(tr("No conversations found.", "Nenhuma conversa encontrada."));
            } else {
                super::scroll::vertical().id_salt("forward-conversations").auto_shrink([false, false])
                    .show_rows(ui, 32.0, conversations.len(), |ui, range| {
                        for index in range {
                            let (id, name) = &conversations[index];
                            if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(name)).clicked() { selected = Some(*id); }
                        }
                    });
            }
            if let Some(conversation) = selected {
                app.send(Action::ForwardAttachment { file, conversation, kind });
                true
            } else { false }
        });
        if !open { app.forward_attachment = None; }
    }
}
