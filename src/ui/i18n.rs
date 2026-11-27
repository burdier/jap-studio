//! Internacionalización estándar basada en archivos JSON (`assets/locales/{es,en,pt,fr,ja}.json`).
//! Sin dependencias adicionales: reutiliza `serde_json` + `include_str!` + caché `OnceLock` sin
//! asignaciones en render, fallback automático (`lang → en → es → key`) y persistencia en `~/.jaybu.profile`.
use super::profile;
use std::{
    collections::BTreeMap,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

static ACTIVE_LANGUAGE: AtomicU8 = AtomicU8::new(0);

pub(in crate::ui) fn set_active_language(lang: Language) {
    ACTIVE_LANGUAGE.store(lang.index() as u8, Ordering::Relaxed);
}

pub(in crate::ui) fn active_language() -> Language {
    match ACTIVE_LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::En,
        2 => Language::Pt,
        3 => Language::Fr,
        4 => Language::Ja,
        _ => Language::Es,
    }
}

pub(super) const PROFILE_KEY: &str = "LANGUAGE";

const LOCALE_ES: &str = include_str!("../../assets/locales/es.json");
const LOCALE_EN: &str = include_str!("../../assets/locales/en.json");
const LOCALE_PT: &str = include_str!("../../assets/locales/pt.json");
const LOCALE_FR: &str = include_str!("../../assets/locales/fr.json");
const LOCALE_JA: &str = include_str!("../../assets/locales/ja.json");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ui) enum Language {
    #[default]
    Es,
    En,
    Pt,
    Fr,
    Ja,
}

impl Language {
    pub const ALL: [Self; 5] = [Self::Es, Self::En, Self::Pt, Self::Fr, Self::Ja];

    fn index(self) -> usize {
        match self {
            Self::Es => 0,
            Self::En => 1,
            Self::Pt => 2,
            Self::Fr => 3,
            Self::Ja => 4,
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
            Self::Pt => "pt",
            Self::Fr => "fr",
            Self::Ja => "ja",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Es => "Español",
            Self::En => "English",
            Self::Pt => "Português",
            Self::Fr => "Français",
            Self::Ja => "日本語",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "es" | "es-es" | "es-la" | "es-419" | "spanish" | "espanol" | "español" => {
                Some(Self::Es)
            }
            "en" | "en-us" | "en-gb" | "english" | "ingles" | "inglés" => Some(Self::En),
            "pt" | "pt-br" | "pt-pt" | "portuguese" | "portugues" | "português" => Some(Self::Pt),
            "fr" | "fr-fr" | "french" | "frances" | "francés" | "français" => Some(Self::Fr),
            "ja" | "jp" | "ja-jp" | "japanese" | "japones" | "japonés" | "日本語" => {
                Some(Self::Ja)
            }
            _ => None,
        }
    }

    pub fn parse_profile(content: &str) -> Option<Self> {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("LANGUAGE=")
                && let Some(lang) = Self::parse(rest)
            {
                return Some(lang);
            }
        }
        None
    }

    pub fn load_saved() -> Option<Self> {
        let content = profile::load().ok()?;
        Self::parse_profile(&content)
    }

    pub fn load() -> Self {
        Self::load_saved().unwrap_or_default()
    }

    pub fn save(self) -> std::io::Result<()> {
        profile::save_values(&[(PROFILE_KEY, self.token().to_string())])
    }
}

fn parse_catalog(raw: &str, name: &str) -> BTreeMap<String, String> {
    serde_json::from_str(raw).unwrap_or_else(|e| panic!("JSON inválido en locale {name}: {e}"))
}

fn catalogs() -> &'static [BTreeMap<String, String>; 5] {
    static CATALOGS: OnceLock<[BTreeMap<String, String>; 5]> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        [
            parse_catalog(LOCALE_ES, "es.json"),
            parse_catalog(LOCALE_EN, "en.json"),
            parse_catalog(LOCALE_PT, "pt.json"),
            parse_catalog(LOCALE_FR, "fr.json"),
            parse_catalog(LOCALE_JA, "ja.json"),
        ]
    })
}

/// Traduce una clave estándar (`"menu.file"`, `"drums.empty.title"`, etc.) sin reservar memoria.
/// Fallback automático: idioma activo → inglés → español → clave literal.
pub(in crate::ui) fn tr(lang: Language, key: &'static str) -> &'static str {
    let tables = catalogs();
    tables[lang.index()]
        .get(key)
        .or_else(|| tables[Language::En.index()].get(key))
        .or_else(|| tables[Language::Es.index()].get(key))
        .map(String::as_str)
        .unwrap_or(key)
}

/// Traduce una clave con interpolación estándar `{nombre}` (ej. `tr_fmt(lang, "mpc.bank_tooltip", &[("bank", "A")])`).
pub(in crate::ui) fn tr_fmt(lang: Language, key: &'static str, args: &[(&str, &str)]) -> String {
    let mut out = tr(lang, key).to_owned();
    for (name, value) in args {
        let placeholder = format!("{{{name}}}");
        out = out.replace(&placeholder, value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn parses_language_aliases_and_profile() {
        assert_eq!(Language::parse("es"), Some(Language::Es));
        assert_eq!(Language::parse("EN-US"), Some(Language::En));
        assert_eq!(Language::parse("pt-BR"), Some(Language::Pt));
        assert_eq!(Language::parse("fr"), Some(Language::Fr));
        assert_eq!(Language::parse("ja"), Some(Language::Ja));
        assert_eq!(Language::parse("日本語"), Some(Language::Ja));
        assert_eq!(Language::parse("klingon"), None);

        let profile = "THEME=dark\nLANGUAGE=ja\nSHORTCUT_PRESET=ableton\n";
        assert_eq!(Language::parse_profile(profile), Some(Language::Ja));
    }

    #[test]
    fn all_locale_json_files_have_100_percent_key_parity_and_non_empty_values() {
        let tables = catalogs();
        let reference_keys: BTreeSet<&str> = tables[Language::Es.index()]
            .keys()
            .map(String::as_str)
            .collect();
        assert!(
            reference_keys.len() >= 150,
            "se esperaban al menos 150 claves en es.json, hay {}",
            reference_keys.len()
        );
        for lang in Language::ALL {
            let table = &tables[lang.index()];
            let keys: BTreeSet<&str> = table.keys().map(String::as_str).collect();
            let missing: Vec<&str> = reference_keys.difference(&keys).copied().collect();
            let extra: Vec<&str> = keys.difference(&reference_keys).copied().collect();
            assert!(
                missing.is_empty(),
                "locale {}.json le faltan claves: {:?}",
                lang.token(),
                missing
            );
            assert!(
                extra.is_empty(),
                "locale {}.json tiene claves extra: {:?}",
                lang.token(),
                extra
            );
            for (k, v) in table {
                assert!(
                    !v.trim().is_empty(),
                    "locale {}.json tiene valor vacío en clave {k}",
                    lang.token()
                );
            }
        }
    }

    #[test]
    fn tr_and_tr_fmt_interpolate_and_fallback_cleanly() {
        assert_eq!(tr(Language::En, "menu.file"), "File");
        assert_eq!(tr(Language::Ja, "menu.file"), "ファイル");
        assert_eq!(tr(Language::Es, "nonexistent.key"), "nonexistent.key");
        assert_eq!(
            tr_fmt(
                Language::En,
                "mpc.bank_tooltip",
                &[("bank", "B"), ("start", "17"), ("end", "32")]
            ),
            "Bank B (pads 17–32)"
        );
    }
}
