//! User-interface translations and system language selection.
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, atomic::{AtomicU8, Ordering}};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    Portuguese,
    #[default]
    System,
    English,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    English,
    Portuguese,
}

impl Language {
    fn from_locale(locale: &str) -> Self {
        let language = locale.split(['_', '-', '.', '@']).next().unwrap_or("");
        if language.eq_ignore_ascii_case("pt") { Self::Portuguese } else { Self::English }
    }
}

static SYSTEM_LANGUAGE: LazyLock<Language> = LazyLock::new(|| {
    #[cfg(target_os = "android")]
    let locale = crate::android::system_locale().or_else(sys_locale::get_locale);
    #[cfg(not(target_os = "android"))]
    let locale = sys_locale::get_locale();
    Language::from_locale(locale.as_deref().unwrap_or("en"))
});
static PREFERENCE: AtomicU8 = AtomicU8::new(0);

pub fn set_preference(preference: LanguagePreference) {
    PREFERENCE.store(match preference {
        LanguagePreference::System => 0,
        LanguagePreference::English => 1,
        LanguagePreference::Portuguese => 2,
    }, Ordering::Relaxed);
}

fn resolve(preference: LanguagePreference, system: Language) -> Language {
    match preference {
        LanguagePreference::System => system,
        LanguagePreference::English => Language::English,
        LanguagePreference::Portuguese => Language::Portuguese,
    }
}

/// Return the translation for the selected language, falling back to English.
pub fn tr<'a>(english: &'a str, portuguese: &'a str) -> &'a str {
    let preference = match PREFERENCE.load(Ordering::Relaxed) {
        1 => LanguagePreference::English,
        2 => LanguagePreference::Portuguese,
        _ => LanguagePreference::System,
    };
    match resolve(preference, *SYSTEM_LANGUAGE) {
        Language::English => english,
        Language::Portuguese => portuguese,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_system_language_and_explicit_overrides() {
        assert_eq!(Language::from_locale("pt_BR.UTF-8"), Language::Portuguese);
        assert_eq!(Language::from_locale("fr-FR"), Language::English);
        assert_eq!(resolve(LanguagePreference::System, Language::Portuguese), Language::Portuguese);
        assert_eq!(resolve(LanguagePreference::English, Language::Portuguese), Language::English);
        assert_eq!(resolve(LanguagePreference::Portuguese, Language::English), Language::Portuguese);
    }
}
