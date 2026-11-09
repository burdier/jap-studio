//! Preferencias editables: el borrador se valida y se guarda antes de aplicarse al exportador.
mod input;
use super::{DawApp, profile};
use elevate::export::ExportOptions;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExportField {
    Name,
    Directory,
    Start,
    Length,
    Tail,
}

impl ExportField {
    pub const ALL: [Self; 5] = [
        Self::Name,
        Self::Directory,
        Self::Start,
        Self::Length,
        Self::Tail,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Nombre base",
            Self::Directory => "Carpeta de destino",
            Self::Start => "Inicio (beats)",
            Self::Length => "Duración (beats; vacío = hasta el final)",
            Self::Tail => "Cola final (segundos)",
        }
    }
}

pub(super) struct ExportPreferences {
    pub saved: ExportOptions,
    pub draft: ExportOptions,
    fields: [String; 5],
    pub editing: Option<ExportField>,
    pub select_all: bool,
    pub message: Option<(bool, String)>,
}

impl Default for ExportPreferences {
    fn default() -> Self {
        Self::from_options(ExportOptions::default())
    }
}

impl ExportPreferences {
    fn from_options(options: ExportOptions) -> Self {
        Self {
            fields: [
                options.name.clone(),
                options.directory.to_string_lossy().into_owned(),
                options.start_beat.to_string(),
                options
                    .length_beats
                    .map_or(String::new(), |n| n.to_string()),
                options.tail_seconds.to_string(),
            ],
            saved: options.clone(),
            draft: options,
            editing: None,
            select_all: false,
            message: None,
        }
    }

    pub fn load() -> Self {
        let content = profile::load().unwrap_or_default();
        match Self::from_profile(&content) {
            Ok(settings) => settings,
            Err(error) => Self {
                message: Some((
                    false,
                    format!("No se pudieron cargar las preferencias: {error}"),
                )),
                ..Self::default()
            },
        }
    }

    fn from_profile(content: &str) -> Result<Self, String> {
        let command = content
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| key.trim() == "EXPORT_DEFAULTS")
            .map(|(_, value)| value.trim())
            .last();
        Ok(Self::from_options(match command {
            Some(command) => ExportOptions::parse(command)?,
            None => ExportOptions::default(),
        }))
    }

    pub fn text(&self, field: ExportField) -> &str {
        &self.fields[field as usize]
    }

    pub fn edit(&mut self, text: &str) {
        let Some(field) = self.editing else {
            return;
        };
        let value = &mut self.fields[field as usize];
        if self.select_all {
            value.clear();
            self.select_all = false;
        }
        if text == "backspace" {
            value.pop();
        } else if value.len() + text.len() <= 4096 {
            value.push_str(text);
        }
        self.message = None;
    }

    pub fn options(&self) -> Result<ExportOptions, String> {
        let mut options = self.draft.clone();
        options.name = self.text(ExportField::Name).trim().into();
        options.directory = self.text(ExportField::Directory).trim().into();
        let number = |field| {
            self.text(field)
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("Número inválido: {}", field.label()))
        };
        options.start_beat = number(ExportField::Start)?;
        options.length_beats = if self.text(ExportField::Length).trim().is_empty() {
            None
        } else {
            Some(number(ExportField::Length)?)
        };
        options.tail_seconds = number(ExportField::Tail)?;
        options.to_command()?;
        Ok(options)
    }

    pub fn restore_saved(&mut self) {
        *self = Self::from_options(self.saved.clone());
    }

    pub fn reset_draft(&mut self) {
        let saved = self.saved.clone();
        *self = Self::default();
        self.saved = saved;
    }
}

impl DawApp {
    pub(super) fn open_export_settings(&mut self) {
        self.open_settings();
        self.settings.category = super::settings::SettingsCategory::Export;
    }

    pub(super) fn save_export_preferences(&mut self) {
        let result = self.settings.export.options().and_then(|options| {
            profile::save_values(&[("EXPORT_DEFAULTS", options.to_command()?)])
                .map_err(|error| error.to_string())?;
            Ok(options)
        });
        match result {
            Ok(options) => {
                self.settings.export.saved = options;
                self.settings.export.editing = None;
                self.settings.export.message = Some((
                    true,
                    "Guardado. «Exportar audio» usará estos valores.".into(),
                ));
            }
            Err(error) => self.settings.export.message = Some((false, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_preferences_survive_reload_and_invalid_drafts_do_not_replace_saved_values() {
        let mut preferences = ExportPreferences::default();
        preferences.editing = Some(ExportField::Name);
        preferences.select_all = true;
        preferences.edit("Mi canción");
        preferences.draft.sample_rate = 44_100;
        let options = preferences.options().unwrap();
        let profile = format!(
            "BG=112233\nEXPORT_DEFAULTS={}\n",
            options.to_command().unwrap()
        );
        let loaded = ExportPreferences::from_profile(&profile).unwrap();
        assert_eq!(loaded.saved.name, "Mi canción");
        assert_eq!(loaded.saved.sample_rate, 44_100);
        assert_eq!(preferences.saved.sample_rate, 48_000);
        preferences.editing = Some(ExportField::Start);
        preferences.select_all = true;
        preferences.edit("NaN");
        assert!(preferences.options().is_err());
        assert_eq!(preferences.saved.start_beat, 0.0);
    }
}
