//! Resource payloads and font configurations.
use eframe::egui;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

struct FontTarget {
    family_name: &'static str,
    font_id: &'static str,
    file_candidates: &'static [&'static str],
}

// Guaranteed embedded fallback fonts:
// Bundled directly into the app so Android, iOS, and minimal OS installs
// always render emojis, Fraktur, Runes, Old Italic, Small Caps, Superscripts, etc.
const EMBEDDED_EMOJIS: &[u8] = include_bytes!("../../assets/fonts/emojis_fallback.ttf");
const EMBEDDED_SYMBOLS_MATH: &[u8] = include_bytes!("../../assets/fonts/symbols_math_fallback.otf");
const EMBEDDED_SYMBOLS_HISTORIC: &[u8] = include_bytes!("../../assets/fonts/symbols_historic_fallback.otf");
const EMBEDDED_SYMBOLS_SYLLABICS: &[u8] = include_bytes!("../../assets/fonts/symbols_syllabics_fallback.otf");
const EMBEDDED_SYMBOLS_ENCLOSED: &[u8] = include_bytes!("../../assets/fonts/symbols_enclosed_fallback.ttf");
const EMBEDDED_SYMBOLS_GAMER: &[u8] = include_bytes!("../../assets/fonts/symbols_gamer_fallback.ttf");
// GNU FreeFont (FreeSerif), subset to Unicode symbols and dingbats.
const EMBEDDED_SYMBOLS_GENERAL: &[u8] = include_bytes!("../../assets/fonts/symbols_general_fallback.otf");

const TARGETS: &[FontTarget] = &[
    FontTarget {
        family_name: "Segoe UI",
        font_id: "segoe_ui_regular",
        file_candidates: &[
            "segoeui.ttf",
            "segoe-ui.ttf",
            "segoe_ui.ttf",
            "segoe ui.ttf",
            "inter-regular.ttf",
            "cantarell-regular.ttf",
            "roboto-regular.ttf",
            "liberationsans-regular.ttf",
            "dejavusans.ttf",
        ],
    },
    FontTarget {
        family_name: "Segoe UI Bold",
        font_id: "segoe_ui_bold",
        file_candidates: &[
            "segoeuib.ttf",
            "segoe-ui-bold.ttf",
            "segoe_ui_bold.ttf",
            "segoe ui bold.ttf",
            "inter-bold.ttf",
            "cantarell-bold.ttf",
            "roboto-bold.ttf",
            "ubuntu-bold.ttf",
            "liberationsans-bold.ttf",
            "dejavusans-bold.ttf",
        ],
    },
    FontTarget {
        family_name: "Segoe UI Italic",
        font_id: "segoe_ui_italic",
        file_candidates: &[
            "segoeuii.ttf",
            "segoe-ui-italic.ttf",
            "segoe_ui_italic.ttf",
            "segoe ui italic.ttf",
            "inter-italic.ttf",
            "cantarell-italic.ttf",
            "roboto-italic.ttf",
            "ubuntu-italic.ttf",
            "liberationsans-italic.ttf",
            "dejavusans-oblique.ttf",
        ],
    },
    FontTarget {
        family_name: "Segoe UI Bold Italic",
        font_id: "segoe_ui_bold_italic",
        file_candidates: &[
            "segoeuiz.ttf",
            "segoe-ui-bold-italic.ttf",
            "segoe_ui_bold_italic.ttf",
            "segoe ui bold italic.ttf",
            "inter-bolditalic.ttf",
            "roboto-bolditalic.ttf",
            "ubuntu-bolditalic.ttf",
            "liberationsans-bolditalic.ttf",
            "dejavusans-boldoblique.ttf",
        ],
    },
    FontTarget {
        family_name: "Comic Sans MS",
        font_id: "comic_sans_regular",
        file_candidates: &[
            "comic.ttf",
            "comicsans.ttf",
            "comic-sans.ttf",
            "comic_sans.ttf",
            "comic sans ms.ttf",
            "comic-sans-ms.ttf",
            "comicrelief.ttf",
            "comic-relief.ttf",
            "comingsoon.ttf",
            "dancingscript-regular.ttf",
            "chalkboard.ttf",
            "purisa.ttf",
            "casual.ttf",
            "carlito-regular.ttf",
        ],
    },
    FontTarget {
        family_name: "Comic Sans MS Bold",
        font_id: "comic_sans_bold",
        file_candidates: &[
            "comicbd.ttf",
            "comicsans-bold.ttf",
            "comic-sans-bold.ttf",
            "comic_sans_bold.ttf",
            "comic sans ms bold.ttf",
            "comicrelief-bold.ttf",
            "comic-relief-bold.ttf",
            "chalkboard-bold.ttf",
            "purisa-bold.ttf",
            "casual-bold.ttf",
        ],
    },
    FontTarget {
        family_name: "Comic Sans MS Italic",
        font_id: "comic_sans_italic",
        file_candidates: &[
            "comici.ttf",
            "comicsans-italic.ttf",
            "comic-sans-italic.ttf",
            "comicrelief-italic.ttf",
            "chalkboard-italic.ttf",
        ],
    },
    FontTarget {
        family_name: "Comic Sans MS Bold Italic",
        font_id: "comic_sans_bold_italic",
        file_candidates: &[
            "comicz.ttf",
            "comicsans-bold-italic.ttf",
            "comic-sans-bold-italic.ttf",
            "comicrelief-bolditalic.ttf",
        ],
    },
    FontTarget {
        family_name: "Tahoma",
        font_id: "tahoma_regular",
        file_candidates: &[
            "tahoma.ttf",
            "tahoma-regular.ttf",
            "tahoma.otf",
            "dejavusans.ttf",
            "liberationsans-regular.ttf",
            "roboto-regular.ttf",
        ],
    },
    FontTarget {
        family_name: "Tahoma Bold",
        font_id: "tahoma_bold",
        file_candidates: &[
            "tahomabd.ttf",
            "tahoma-bold.ttf",
            "tahomab.ttf",
            "tahomabd.otf",
            "dejavusans-bold.ttf",
            "liberationsans-bold.ttf",
            "roboto-bold.ttf",
        ],
    },
    FontTarget {
        family_name: "Tahoma Italic",
        font_id: "tahoma_italic",
        file_candidates: &[
            "tahomai.ttf",
            "tahoma-italic.ttf",
            "dejavusans-oblique.ttf",
            "liberationsans-italic.ttf",
            "roboto-italic.ttf",
        ],
    },
    FontTarget {
        family_name: "Tahoma Bold Italic",
        font_id: "tahoma_bold_italic",
        file_candidates: &[
            "tahomaz.ttf",
            "tahoma-bold-italic.ttf",
            "dejavusans-boldoblique.ttf",
            "liberationsans-bolditalic.ttf",
            "roboto-bolditalic.ttf",
        ],
    },
    FontTarget {
        family_name: "Verdana",
        font_id: "verdana_regular",
        file_candidates: &[
            "verdana.ttf",
            "verdana-regular.ttf",
            "dejavusans.ttf",
            "liberationsans-regular.ttf",
            "roboto-regular.ttf",
        ],
    },
    FontTarget {
        family_name: "Verdana Bold",
        font_id: "verdana_bold",
        file_candidates: &[
            "verdanab.ttf",
            "verdana-bold.ttf",
            "verdanabd.ttf",
            "dejavusans-bold.ttf",
            "liberationsans-bold.ttf",
            "roboto-bold.ttf",
        ],
    },
    FontTarget {
        family_name: "Verdana Italic",
        font_id: "verdana_italic",
        file_candidates: &[
            "verdanai.ttf",
            "verdana-italic.ttf",
            "dejavusans-oblique.ttf",
            "liberationsans-italic.ttf",
            "roboto-italic.ttf",
        ],
    },
    FontTarget {
        family_name: "Verdana Bold Italic",
        font_id: "verdana_bold_italic",
        file_candidates: &[
            "verdanaz.ttf",
            "verdana-bold-italic.ttf",
            "dejavusans-boldoblique.ttf",
            "liberationsans-bolditalic.ttf",
            "roboto-bolditalic.ttf",
        ],
    },
    FontTarget {
        family_name: "Ubuntu",
        font_id: "ubuntu_regular",
        file_candidates: &[
            "ubuntu-regular.ttf",
            "ubuntu.ttf",
            "ubuntu-r.ttf",
            "roboto-regular.ttf",
        ],
    },
    FontTarget {
        family_name: "Ubuntu Bold",
        font_id: "ubuntu_bold",
        file_candidates: &[
            "ubuntu-bold.ttf",
            "ubuntub.ttf",
            "ubuntu-b.ttf",
            "roboto-bold.ttf",
            "dejavusans-bold.ttf",
        ],
    },
    FontTarget {
        family_name: "Ubuntu Italic",
        font_id: "ubuntu_italic",
        file_candidates: &[
            "ubuntu-italic.ttf",
            "ubuntui.ttf",
            "ubuntu-ri.ttf",
            "roboto-italic.ttf",
            "dejavusans-oblique.ttf",
        ],
    },
    FontTarget {
        family_name: "Ubuntu Bold Italic",
        font_id: "ubuntu_bold_italic",
        file_candidates: &[
            "ubuntu-bolditalic.ttf",
            "ubuntu-bold-italic.ttf",
            "ubuntubi.ttf",
            "ubuntu-bi.ttf",
            "roboto-bolditalic.ttf",
            "dejavusans-boldoblique.ttf",
        ],
    },
    FontTarget {
        family_name: "Monospace Bold",
        font_id: "monospace_bold",
        file_candidates: &[
            "ubuntumono-bold.ttf",
            "dejavusansmono-bold.ttf",
            "liberationmono-bold.ttf",
            "hack-bold.ttf",
            "robotomono-bold.ttf",
        ],
    },
    FontTarget {
        family_name: "Monospace Italic",
        font_id: "monospace_italic",
        file_candidates: &[
            "ubuntumono-italic.ttf",
            "dejavusansmono-oblique.ttf",
            "liberationmono-italic.ttf",
            "hack-italic.ttf",
            "robotomono-italic.ttf",
        ],
    },
    FontTarget {
        family_name: "Monospace Bold Italic",
        font_id: "monospace_bold_italic",
        file_candidates: &[
            "ubuntumono-bolditalic.ttf",
            "dejavusansmono-boldoblique.ttf",
            "liberationmono-bolditalic.ttf",
            "hack-bolditalic.ttf",
            "robotomono-bolditalic.ttf",
        ],
    },
];

fn scan_font_dir(dir: &Path, map: &mut HashMap<String, PathBuf>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_font_dir(&path, map, depth + 1);
        } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let lower = file_name.to_lowercase();
            if lower.ends_with(".ttf") || lower.ends_with(".otf") {
                map.entry(lower).or_insert(path);
            }
        }
    }
}

fn collect_system_font_paths() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let mut search_dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(windir) = std::env::var("WINDIR").or_else(|_| std::env::var("SYSTEMROOT")) {
            search_dirs.push(PathBuf::from(windir).join("Fonts"));
        } else {
            search_dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        search_dirs.push(PathBuf::from("/System/Library/Fonts"));
        search_dirs.push(PathBuf::from("/Library/Fonts"));
        if let Ok(home) = std::env::var("HOME") {
            search_dirs.push(PathBuf::from(home).join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            search_dirs.push(PathBuf::from(&home).join(".fonts"));
            search_dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        }
        search_dirs.push(PathBuf::from("/usr/local/share/fonts"));
        search_dirs.push(PathBuf::from("/usr/share/fonts"));
    }

    #[cfg(target_os = "android")]
    {
        search_dirs.push(PathBuf::from("/system/fonts"));
        search_dirs.push(PathBuf::from("/product/fonts"));
    }

    for dir in search_dirs {
        scan_font_dir(&dir, &mut map, 0);
    }
    map
}

fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mono_fallback = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let font_map = collect_system_font_paths();

    // Find and load generic system bold and italic fonts for reliable fallbacks
    let mut system_bold_id: Option<String> = None;
    for candidate in [
        "ubuntu-bold.ttf",
        "ubuntub.ttf",
        "ubuntu-b.ttf",
        "roboto-bold.ttf",
        "dejavusans-bold.ttf",
        "liberationsans-bold.ttf",
        "arialbd.ttf",
        "segoeuib.ttf",
        "freesansbold.ttf",
    ] {
        if let Some(path) = font_map.get(candidate) {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    "system_bold_font".to_string(),
                    egui::FontData::from_owned(bytes).into(),
                );
                system_bold_id = Some("system_bold_font".to_string());
                break;
            }
        }
    }

    let mut system_italic_id: Option<String> = None;
    for candidate in [
        "ubuntu-italic.ttf",
        "ubuntui.ttf",
        "ubuntu-ri.ttf",
        "roboto-italic.ttf",
        "dejavusans-oblique.ttf",
        "liberationsans-italic.ttf",
        "ariali.ttf",
        "segoeuii.ttf",
    ] {
        if let Some(path) = font_map.get(candidate) {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    "system_italic_font".to_string(),
                    egui::FontData::from_owned(bytes).into(),
                );
                system_italic_id = Some("system_italic_font".to_string());
                break;
            }
        }
    }

    // Universal fallbacks for emojis, enclosed/math symbols, ancient/runic scripts, and CJK
    let mut universal_fallbacks: Vec<String> = Vec::new();

    // 0. Guaranteed embedded fallback fonts:
    fonts.font_data.insert(
        "embedded_emojis".to_string(),
        egui::FontData::from_static(EMBEDDED_EMOJIS).into(),
    );
    fonts.font_data.insert(
        "embedded_symbols_math".to_string(),
        egui::FontData::from_static(EMBEDDED_SYMBOLS_MATH).into(),
    );
    fonts.font_data.insert(
        "embedded_symbols_historic".to_string(),
        egui::FontData::from_static(EMBEDDED_SYMBOLS_HISTORIC).into(),
    );
    fonts.font_data.insert(
        "embedded_symbols_syllabics".to_string(),
        egui::FontData::from_static(EMBEDDED_SYMBOLS_SYLLABICS).into(),
    );
    fonts.font_data.insert(
        "embedded_symbols_enclosed".to_string(),
        egui::FontData::from_static(EMBEDDED_SYMBOLS_ENCLOSED).into(),
    );
    fonts.font_data.insert(
        "embedded_symbols_gamer".to_string(),
        egui::FontData::from_static(EMBEDDED_SYMBOLS_GAMER).into(),
    );
    fonts.font_data.insert("embedded_symbols_general".to_string(), egui::FontData::from_static(EMBEDDED_SYMBOLS_GENERAL).into());

    for id in [
        "embedded_symbols_general",
        "embedded_emojis",
        "embedded_symbols_math",
        "embedded_symbols_historic",
        "embedded_symbols_syllabics",
        "embedded_symbols_enclosed",
        "embedded_symbols_gamer",
    ] {
        universal_fallbacks.push(id.to_string());
    }

    // 1. System Emojis (Segoe UI Emoji, Noto Color Emoji, Apple Color Emoji, Twemoji)
    for (candidate, id) in [
        ("seguiemj.ttf", "system_segoe_emoji"),
        ("notocoloremoji.ttf", "system_noto_color_emoji"),
        ("noto-color-emoji.ttf", "system_noto_color_emoji2"),
        ("apple color emoji.ttf", "system_apple_emoji"),
        ("apple color emoji.ttc", "system_apple_emoji_ttc"),
        ("twemoji.ttf", "system_twemoji"),
    ] {
        if let Some(path) = font_map.get(candidate) {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    id.to_string(),
                    egui::FontData::from_owned(bytes).into(),
                );
                if !universal_fallbacks.contains(&id.to_string()) {
                    universal_fallbacks.push(id.to_string());
                }
                break;
            }
        }
    }

    // 2. Comprehensive Unicode glyph coverage: Math alphanumeric (bold, italic, script),
    // Fraktur, Runes, Enclosed alphanumerics, Small Caps, Turned letters, Phonetics, and CJK.
    // Load all present candidates so no script or aesthetic Unicode style is left as tofu.
    for (candidate, id) in [
        ("freeserif.otf", "system_freeserif_otf"),
        ("freeserif.ttf", "system_freeserif_ttf"),
        ("seguisym.ttf", "system_segoe_symbol"),
        ("seguihis.ttf", "system_segoe_historic"),
        ("freesans.otf", "system_freesans_otf"),
        ("freesans.ttf", "system_freesans_ttf"),
        ("freemono.otf", "system_freemono_otf"),
        ("freemono.ttf", "system_freemono_ttf"),
        ("symbola.ttf", "system_symbola"),
        ("dejavumathtexgyre.ttf", "system_dejavu_math"),
        ("dejavuserif-bolditalic.ttf", "system_dejavu_serif_bi"),
        ("dejavuserif.ttf", "system_dejavu_serif"),
        ("dejavusans.ttf", "system_dejavu_sans"),
        ("unifont.ttf", "system_unifont"),
        ("adwaitamono-regular.ttf", "system_adwaita_mono"),
        ("malgun.ttf", "system_malgun"),
        ("malgunbd.ttf", "system_malgun_bd"),
        ("notosanscjk-regular.ttc", "system_notosans_cjk"),
        ("droidsansfallback.ttf", "system_droid_cjk"),
        ("wqy-microhei.ttc", "system_wqy_microhei"),
        ("meiryo.ttf", "system_meiryo"),
        ("msjhl.ttf", "system_msjhl"),
        ("yugothic.ttf", "system_yugothic"),
    ] {
        if let Some(path) = font_map.get(candidate) {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    id.to_string(),
                    egui::FontData::from_owned(bytes).into(),
                );
                if !universal_fallbacks.contains(&id.to_string()) {
                    universal_fallbacks.push(id.to_string());
                }
            }
        }
    }

    for target in TARGETS {
        let mut loaded = false;
        for &candidate in target.file_candidates {
            if let Some(path) = font_map.get(candidate) {
                if let Ok(bytes) = std::fs::read(path) {
                    fonts.font_data.insert(
                        target.font_id.to_string(),
                        egui::FontData::from_owned(bytes).into(),
                    );
                    let mut family_fonts = vec![target.font_id.to_string()];
                    if target.family_name.contains("Bold") && let Some(bold_id) = &system_bold_id {
                        if !family_fonts.contains(bold_id) {
                            family_fonts.push(bold_id.clone());
                        }
                    }
                    if target.family_name.starts_with("Monospace") {
                        family_fonts.extend(mono_fallback.iter().cloned());
                    } else {
                        family_fonts.extend(fallback.iter().cloned());
                    }
                    fonts.families.insert(
                        egui::FontFamily::Name(target.family_name.into()),
                        family_fonts,
                    );
                    loaded = true;
                    break;
                }
            }
        }
        if !loaded {
            let mut family_fonts = Vec::new();
            let is_bold = target.family_name.contains("Bold");
            let is_italic = target.family_name.contains("Italic");

            if is_bold && let Some(bold_id) = &system_bold_id {
                family_fonts.push(bold_id.clone());
            } else if is_italic && let Some(italic_id) = &system_italic_id {
                family_fonts.push(italic_id.clone());
            } else {
                let primary_regular = match target.family_name {
                    "Segoe UI Bold" | "Segoe UI Italic" | "Segoe UI Bold Italic" => {
                        Some("segoe_ui_regular")
                    }
                    "Comic Sans MS Bold" | "Comic Sans MS Italic" | "Comic Sans MS Bold Italic" => {
                        Some("comic_sans_regular")
                    }
                    "Tahoma Bold" | "Tahoma Italic" | "Tahoma Bold Italic" => Some("tahoma_regular"),
                    "Verdana Bold" | "Verdana Italic" | "Verdana Bold Italic" => {
                        Some("verdana_regular")
                    }
                    "Ubuntu Bold" | "Ubuntu Italic" | "Ubuntu Bold Italic" => Some("ubuntu_regular"),
                    _ => None,
                };
                if let Some(reg_id) = primary_regular {
                    if fonts.font_data.contains_key(reg_id) {
                        family_fonts.push(reg_id.to_string());
                    }
                }
            }
            if target.family_name.starts_with("Monospace") {
                family_fonts.extend(mono_fallback.iter().cloned());
            } else {
                family_fonts.extend(fallback.iter().cloned());
            }
            fonts.families.insert(
                egui::FontFamily::Name(target.family_name.into()),
                family_fonts,
            );
        }
    }

    // Ensure the global Proportional family used for general UI (labels, headers, profile, contact list)
    // uses the preferred interface font and has full access to emojis and Unicode fallbacks:
    let prop_family = fonts.families.entry(egui::FontFamily::Proportional).or_default();
    if fonts.font_data.contains_key("segoe_ui_regular") {
        let id = "segoe_ui_regular".to_string();
        if !prop_family.contains(&id) {
            prop_family.insert(0, id);
        }
    }
    for fb in &universal_fallbacks {
        if !prop_family.contains(fb) {
            prop_family.push(fb.clone());
        }
    }

    // Ensure Monospace also has Unicode and emoji fallbacks:
    let mono_family = fonts.families.entry(egui::FontFamily::Monospace).or_default();
    for fb in &universal_fallbacks {
        if !mono_family.contains(fb) {
            mono_family.push(fb.clone());
        }
    }

    // Ensure every named font family (e.g., "Segoe UI Bold", "Comic Sans MS") has Unicode & emoji fallbacks:
    for family in fonts.families.values_mut() {
        for fb in &universal_fallbacks {
            if !family.contains(fb) {
                family.push(fb.clone());
            }
        }
    }

    fonts
}

pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_general_symbols_cover_common_display_name_stars() {
        let mut definitions = egui::FontDefinitions::empty();
        definitions.font_data.insert("symbols".into(), egui::FontData::from_static(EMBEDDED_SYMBOLS_GENERAL).into());
        definitions.families.insert(egui::FontFamily::Proportional, vec!["symbols".into()]);
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| { ui.label("⋆ ★ ✦ ✧ ☆"); });
        output.textures_delta.clear();
        ctx.fonts_mut(|fonts| {
            for character in ['⋆', '★', '✦', '✧', '☆'] {
                assert!(fonts.has_glyph(&egui::FontId::proportional(14.0), character), "missing {character}");
            }
        });
    }

    #[test]
    fn named_interface_fonts_have_real_fallbacks() {
        let fonts = font_definitions();
        let fallback = &fonts.families[&egui::FontFamily::Proportional];
        assert!(!fallback.is_empty());
        for name in [
            "Segoe UI",
            "Segoe UI Bold",
            "Segoe UI Italic",
            "Segoe UI Bold Italic",
            "Comic Sans MS",
            "Comic Sans MS Bold",
            "Comic Sans MS Italic",
            "Comic Sans MS Bold Italic",
            "Tahoma",
            "Tahoma Bold",
            "Verdana",
            "Verdana Bold",
            "Ubuntu",
            "Ubuntu Bold",
        ] {
            let family = &fonts.families[&egui::FontFamily::Name(name.into())];
            assert!(!family.is_empty());
            assert!(family.iter().any(|f| fallback.contains(f)));
        }
    }

    #[test]
    fn unicode_and_emojis_render_without_panics() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);

        let test_samples = [
            "𝑴𝑬𝑵𝑺𝑨𝑮𝑬𝑴 𝑫𝑬 𝑻𝑬𝑺𝑻𝑬",
            "𐌌𐌄𐌍𐌔𐌀𐌂𐌄𐌌 𐌃𐌄 𐌕𐌄𐌔𐌕𐌄",
            "𝔐𝔈𝔑𝔖𝔄𝔊𝔈𝔐 𝔇𝔈 𝔗𝔈𝔖𝔗𝔈",
            "𝕸𝕰𝕹𝕾𝕬𝕲𝕰𝕸 𝕯𝕰 𝕿𝕰𝕾𝕿𝕰",
            "əʇsəʇ əp ɯəɓɐsuəW",
            "ᴹᵉⁿˢᵃᵍᵉᵐ ᵈᵉ ᵗᵉˢᵗᵉ",
            "ᴍᴇɴsᴀɢᴇᴍ ᴅᴇ ᴛᴇsᴛᴇ",
            "⒨⒠⒩⒮⒜⒢⒠⒨ ⒟⒠ ⒯⒠⒮⒯⒠",
            "ℳℰ𝒩𝒮𝒜𝒢ℰ𝑴 𝒟ℰ 𝒯ℰ𝒮𝒯ℰ",
            "ᛗᛖᚾᛊᚨᚷᛖᛗ ᛞᛖ ᛏᛖᛊᛏᛖ",
            "MᕮᘉSᗩGᕮM Dᕮ TᕮSTᕮ",
            "MΣПƧΛGΣM DΣ TΣƧTΣ",
            "MᕮᏁᏕᎯᎶᕮM Dᕮ ᎿᕮᏕᎿᕮ",
            "𖤍•°᭄ Wellington ᭄•°𖤍",
            "꧁•°Wellington°•᭄࿐",
            "😀 🔥 ❤️ ✨ 🎉 🚀 👍",
        ];

        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for sample in test_samples {
                ui.label(egui::RichText::new(sample));
                ui.label(egui::RichText::new(sample).family(egui::FontFamily::Name("Segoe UI Bold".into())));
            }
        });
        output.textures_delta.clear();

        ctx.fonts_mut(|fonts| {
            for sample in test_samples {
                let galley = fonts.layout_no_wrap(
                    sample.to_string(),
                    egui::FontId::new(14.0, egui::FontFamily::Name("Segoe UI Bold".into())),
                    egui::Color32::WHITE,
                );
                for row in &galley.rows {
                    for g in &row.glyphs {
                        assert_ne!(
                            g.chr, '\u{FFFD}',
                            "Character '{}' (U+{:04X}) was replaced with U+FFFD in sample '{}'",
                            g.chr, g.chr as u32, sample
                        );
                    }
                }
            }
        });
    }

    #[test]
    fn embedded_fonts_render_without_any_system_fonts() {
        let mut fonts = egui::FontDefinitions::empty();
        fonts.font_data.insert(
            "embedded_emojis".to_string(),
            egui::FontData::from_static(EMBEDDED_EMOJIS).into(),
        );
        fonts.font_data.insert(
            "embedded_symbols_math".to_string(),
            egui::FontData::from_static(EMBEDDED_SYMBOLS_MATH).into(),
        );
        fonts.font_data.insert(
            "embedded_symbols_historic".to_string(),
            egui::FontData::from_static(EMBEDDED_SYMBOLS_HISTORIC).into(),
        );
        fonts.font_data.insert(
            "embedded_symbols_syllabics".to_string(),
            egui::FontData::from_static(EMBEDDED_SYMBOLS_SYLLABICS).into(),
        );
        fonts.font_data.insert(
            "embedded_symbols_enclosed".to_string(),
            egui::FontData::from_static(EMBEDDED_SYMBOLS_ENCLOSED).into(),
        );
        fonts.font_data.insert(
            "embedded_symbols_gamer".to_string(),
            egui::FontData::from_static(EMBEDDED_SYMBOLS_GAMER).into(),
        );
        fonts.families.insert(
            egui::FontFamily::Proportional,
            vec![
                "embedded_emojis".to_string(),
                "embedded_symbols_math".to_string(),
                "embedded_symbols_historic".to_string(),
                "embedded_symbols_syllabics".to_string(),
                "embedded_symbols_enclosed".to_string(),
                "embedded_symbols_gamer".to_string(),
            ],
        );
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);

        let test_samples = [
            "𝑴𝑬𝑵𝑺𝑨𝑮𝑬𝑴 𝑫𝑬 𝑻𝑬𝑺𝑻𝑬",
            "𐌌𐌄𐌍𐌔𐌀𐌂𐌄𐌌 𐌃𐌄 𐌕𐌄𐌔𐌕𐌄",
            "𝔐𝔈𝔑𝔖𝔄𝔊𝔈𝔐 𝔇𝔈 𝔗𝔈𝔖𝔗𝔈",
            "𝕸𝕰𝕹𝕾𝕬𝕲𝕰𝕸 𝕯𝕰 𝕿𝕰𝕾𝕿𝕰",
            "əʇsəʇ əp ɯəɓɐsuəW",
            "ᴹᵉⁿˢᵃᵍᵉᵐ ᵈᵉ ᵗᵉˢᵗᵉ",
            "ᴍᴇɴsᴀɢᴇᴍ ᴅᴇ ᴛᴇsᴛᴇ",
            "⒨⒠⒩⒮⒜⒢⒠⒨ ⒟⒠ ⒯⒠⒮⒯⒠",
            "ℳℰ𝒩𝒮𝒜𝒢ℰ𝑴 𝒟ℰ 𝒯ℰ𝒮𝒯ℰ",
            "ᛗᛖᚾᛊᚨᚷᛖᛗ ᛞᛖ ᛏᛖᛊᛏᛖ",
            "MᕮᘉSᗩGᕮM Dᕮ TᕮSTᕮ",
            "MΣПƧΛGΣM DΣ TΣƧTΣ",
            "MᕮᏁᏕᎯᎶᕮM Dᕮ ᎿᕮᏕᎿᕮ",
            "𖤍•°᭄ Wellington ᭄•°𖤍",
            "꧁•°Wellington°•᭄࿐",
            "😀 🔥 ❤️ ✨ 🎉 🚀 👍",
        ];

        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            for sample in test_samples {
                ui.label(egui::RichText::new(sample));
            }
        });
        output.textures_delta.clear();

        ctx.fonts_mut(|fonts| {
            for sample in test_samples {
                let galley = fonts.layout_no_wrap(
                    sample.to_string(),
                    egui::FontId::new(14.0, egui::FontFamily::Proportional),
                    egui::Color32::WHITE,
                );
                for row in &galley.rows {
                    for g in &row.glyphs {
                        assert_ne!(
                            g.chr, '\u{FFFD}',
                            "Character '{}' (U+{:04X}) was replaced with U+FFFD in sample '{}'",
                            g.chr, g.chr as u32, sample
                        );
                    }
                }
            }
        });
    }
}
