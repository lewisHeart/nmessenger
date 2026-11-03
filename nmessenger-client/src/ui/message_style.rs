use super::nmessenger;
use crate::i18n::tr;
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke, TextFormat, Vec2};
use nm_protocol::MessageStyle;

pub fn font(style: &MessageStyle) -> FontId {
    let key = style.font_family.trim().to_lowercase();
    let family = match key.as_str() {
        "monospace" | "mono" => {
            if style.bold && style.italic {
                FontFamily::Name("Monospace Bold Italic".into())
            } else if style.bold {
                FontFamily::Name("Monospace Bold".into())
            } else if style.italic {
                FontFamily::Name("Monospace Italic".into())
            } else {
                FontFamily::Monospace
            }
        }
        "ubuntu" => FontFamily::Name(
            match (style.bold, style.italic) {
                (true, true) => "Ubuntu Bold Italic",
                (true, false) => "Ubuntu Bold",
                (false, true) => "Ubuntu Italic",
                _ => "Ubuntu",
            }
            .into(),
        ),
        "comic" | "comic-sans" | "comic sans" | "comic sans ms" => FontFamily::Name(
            match (style.bold, style.italic) {
                (true, true) => "Comic Sans MS Bold Italic",
                (true, false) => "Comic Sans MS Bold",
                (false, true) => "Comic Sans MS Italic",
                _ => "Comic Sans MS",
            }
            .into(),
        ),
        "tahoma" => FontFamily::Name(
            match (style.bold, style.italic) {
                (true, true) => "Tahoma Bold Italic",
                (true, false) => "Tahoma Bold",
                (false, true) => "Tahoma Italic",
                _ => "Tahoma",
            }
            .into(),
        ),
        "verdana" => FontFamily::Name(
            match (style.bold, style.italic) {
                (true, true) => "Verdana Bold Italic",
                (true, false) => "Verdana Bold",
                (false, true) => "Verdana Italic",
                _ => "Verdana",
            }
            .into(),
        ),
        _ => FontFamily::Name(
            match (style.bold, style.italic) {
                (true, true) => "Segoe UI Bold Italic",
                (true, false) => "Segoe UI Bold",
                (false, true) => "Segoe UI Italic",
                _ => "Segoe UI",
            }
            .into(),
        ),
    };
    FontId::new(style.font_size.clamp(8.0, 48.0), family)
}

pub fn format(style: &MessageStyle) -> TextFormat {
    let color = Color32::from_rgb(style.color[0], style.color[1], style.color[2]);
    TextFormat {
        font_id: font(style),
        color,
        italics: style.italic,
        underline: if style.underline {
            Stroke::new(1.0_f32, color)
        } else {
            Stroke::NONE
        },
        extra_letter_spacing: if style.bold { 0.4 } else { 0.0 },
        ..Default::default()
    }
}

pub fn rich(text: &str, style: &MessageStyle) -> RichText {
    let mut rich = RichText::new(text)
        .font(font(style))
        .color(format(style).color);
    if style.underline {
        rich = rich.underline();
    }
    if style.bold {
        rich = rich.strong();
    }
    if style.italic {
        rich = rich.italics();
    }
    rich
}

pub fn toolbar(ui: &mut egui::Ui, style: &mut MessageStyle) {
    ui.set_min_width(300.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr("Message formatting", "Formatação da mensagem"))
                .strong()
                .size(12.0)
                .color(nmessenger::HEADING),
        );
    });
    ui.add_space(6.0);
    toolbar_content(ui, style);
}

pub fn toolbar_content(ui: &mut egui::Ui, style: &mut MessageStyle) {

    // Seleção de Fonte (Segoe UI, Comic Sans, Tahoma, Verdana, Ubuntu, Monoespaçada)
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(tr("Font:", "Fonte:")).size(11.0).color(nmessenger::MUTED));
        for (value, label, font_family_name) in [
            ("sans-serif", "Segoe UI", "Segoe UI"),
            ("comic", "Comic Sans", "Comic Sans MS"),
            ("tahoma", "Tahoma", "Tahoma"),
            ("verdana", "Verdana", "Verdana"),
            ("ubuntu", "Ubuntu", "Ubuntu"),
            ("monospace", tr("Mono", "Monoespaçada"), "monospace"),
        ] {
            let key = style.font_family.trim().to_lowercase();
            let selected = match value {
                "sans-serif" => {
                    key.is_empty()
                        || key == "sans-serif"
                        || key == "segoe ui"
                        || key == "segoe-ui"
                }
                "comic" => {
                    key == "comic"
                        || key == "comic-sans"
                        || key == "comic sans"
                        || key == "comic sans ms"
                }
                "tahoma" => key == "tahoma",
                "verdana" => key == "verdana",
                "ubuntu" => key == "ubuntu",
                "monospace" => key == "monospace" || key == "mono",
                _ => key == value,
            };
            let font_display = if font_family_name == "monospace" {
                FontFamily::Monospace
            } else {
                FontFamily::Name(font_family_name.into())
            };
            let text = RichText::new(label)
                .size(11.0)
                .font(FontId::new(11.0, font_display))
                .color(if selected {
                    Color32::from_rgb(1, 45, 147)
                } else {
                    nmessenger::INK
                });
            let btn = ui.add(
                egui::Button::new(text)
                    .fill(if selected {
                        Color32::from_rgb(221, 237, 250)
                    } else {
                        Color32::from_white_alpha(200)
                    })
                    .stroke(if selected {
                        Stroke::new(1.0, Color32::from_rgb(140, 195, 240))
                    } else {
                        Stroke::new(0.6, nmessenger::BORDER)
                    })
                    .corner_radius(4)
                    .min_size(Vec2::new(56.0, 22.0)),
            );
            if btn.clicked() {
                style.font_family = value.into();
            }
        }
    });
    ui.add_space(4.0);

    // Tamanho da Fonte com botões de diminuir/aumentar e presets
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        ui.label(
            RichText::new(tr("Size:", "Tamanho:"))
                .size(11.0)
                .color(nmessenger::MUTED),
        );
        if ui
            .add(
                egui::Button::new(RichText::new("−").strong().size(12.0))
                    .min_size(Vec2::new(22.0, 22.0))
                    .corner_radius(3),
            )
            .on_hover_text(tr("Decrease size", "Diminuir tamanho"))
            .clicked()
        {
            style.font_size = (style.font_size - 1.0).max(8.0);
        }
        ui.add(
            egui::DragValue::new(&mut style.font_size)
                .range(8.0..=36.0)
                .speed(0.5)
                .suffix(" px"),
        )
        .on_hover_text(tr("Adjust font size", "Ajustar tamanho da fonte"));
        if ui
            .add(
                egui::Button::new(RichText::new("+").strong().size(12.0))
                    .min_size(Vec2::new(22.0, 22.0))
                    .corner_radius(3),
            )
            .on_hover_text(tr("Increase size", "Aumentar tamanho"))
            .clicked()
        {
            style.font_size = (style.font_size + 1.0).min(36.0);
        }
        ui.add_space(4.0);
        for preset in [9.0, 11.0, 12.0, 14.0, 16.0] {
            let selected = (style.font_size - preset).abs() < 0.5;
            let text = RichText::new(format!("{:.0}", preset))
                .size(10.5)
                .color(if selected {
                    Color32::from_rgb(1, 45, 147)
                } else {
                    nmessenger::MUTED
                });
            if ui
                .add(
                    egui::Button::new(text)
                        .fill(if selected {
                            Color32::from_rgb(221, 237, 250)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(if selected {
                            Stroke::new(1.0, Color32::from_rgb(140, 195, 240))
                        } else {
                            Stroke::NONE
                        })
                        .corner_radius(3)
                        .min_size(Vec2::new(20.0, 20.0)),
                )
                .clicked()
            {
                style.font_size = preset;
            }
        }
    });
    ui.add_space(4.0);

    // Efeitos: Negrito, Itálico, Sublinhado e Direção
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        let toggle_btn = |ui: &mut egui::Ui, active: &mut bool, text: RichText, tooltip: &str| {
            let is_active = *active;
            let btn = ui.add(
                egui::Button::new(text.color(if is_active {
                    Color32::from_rgb(1, 45, 147)
                } else {
                    nmessenger::INK
                }))
                .fill(if is_active {
                    Color32::from_rgb(215, 235, 252)
                } else {
                    Color32::from_white_alpha(190)
                })
                .stroke(if is_active {
                    Stroke::new(1.0, Color32::from_rgb(120, 180, 240))
                } else {
                    Stroke::new(0.6, nmessenger::BORDER)
                })
                .corner_radius(4)
                .min_size(Vec2::new(26.0, 24.0)),
            );
            if btn.clicked() {
                *active = !*active;
            }
            btn.on_hover_text(tooltip);
        };

        toggle_btn(
            ui,
            &mut style.bold,
            RichText::new(tr("B", "N")).strong(),
            tr("Bold", "Negrito"),
        );
        toggle_btn(
            ui,
            &mut style.italic,
            RichText::new("I").italics(),
            tr("Italic", "Itálico"),
        );
        toggle_btn(
            ui,
            &mut style.underline,
            RichText::new(tr("U", "S")).underline(),
            tr("Underline", "Sublinhado"),
        );

        ui.add_space(6.0);
        let rtl_active = style.rtl;
        let rtl_btn = ui.add(
            egui::Button::new(
                RichText::new(tr("Right to left", "Direita para esquerda"))
                    .size(10.5)
                    .color(if rtl_active {
                        Color32::from_rgb(1, 45, 147)
                    } else {
                        nmessenger::INK
                    }),
            )
            .fill(if rtl_active {
                Color32::from_rgb(215, 235, 252)
            } else {
                Color32::from_white_alpha(190)
            })
            .stroke(if rtl_active {
                Stroke::new(1.0, Color32::from_rgb(120, 180, 240))
            } else {
                Stroke::new(0.6, nmessenger::BORDER)
            })
            .corner_radius(4)
            .min_size(Vec2::new(30.0, 24.0)),
        );
        if rtl_btn.clicked() {
            style.rtl = !style.rtl;
        }
        rtl_btn.on_hover_text(tr(
            "Align text from right to left",
            "Alinhar texto da direita para a esquerda",
        ));
    });
    ui.add_space(4.0);

    // Paleta de Cores MSN + Seletor Personalizado
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        ui.label(RichText::new(tr("Color:", "Cor:")).size(11.0).color(nmessenger::MUTED));
        const PALETTE: &[[u8; 3]] = &[
            [0, 0, 0],      // Preto
            [10, 80, 160],  // Azul clássico
            [220, 50, 40],  // Vermelho
            [34, 139, 34],  // Verde
            [230, 120, 20], // Laranja
            [128, 0, 128],  // Roxo
            [220, 70, 140], // Rosa
            [139, 69, 19],  // Marrom
            [90, 90, 90],   // Cinza escuro
        ];
        for &color in PALETTE {
            let selected = style.color == color;
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
            let fill = Color32::from_rgb(color[0], color[1], color[2]);
            ui.painter().rect_filled(rect, 3, fill);
            if selected {
                ui.painter().rect_stroke(
                    rect.expand(2.0),
                    4,
                    Stroke::new(2.0, Color32::from_rgb(1, 45, 147)),
                    egui::StrokeKind::Outside,
                );
            } else if response.hovered() {
                ui.painter().rect_stroke(
                    rect.expand(1.0),
                    3,
                    Stroke::new(1.0, Color32::from_rgb(160, 205, 240)),
                    egui::StrokeKind::Outside,
                );
            }
            if response.clicked() {
                style.color = color;
            }
        }
        ui.add_space(4.0);
        ui.color_edit_button_srgb(&mut style.color)
            .on_hover_text(tr("Choose custom color", "Escolher cor personalizada"));
    });
    ui.add_space(8.0);

    // Caixa de pré-visualização ao vivo
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, nmessenger::BORDER))
        .corner_radius(6)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let label = egui::Label::new(rich(
                tr(
                    "Here is how your message will look",
                    "Assim ficará sua mensagem",
                ),
                style,
            ));
            if style.rtl {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    ui.add(label.wrap());
                });
            } else {
                ui.add(label.wrap());
            }
        });
    ui.label(
        RichText::new(tr(
            "Formatting is sent with the message.",
            "A formatação acompanha a mensagem enviada.",
        ))
        .small()
        .color(nmessenger::MUTED),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_family_and_weight_variants_resolve_correctly() {
        let mut style = MessageStyle::default();
        style.font_family = "comic".into();
        style.bold = false;
        assert_eq!(font(&style).family, FontFamily::Name("Comic Sans MS".into()));

        style.bold = true;
        assert_eq!(font(&style).family, FontFamily::Name("Comic Sans MS Bold".into()));

        style.font_family = "ubuntu".into();
        assert_eq!(font(&style).family, FontFamily::Name("Ubuntu Bold".into()));

        style.bold = false;
        assert_eq!(font(&style).family, FontFamily::Name("Ubuntu".into()));

        style.font_family = "monospace".into();
        assert_eq!(font(&style).family, FontFamily::Monospace);

        style.bold = true;
        assert_eq!(font(&style).family, FontFamily::Name("Monospace Bold".into()));
    }
}
