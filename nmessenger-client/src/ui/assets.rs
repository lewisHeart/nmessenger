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
        search_dirs.push(PathBuf::from("/usr/share/fonts"));
        search_dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Ok(home) = std::env::var("HOME") {
            search_dirs.push(PathBuf::from(&home).join(".fonts"));
            search_dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        }
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

    fonts
}

pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
