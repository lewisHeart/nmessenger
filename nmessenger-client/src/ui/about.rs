use super::nmessenger;
use crate::{app::MessengerApp, i18n::tr};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn show(app: &mut MessengerApp, ctx: &egui::Context) {
    if !app.about_open {
        return;
    }
    let is_mobile = crate::platform::is_mobile();
    if is_mobile {
        if !super::dialog::mobile_page(ctx, "nmessenger-about", tr("About NMessenger", "Sobre o NMessenger"), |ui| {
            super::scroll::vertical()
                .id_salt("about-mobile-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| { content(ui); });
            false
        }) {
            app.about_open = false;
        }
        return;
    }
    let open = super::dialog::show(
        ctx,
        "nmessenger-about",
        tr("About NMessenger", "Sobre o NMessenger"),
        [400.0, 520.0],
        |ui| {
            content(ui);
            false
        },
    );
    app.about_open = open;
}

fn content(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();

    // ── Logo + Nome ────────────────────────────────────────────────────────
    ui.vertical_centered(|ui| {
        ui.add_space(6.0);

        // Usa icon.svg (registrado no catálogo) como logo do app
        let logo_size = Vec2::splat(72.0);
        let (logo_rect, _) = ui.allocate_exact_size(logo_size, egui::Sense::hover());
        nmessenger::paint(&ctx, ui.painter(), "icon.svg", logo_rect);

        ui.add_space(10.0);
        ui.label(
            RichText::new("NMessenger")
                .font(FontId::new(22.0, egui::FontFamily::Name("Segoe UI Bold".into())))
                .color(nmessenger::HEADING),
        );

        ui.add_space(3.0);
        ui.label(
            RichText::new(format!("{} {}", tr("Version", "Versão"), APP_VERSION))
                .size(11.5)
                .color(nmessenger::MUTED),
        );

        ui.add_space(14.0);
    });

    // ── Separador ─────────────────────────────────────────────────────────
    separator(ui);
    ui.add_space(10.0);

    // ── Segurança ─────────────────────────────────────────────────────────
    section_title(ui, "🔒", tr("Security", "Segurança"));
    ui.add_space(5.0);

    egui::Frame::new()
        .fill(Color32::from_rgb(240, 247, 255))
        .stroke(Stroke::new(1.0, Color32::from_rgb(195, 220, 245)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            security_row(ui, "🔐", tr(
                "End-to-End Encryption (E2EE)",
                "Criptografia ponta a ponta (E2EE)",
            ));
            ui.add_space(4.0);
            security_row(ui, "🛡️", tr(
                "TLS 1.3 transport with verified certificates",
                "Transporte TLS 1.3 com certificados verificados",
            ));
            ui.add_space(4.0);
            security_row(ui, "🔑", tr(
                "X25519 key exchange · ChaCha20-Poly1305",
                "Troca de chaves X25519 · ChaCha20-Poly1305",
            ));
            ui.add_space(4.0);
            security_row(ui, "👁️", tr(
                "Zero knowledge: server never sees message content",
                "Zero knowledge: o servidor nunca lê o conteúdo",
            ));
        });

    ui.add_space(12.0);

    // ── Inspiração ────────────────────────────────────────────────────────
    section_title(ui, "✨", tr("Inspiration", "Inspiração"));
    ui.add_space(5.0);

    egui::Frame::new()
        .fill(Color32::from_rgb(252, 250, 255))
        .stroke(Stroke::new(1.0, Color32::from_rgb(220, 210, 240)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(tr(
                    "NMessenger brings back the expressive, nostalgic feel of instant messengers \
                     from the 2000s — particularly Windows Live Messenger — built on \
                     a modern, secure, high-performance stack.",
                    "NMessenger resgata a experiência expressiva e nostálgica dos comunicadores \
                     instantâneos dos anos 2000 — especialmente o Windows Live Messenger \
                     — sustentado por uma infraestrutura moderna, segura e de alto desempenho.",
                ))
                .size(11.5)
                .color(Color32::from_rgb(80, 60, 110)),
            );
        });

    ui.add_space(14.0);

    // ── Rodapé ────────────────────────────────────────────────────────────
    separator(ui);
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("© 2025 NMessenger")
                .size(10.5)
                .color(nmessenger::MUTED),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(tr(
                "Open Source — built with ❤️ in Rust",
                "Código aberto — feito com ❤️ em Rust",
            ))
            .size(10.5)
            .color(nmessenger::MUTED),
        );
    });
    ui.add_space(6.0);
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn separator(ui: &mut egui::Ui) {
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [r.left_top(), r.right_top()],
        Stroke::new(1.0, Color32::from_rgb(220, 230, 242)),
    );
    ui.add_space(1.0);
}

fn section_title(ui: &mut egui::Ui, icon: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(13.5));
        ui.add_space(3.0);
        ui.label(
            RichText::new(label)
                .font(FontId::new(13.0, egui::FontFamily::Name("Segoe UI Bold".into())))
                .color(nmessenger::HEADING),
        );
    });
}

fn security_row(ui: &mut egui::Ui, icon: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(12.0));
        ui.add_space(4.0);
        ui.label(
            RichText::new(label)
                .size(11.5)
                .color(Color32::from_rgb(40, 70, 110)),
        );
    });
}
