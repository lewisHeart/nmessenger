//! Edit My Profile dialog with live Markdown preview and formatting toolbar.
use super::{dialog, nmessenger, theme};
use crate::app::MessengerApp;
use crate::i18n::tr;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use nm_protocol::*;

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.contact_forms.edit_profile_open {
        return;
    }

    let title = tr("Edit My Profile - NMessenger", "Editar Meu Perfil - NMessenger");

    if let Some(shared) = app.shared.clone() {
        if !ctx.embed_viewports() && !crate::platform::is_mobile() {
            dialog::show_deferred(
                shared,
                ctx,
                "user-profile-edit",
                &title,
                [540.0, 620.0],
                content,
                |app| {
                    app.contact_forms.edit_profile_open = false;
                },
            );
            return;
        }
    }

    let open = dialog::show(
        ctx,
        "user-profile-edit",
        &title,
        [540.0, 620.0],
        |ui| content(app, ui),
    );

    if !open {
        app.contact_forms.edit_profile_open = false;
    }
}

fn content(app: &mut MessengerApp, ui: &mut egui::Ui) -> bool {
    let Some(me) = app.data.as_ref().map(|d| d.me.clone()) else {
        return true;
    };
    let mut done = false;

    ui.horizontal(|ui| {
        theme::avatar(app, ui, &me, 56.0);
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(tr("Personalize your NMessenger profile", "Personalize seu perfil do NMessenger"))
                    .size(14.0)
                    .strong()
                    .color(nmessenger::HEADING),
            );
            ui.label(
                RichText::new(tr("Your name, personal message and Markdown bio are visible to your contacts.", "Seu nome, mensagem pessoal e biografia Markdown são visíveis para seus contatos."))
                    .size(10.5)
                    .color(nmessenger::MUTED),
            );
            ui.add_space(4.0);
            let camera_icon = nmessenger::texture(ui.ctx(), "icons/camera.png")
                .map(|t| egui::load::SizedTexture::new(t.id(), egui::vec2(14.0, 14.0)));
            let change_avatar = theme::primary_button_with_icon(
                ui,
                camera_icon,
                RichText::new(tr("Change profile picture...", "Alterar imagem de perfil..."))
                    .size(10.5)
                    .color(Color32::from_rgb(1, 45, 147))
                    .strong(),
                Vec2::new(175.0, 26.0),
            );
            if change_avatar.clicked() {
                app.options.avatar_picker = true;
            }
        });
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    // Basic Fields
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Display name:", "Nome de exibição:")).strong().size(11.0));
    });
    ui.add(
        egui::TextEdit::singleline(&mut app.contact_forms.edit_display_name)
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::symmetric(8, 6)),
    );

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Personal message:", "Mensagem pessoal (subtítulo):")).strong().size(11.0));
    });
    ui.add(
        egui::TextEdit::singleline(&mut app.contact_forms.edit_personal_message)
            .desired_width(f32::INFINITY)
            .hint_text(tr("What's on your mind?", "O que você está pensando agora?"))
            .margin(egui::Margin::symmetric(8, 6)),
    );

    ui.add_space(10.0);

    // Markdown Bio Section with Toolbar
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("About me (Markdown):", "Sobre mim (Descrição com Markdown):")).strong().size(11.5).color(nmessenger::HEADING));
    });
    ui.add_space(2.0);

    // Formatting Toolbar
    ui.horizontal_wrapped(|ui| {
        if ui.button(RichText::new("N").strong()).on_hover_text(tr("Bold: **text**", "Negrito: **texto**")).clicked() {
            app.contact_forms.edit_bio.push_str(tr("**bold text** ", "**texto em negrito** "));
        }
        if ui.button(RichText::new("I").italics()).on_hover_text(tr("Italic: *text*", "Itálico: *texto*")).clicked() {
            app.contact_forms.edit_bio.push_str(tr("*italic text* ", "*texto em itálico* "));
        }
        if ui.button(tr("Heading", "Título")).on_hover_text(tr("Heading: ## Title", "Cabeçalho: ## Título")).clicked() {
            app.contact_forms.edit_bio.push_str(tr("\n## Heading\n", "\n## Título\n"));
        }
        if ui.button(tr("Quote", "Citação")).on_hover_text(tr("Quote: > text", "Citação: > frase")).clicked() {
            app.contact_forms.edit_bio.push_str(tr("\n> Inspirational quote...\n", "\n> Citação inspiradora...\n"));
        }
        if ui.button(tr("• List", "• Lista")).on_hover_text(tr("List item", "Marcador de lista")).clicked() {
            app.contact_forms.edit_bio.push_str(tr("\n- List item\n", "\n- Item da lista\n"));
        }
        if ui.button("Link").on_hover_text(tr("Insert link: [text](url)", "Inserir Link: [texto](url)")).clicked() {
            app.contact_forms.edit_bio.push_str(tr("[My website](https://example.com) ", "[Meu site](https://exemplo.com) "));
        }
        if ui
            .button(RichText::new(tr("Import photo...", "Importar foto do PC...")).strong())
            .on_hover_text(tr("Upload an image from your device to your bio", "Importar e fazer upload de uma imagem do seu computador para a descrição"))
            .clicked()
        {
            app.send(crate::transport::Action::UploadBioImage);
        }
    });

    ui.add_space(3.0);
    ui.add(
        egui::TextEdit::multiline(&mut app.contact_forms.edit_bio)
            .desired_rows(6)
            .desired_width(f32::INFINITY)
            .hint_text(tr("Write about yourself! Supports headings (#), bold (**), italics (*), images and quotes (>).", "Escreva sobre você no estilo Orkut! Suporta títulos (#), negrito (**), itálico (*), imagens e citações (>)..."))
            .margin(egui::Margin::symmetric(8, 6)),
    );

    ui.add_space(8.0);

    // Live Preview Box
    ui.label(
        RichText::new(tr("Profile preview:", "Pré-visualização do perfil:"))
            .size(11.0)
            .strong()
            .color(nmessenger::MUTED),
    );
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 245)))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_height(90.0);
            ui.set_max_height(120.0);
            super::scroll::vertical()
                .auto_shrink([false, false])
                .id_salt("edit-preview-scroll")
                .show(ui, |ui| {
                    let bio = app.contact_forms.edit_bio.clone();
                    super::markdown::render(app, ui, &bio);
                });
        });

    ui.add_space(14.0);

    // Footer Buttons
    ui.horizontal(|ui| {
        let save_btn = theme::primary_button(
            ui,
            RichText::new(tr("Save changes", "Salvar alterações"))
                .size(11.5)
                .color(Color32::from_rgb(1, 45, 147))
                .strong(),
            Vec2::new(140.0, 28.0),
        );

        if save_btn.clicked() {
            let req = UpdateProfileRequest {
                display_name: Some(app.contact_forms.edit_display_name.trim().to_string()),
                personal_message: Some(app.contact_forms.edit_personal_message.clone()),
                bio: Some(app.contact_forms.edit_bio.clone()),
                ..Default::default()
            };
            app.send(crate::transport::Action::Profile(req));
            done = true;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(RichText::new(tr("Cancel", "Cancelar")).size(11.0)).clicked() {
                done = true;
            }
        });
    });

    done
}
