//! The selected reference's animated emojis, with legacy shortcut compatibility.
use super::{nmessenger, theme};
use crate::{app::MessengerApp, media::images::ImageSource, resources};
use eframe::egui::{self, Color32, FontId, RichText, Vec2};
use nm_protocol::MessageStyle;

pub struct Emoticon {
    pub shortcut: &'static str,
    pub name: &'static str,
}

macro_rules! catalog {
    ($(($shortcut:literal, $name:literal)),* $(,)?) => {
        pub const CATALOG: &[Emoticon] = &[$(Emoticon { shortcut: $shortcut, name: $name }),*];
    };
}

catalog![
    (":)", "Sorriso"),
    (":D", "Sorriso aberto"),
    (";)", "Piscadinha"),
    (":O", "Surpresa"),
    (":P", "Língua"),
    ("(H)", "Óculos escuros"),
    (":@", "Zangado"),
    (":$", "Envergonhado"),
    (":S", "Confuso"),
    (":(", "Triste"),
    (":'(", "Chorando"),
    (":|", "Indiferente"),
    ("(6)", "Diabinho"),
    ("(A)", "Anjo"),
    ("(L)", "Coração"),
    ("(U)", "Coração partido"),
    ("(M)", "Messenger"),
    ("(@)", "Gato"),
    ("(&)", "Cachorro"),
    ("(S)", "Lua"),
    ("(*)", "Estrela"),
    ("(~)", "Filme"),
    ("(8)", "Música"),
    ("(E)", "Email"),
    ("(F)", "Rosa"),
    ("(W)", "Rosa murcha"),
    ("(O)", "Relógio"),
    ("(K)", "Beijo"),
    ("(G)", "Presente"),
    ("(^)", "Bolo"),
    ("(P)", "Câmera"),
    ("(I)", "Ideia"),
    ("(C)", "Café"),
    ("(T)", "Telefone"),
    ("(Z)", "Pessoa 1"),
    ("(X)", "Pessoa 2"),
    ("(B)", "Cerveja"),
    ("(D)", "Drink"),
    ("({)", "Abraço esquerdo"),
    ("(})", "Abraço direito"),
    ("(Y)", "Positivo"),
    ("(N)", "Negativo"),
    (":wlm_42:", "Morcego"),
    (":wlm_43:", "Tigela"),
    ("(%)", "Algemas"),
    ("(#)", "Sol"),
    ("(R)", "Arco-íris"),
    (":-#", "Segredo"),
    ("8o|", "Furioso"),
    ("8-|", "Nerd"),
    ("^o)", "Desconfiado"),
    (":-*", "Segredinho"),
    ("+o(", "Doente"),
    ("(sn)", "Caracol"),
    ("(tu)", "Tartaruga"),
    ("(pl)", "Prato"),
    (":wlm_56:", "Símbolo 57"),
    ("(pi)", "Pizza"),
    ("(so)", "Futebol"),
    ("(au)", "Carro"),
    ("(ap)", "Avião"),
    ("(um)", "Guarda-chuva"),
    ("(ip)", "Ilha"),
    ("(co)", "Computador"),
    ("(mp)", "Celular"),
    ("|-)", "Dormindo"),
    ("(st)", "Chuva"),
    (":wlm_67:", "Dedos cruzados"),
    (":wlm_68:", "Mãos"),
    (":wlm_69:", "Moedas"),
    ("(bah)", "Ovelha"),
    (":^)", "Não sei"),
    ("*-)", "Pensando"),
    ("(li)", "Relâmpago"),
    ("<:o)", "Festa"),
    ("8-)", "Revirando os olhos"),
    (":wlm_76:", "Símbolo 77"),
    (":wlm_77:", "Bocejo"),
    (":wlm_78:", "Gato preto"),
    (":wlm_79:", "Coelho"),
];

const ALIASES: &[(&str, usize)] = &[
    (":-)", 0),
    (":-D", 1),
    (":d", 1),
    (":-d", 1),
    (";-)", 2),
    (":-O", 3),
    (":o", 3),
    (":-o", 3),
    (":-P", 4),
    (":p", 4),
    (":-p", 4),
    (":-@", 6),
    (":-$", 7),
    (":-S", 8),
    (":s", 8),
    (":-s", 8),
    (":-(", 9),
    (":-|", 11),
    ("<3", 14),
    ("<:O)", 74),
];

#[derive(Debug, PartialEq)]
pub enum Run<'a> {
    Text(&'a str),
    Image { spelling: &'a str, frame: usize },
}

fn prefix(text: &str) -> Option<(usize, usize)> {
    let mut found = None;
    for (pattern, frame) in CATALOG
        .iter()
        .enumerate()
        .map(|(i, e)| (e.shortcut, i))
        .chain(ALIASES.iter().copied())
        .chain(
            nmessenger::EMOJI_CATALOG
                .iter()
                .enumerate()
                .map(|(index, (code, _, _))| (*code, CATALOG.len() + index)),
        )
        .chain(
            resources::EMOJI_UNICODE
                .iter()
                .enumerate()
                .map(|(index, (code, _))| {
                    (
                        *code,
                        CATALOG.len() + nmessenger::EMOJI_CATALOG.len() + index,
                    )
                }),
        )
    {
        let matches = if pattern.starts_with('(') {
            text.get(..pattern.len())
                .is_some_and(|part| part.eq_ignore_ascii_case(pattern))
        } else {
            text.starts_with(pattern)
        };
        if matches && found.is_none_or(|(len, _)| pattern.len() > len) {
            found = Some((pattern.len(), frame));
        }
    }
    found
}

/// UTF-8 offsets and the original spelling are preserved, including aliases.
pub fn parse(text: &str) -> Vec<Run<'_>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut position = 0;
    while position < text.len() {
        let rest = &text[position..];
        let candidate = matches!(
            rest.as_bytes()[0],
            b':' | b';' | b'(' | b'<' | b'8' | b'^' | b'+' | b'*' | b'|'
        );
        if let Some((bytes, frame)) = (candidate || rest.as_bytes()[0] >= 128)
            .then(|| prefix(rest))
            .flatten()
        {
            if start < position {
                runs.push(Run::Text(&text[start..position]));
            }
            runs.push(Run::Image {
                spelling: &text[position..position + bytes],
                frame,
            });
            position += bytes;
            start = position;
        } else {
            position += rest.chars().next().unwrap().len_utf8();
        }
    }
    if start < text.len() {
        runs.push(Run::Text(&text[start..]));
    }
    runs
}

fn font(style: &MessageStyle) -> FontId {
    super::message_style::font(style)
}

fn styled(text: &str, style: &MessageStyle) -> RichText {
    super::message_style::rich(text, style)
}

pub fn show(
    app: &mut MessengerApp,
    ui: &mut egui::Ui,
    text: &str,
    style: &MessageStyle,
    enabled: bool,
) {
    if !enabled {
        ui.with_layout(
            if style.rtl {
                egui::Layout::right_to_left(egui::Align::TOP)
            } else {
                egui::Layout::top_down(egui::Align::Min)
            },
            |ui| {
                ui.add(egui::Label::new(styled(text, style)).wrap());
            },
        );
        return;
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for line in text.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let runs = parse(line);
            let layout = if style.rtl {
                egui::Layout::right_to_left(egui::Align::BOTTOM)
            } else {
                egui::Layout::left_to_right(egui::Align::BOTTOM)
            }
            .with_main_wrap(true);
            let font_height = ui.fonts_mut(|fonts| fonts.row_height(&font(style)));
            let line_height = if runs.iter().any(|run| matches!(run, Run::Image { .. })) {
                font_height.max(24.0)
            } else {
                font_height
            };
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), line_height),
                layout,
                |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    if line.is_empty() {
                        ui.add(egui::Label::new(styled("\u{200b}", style)));
                    }
                    for run in runs {
                        match run {
                            Run::Text(part) => {
                                for word in part.split_inclusive(char::is_whitespace) {
                                    let width = ui.fonts_mut(|f| {
                                        f.layout_no_wrap(word.into(), font(style), Color32::WHITE)
                                            .size()
                                            .x
                                    });
                                    let label = egui::Label::new(styled(word, style));
                                    if width > ui.max_rect().width() {
                                        ui.add(label.wrap());
                                    } else {
                                        ui.add(label.wrap_mode(egui::TextWrapMode::Extend));
                                    }
                                }
                            }
                            Run::Image { frame, spelling } => {
                                let Some(asset) = graphic(frame) else {
                                    ui.add(egui::Label::new(styled(spelling, style)));
                                    continue;
                                };
                                let (rect, response) =
                                    ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                                paint(app, ui, asset, rect);
                                let (name, code) = if frame < CATALOG.len() {
                                    (CATALOG[frame].name, CATALOG[frame].shortcut)
                                } else if frame < CATALOG.len() + nmessenger::EMOJI_CATALOG.len() {
                                    let (code, name, _) =
                                        nmessenger::EMOJI_CATALOG[frame - CATALOG.len()];
                                    (name, code)
                                } else {
                                    let (code, _) = resources::EMOJI_UNICODE
                                        [frame - CATALOG.len() - nmessenger::EMOJI_CATALOG.len()];
                                    ("Emoji", code)
                                };
                                response.on_hover_text(format!("{name}  {code}"));
                            }
                        }
                    }
                },
            );
        }
    });
}

pub fn picker(app: &mut MessengerApp, ui: &mut egui::Ui) -> Option<&'static str> {
    let mut selected = None;
    ui.set_min_width(224.0);
    ui.label(RichText::new("Emojis").strong().color(nmessenger::HEADING));
    ui.add_space(4.0);
    super::scroll::vertical()
        .max_height(208.0)
        .id_salt("nmessenger-emoji-scroll")
        .show(ui, |ui| {
            egui::Grid::new("nmessenger-emoticon-grid")
                .num_columns(6)
                .spacing(Vec2::splat(4.0))
                .show(ui, |ui| {
                    for (frame, (code, name, asset)) in nmessenger::EMOJI_CATALOG.iter().enumerate()
                    {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(34.0), egui::Sense::click());
                        if response.hovered() {
                            ui.painter()
                                .rect_filled(rect, 2, Color32::from_rgb(219, 238, 252));
                            ui.painter().rect_stroke(
                                rect,
                                2,
                                egui::Stroke::new(1.0_f32, theme::BLUE),
                                egui::StrokeKind::Inside,
                            );
                        }
                        paint(
                            app,
                            ui,
                            asset,
                            egui::Rect::from_center_size(rect.center(), Vec2::splat(24.0)),
                        );
                        if response.on_hover_text(format!("{name}  {code}")).clicked() {
                            selected = Some(*code);
                        }
                        if frame % 6 == 5 {
                            ui.end_row();
                        }
                    }
                });
        });
    selected
}

fn graphic(frame: usize) -> Option<&'static str> {
    if frame >= CATALOG.len() + nmessenger::EMOJI_CATALOG.len() {
        return resources::EMOJI_UNICODE
            .get(frame - CATALOG.len() - nmessenger::EMOJI_CATALOG.len())
            .map(|(_, asset)| *asset);
    }
    if frame >= CATALOG.len() {
        return nmessenger::EMOJI_CATALOG
            .get(frame - CATALOG.len())
            .map(|(_, _, asset)| *asset);
    }
    let name = match frame {
        0 => "slightly-smiling-face",
        1 => "grinning-face-with-big-eyes",
        2 => "winking-face",
        3 => "face-with-open-mouth",
        4 => "face-with-tongue",
        5 => "smiling-face-with-sunglasses",
        6 => "pouting-face",
        7 => "flushed-face",
        8 => "confused-face",
        9 => "frowning-face",
        10 => "loudly-crying-face",
        11 => "expressionless-face",
        12 => "smiling-face-with-horns",
        13 => "smiling-face-with-halo",
        14 => "red-heart",
        15 => "broken-heart",
        17 => "cat-face",
        18 => "dog-face",
        19 => "crescent-moon",
        20 => "star",
        22 => "musical-note",
        23 => "envelope",
        24 | 25 => "wilted-flower",
        26 => "alarm-clock",
        27 => "kiss-mark",
        28 => "wrapped-gift",
        30 => "camera",
        32 => "hot-beverage",
        40 => "thumbs-up",
        41 => "thumbs-down",
        50 => "thinking-face",
        52 => "nauseated-face",
        65 => "sleeping-face",
        72 => "thinking-face",
        74 => "partying-face",
        _ => return None,
    };
    resources::EMOJI_ANIMATIONS
        .iter()
        .find(|(preview, _)| {
            preview
                .strip_prefix("emojis/")
                .and_then(|value| value.strip_suffix(".png"))
                == Some(name)
        })
        .map(|(preview, _)| *preview)
}

pub fn paint(app: &mut MessengerApp, ui: &egui::Ui, preview: &'static str, rect: egui::Rect) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let animated = resources::FLUENT_ANIMATIONS
        .iter()
        .chain(resources::EMOJI_ANIMATIONS.iter())
        .find(|(asset, _)| *asset == preview)
        .map(|(_, asset)| *asset);
    if let Some(animated) = animated
        && let Some((texture, _)) = app.texture(ui.ctx(), ImageSource::Bundled(animated))
    {
        ui.painter().image(
            texture,
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        nmessenger::paint(ui.ctx(), ui.painter(), preview, rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_and_shortcuts_use_the_reference_animation_with_original_pixels_and_timing() {
        for input in ["😀", "❤️", ":-)", ";-)"] {
            let runs = parse(input);
            let [Run::Image { frame, .. }] = runs.as_slice() else {
                panic!("Emoji not recognized: {input}")
            };
            let asset = graphic(*frame).unwrap();
            let (_, animated) = resources::EMOJI_ANIMATIONS
                .iter()
                .find(|(preview, _)| *preview == asset)
                .unwrap();
            let frames =
                crate::media::images::decode_sized(resources::bytes(animated).unwrap(), Some(48))
                    .unwrap();
            assert!(frames.len() > 1, "{animated} must animate");
            assert!(frames.iter().all(|frame| frame.image.size[0] <= 48
                && frame.image.size[1] <= 48
                && frame.delay_ms >= 20));
            assert!(
                frames
                    .windows(2)
                    .any(|pair| pair[0].image.pixels != pair[1].image.pixels)
            );
        }
    }

    #[test]
    fn unicode_aliases_and_plain_text_round_trip_without_changing_stored_content() {
        let text = "Olá, João 👋 :-)! ;)(l)\nحسناً (Y) :wlm_79: :wlm_999: fim";
        let runs = parse(text);
        let frames: Vec<_> = runs
            .iter()
            .filter_map(|run| match run {
                Run::Image { frame, .. } => Some(*frame),
                _ => None,
            })
            .collect();
        assert_eq!(frames, [0, 2, 14, 40, 79]);
        let restored: String = runs
            .iter()
            .map(|run| match run {
                Run::Text(part) => *part,
                Run::Image { spelling, .. } => *spelling,
            })
            .collect();
        assert_eq!(restored, text);
    }

    #[test]
    fn palette_is_bounded_by_catalog_size() {
        assert_eq!(CATALOG.len(), 80);
        assert_eq!(parse(":wlm_80:"), [Run::Text(":wlm_80:")]);
    }
}
