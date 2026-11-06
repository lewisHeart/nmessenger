//! Rich Markdown renderer for NMessenger user profiles.
use super::nmessenger;
use crate::{app::MessengerApp, media::images::ImageSource};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Stroke, Vec2};

pub fn render(app: &mut MessengerApp, ui: &mut egui::Ui, text: &str) {
    if text.trim().is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new(crate::i18n::tr("No custom description yet.", "Nenhuma descrição personalizada informada."))
                    .size(11.0)
                    .color(Color32::from_rgb(140, 160, 180)),
            );
            ui.label(
                RichText::new(crate::i18n::tr("You can personalize your profile with Markdown formatting!", "Você pode personalizar seu perfil com formatação Markdown"))
                    .size(10.0)
                    .color(Color32::from_rgb(170, 185, 200)),
            );
        });
        return;
    }

    let lines: Vec<&str> = text.lines().collect();
    let mut in_quote_block = false;
    let mut quote_lines: Vec<&str> = Vec::new();

    let flush_quote = |ui: &mut egui::Ui, quote_lines: &mut Vec<&str>| {
        if quote_lines.is_empty() {
            return;
        }
        let quote_content = quote_lines.join("\n");
        quote_lines.clear();

        egui::Frame::new()
            .fill(Color32::from_rgb(240, 246, 254))
            .stroke(Stroke::NONE)
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            bar_rect.min,
                            Pos2::new(bar_rect.max.x, ui.cursor().bottom().max(bar_rect.max.y)),
                        ),
                        2,
                        Color32::from_rgb(50, 130, 220),
                    );
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        render_inlines(ui, &quote_content, 11.0, Color32::from_rgb(40, 75, 120), true);
                    });
                });
            });
        ui.add_space(4.0);
    };

    for raw_line in lines {
        let line = raw_line.trim_end();

        // Quote block handling
        if let Some(stripped) = line.strip_prefix("> ") {
            in_quote_block = true;
            quote_lines.push(stripped);
            continue;
        } else if in_quote_block {
            in_quote_block = false;
            flush_quote(ui, &mut quote_lines);
        }

        // Empty line
        if line.trim().is_empty() {
            ui.add_space(6.0);
            continue;
        }

        // Horizontal rule: --- or ***
        if line.trim() == "---" || line.trim() == "***" || line.trim() == "___" {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);
            continue;
        }

        // Headings
        if let Some(h1) = line.strip_prefix("# ") {
            ui.add_space(6.0);
            ui.label(
                RichText::new(h1.trim())
                    .strong()
                    .size(16.0)
                    .color(nmessenger::HEADING),
            );
            ui.painter().hline(
                ui.cursor().left()..=ui.cursor().left() + (ui.available_width() * 0.9).min(350.0),
                ui.cursor().top() - 1.0,
                Stroke::new(1.0, Color32::from_rgb(200, 222, 244)),
            );
            ui.add_space(4.0);
            continue;
        }
        if let Some(h2) = line.strip_prefix("## ") {
            ui.add_space(5.0);
            ui.label(
                RichText::new(h2.trim())
                    .strong()
                    .size(13.5)
                    .color(Color32::from_rgb(10, 60, 130)),
            );
            ui.add_space(2.0);
            continue;
        }
        if let Some(h3) = line.strip_prefix("### ") {
            ui.add_space(4.0);
            ui.label(
                RichText::new(h3.trim())
                    .strong()
                    .size(12.0)
                    .color(Color32::from_rgb(40, 80, 140)),
            );
            ui.add_space(2.0);
            continue;
        }

        // Standalone image: ![alt](url)
        if let Some(caps) = parse_standalone_image(line) {
            render_image(app, ui, &caps.0, &caps.1);
            continue;
        }

        // Bullet point: - or *
        if let Some(bullet) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("•").size(12.0).color(Color32::from_rgb(46, 131, 237)));
                render_inlines(ui, bullet, 11.5, nmessenger::INK, false);
            });
            continue;
        }

        // Regular paragraph line with inlines
        render_inlines(ui, line, 11.5, nmessenger::INK, false);
    }

    if in_quote_block {
        flush_quote(ui, &mut quote_lines);
    }
}

fn parse_standalone_image(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.starts_with("![") && trimmed.ends_with(')') {
        let close_bracket = trimmed.find("](")?;
        let alt = trimmed[2..close_bracket].to_string();
        let url = trimmed[close_bracket + 2..trimmed.len() - 1].to_string();
        return Some((alt, url));
    }
    None
}

fn render_image(app: &mut MessengerApp, ui: &mut egui::Ui, alt: &str, url: &str) {
    ui.add_space(4.0);
    // 1. Check if it's a bundled resource asset (badges/..., icons/..., emojis/...)
    if let Some(texture) = nmessenger::texture(ui.ctx(), url) {
        let size = texture.size_vec2();
        let display_size = if size.x > 0.0 && size.y > 0.0 {
            let max_w = 260.0_f32.min(ui.available_width() - 20.0);
            let max_h = 200.0_f32;
            let ratio = (max_w / size.x).min(max_h / size.y).min(1.0);
            size * ratio
        } else {
            Vec2::new(48.0, 48.0)
        };

        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 245)))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                ui.image(egui::load::SizedTexture::new(texture.id(), display_size));
                if !alt.is_empty() {
                    ui.label(
                        RichText::new(alt)
                            .size(9.5)
                            .color(Color32::from_rgb(120, 140, 160)),
                    );
                }
            });
        ui.add_space(4.0);
        return;
    }

    // 2. Check if it's an uploaded file (UUID or files/{uuid} or /files/{uuid})
    let file_id = url
        .strip_prefix("files/")
        .or_else(|| url.strip_prefix("/files/"))
        .unwrap_or(url)
        .parse::<uuid::Uuid>()
        .ok();
    if let Some(uuid) = file_id {
        if let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::File(uuid)) {
            let max_w = 260.0_f32.min(ui.available_width() - 20.0);
            let max_h = 200.0_f32;
            let ratio = if size.x > 0.0 && size.y > 0.0 {
                (max_w / size.x).min(max_h / size.y).min(1.0)
            } else {
                1.0
            };
            let display_size = size * ratio;

            egui::Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 245)))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.image(egui::load::SizedTexture::new(texture, display_size));
                    if !alt.is_empty() {
                        ui.label(
                            RichText::new(alt)
                                .size(9.5)
                                .color(Color32::from_rgb(120, 140, 160)),
                        );
                    }
                });
            ui.add_space(4.0);
            return;
        } else {
            // Spinner / loading state
            egui::Frame::new()
                .fill(Color32::from_rgb(248, 252, 255))
                .stroke(Stroke::new(1.0, Color32::from_rgb(210, 230, 248)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(if alt.is_empty() { "Carregando imagem..." } else { alt })
                                .size(10.5)
                                .color(nmessenger::MUTED),
                        );
                    });
                });
            ui.add_space(4.0);
            return;
        }
    }

    // 3. Check if it's a local file path
    let local_path = std::path::PathBuf::from(url.strip_prefix("file://").unwrap_or(url));
    if local_path.is_file() {
        if let Some((texture, size)) = app.texture(ui.ctx(), ImageSource::Local(local_path)) {
            let max_w = 260.0_f32.min(ui.available_width() - 20.0);
            let max_h = 200.0_f32;
            let ratio = if size.x > 0.0 && size.y > 0.0 {
                (max_w / size.x).min(max_h / size.y).min(1.0)
            } else {
                1.0
            };
            let display_size = size * ratio;

            egui::Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, Color32::from_rgb(215, 230, 245)))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.image(egui::load::SizedTexture::new(texture, display_size));
                    if !alt.is_empty() {
                        ui.label(
                            RichText::new(alt)
                                .size(9.5)
                                .color(Color32::from_rgb(120, 140, 160)),
                        );
                    }
                });
            ui.add_space(4.0);
            return;
        }
    }

    // 4. Fallback link (no missing font glyphs)
    ui.horizontal(|ui| {
        nmessenger::image(ui, "icons/framed-picture.png", Vec2::splat(14.0));
        ui.hyperlink_to(
            RichText::new(if alt.is_empty() { url } else { alt })
                .size(11.0)
                .color(Color32::from_rgb(10, 80, 180)),
            url,
        );
    });
    ui.add_space(4.0);
}

fn render_inlines(
    ui: &mut egui::Ui,
    text: &str,
    font_size: f32,
    default_color: Color32,
    in_quote: bool,
) {
    ui.horizontal_wrapped(|ui| {
        let mut current_word = String::new();

        let flush_current = |ui: &mut egui::Ui, current_word: &mut String| {
            if current_word.is_empty() {
                return;
            }
            let word = std::mem::take(current_word);
            let mut rt = RichText::new(word).size(font_size).color(default_color);
            if in_quote {
                rt = rt.italics();
            }
            ui.label(rt);
        };

        // Simple token scanning for **bold**, *italic*, [link](url), `code`
        let mut i = 0;
        let bytes = text.as_bytes();
        let len = bytes.len();

        while i < len {
            // Link: [text](url)
            if bytes[i] == b'[' {
                if let Some(close_b) = text[i..].find("](") {
                    let end_link = text[i + close_b..].find(')');
                    if let Some(end_paren) = end_link {
                        flush_current(ui, &mut current_word);
                        let link_text = &text[i + 1..i + close_b];
                        let link_url = &text[i + close_b + 2..i + close_b + end_paren];
                        ui.hyperlink_to(
                            RichText::new(link_text)
                                .size(font_size)
                                .color(Color32::from_rgb(10, 80, 180)),
                            link_url,
                        );
                        i = i + close_b + end_paren + 1;
                        continue;
                    }
                }
            }

            // Bold: **text**
            if bytes[i..].starts_with(b"**") {
                if let Some(end_bold) = text[i + 2..].find("**") {
                    flush_current(ui, &mut current_word);
                    let bold_text = &text[i + 2..i + 2 + end_bold];
                    ui.label(
                        RichText::new(bold_text)
                            .strong()
                            .size(font_size)
                            .color(if in_quote {
                                Color32::from_rgb(20, 55, 95)
                            } else {
                                nmessenger::HEADING
                            }),
                    );
                    i = i + 2 + end_bold + 2;
                    continue;
                }
            }

            // Italic: *text*
            if bytes[i] == b'*' && !bytes[i..].starts_with(b"**") {
                if let Some(end_italic) = text[i + 1..].find('*') {
                    flush_current(ui, &mut current_word);
                    let italic_text = &text[i + 1..i + 1 + end_italic];
                    ui.label(
                        RichText::new(italic_text)
                            .italics()
                            .size(font_size)
                            .color(default_color),
                    );
                    i = i + 1 + end_italic + 1;
                    continue;
                }
            }

            // Code: `code`
            if bytes[i] == b'`' {
                if let Some(end_code) = text[i + 1..].find('`') {
                    flush_current(ui, &mut current_word);
                    let code_text = &text[i + 1..i + 1 + end_code];
                    egui::Frame::new()
                        .fill(Color32::from_rgb(238, 242, 248))
                        .corner_radius(3)
                        .inner_margin(egui::Margin::symmetric(4, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(code_text)
                                    .monospace()
                                    .size(font_size - 0.5)
                                    .color(Color32::from_rgb(160, 40, 40)),
                            );
                        });
                    i = i + 1 + end_code + 1;
                    continue;
                }
            }

            // Character by character
            let ch = text[i..].chars().next().unwrap();
            current_word.push(ch);
            i += ch.len_utf8();
        }

        flush_current(ui, &mut current_word);
    });
}
