//! Captura y restauración sin abrir un dispositivo de audio.
use super::*;
use elevate::project::ProjectPiano;

impl Piano {
    pub fn project_state(&self) -> ProjectPiano {
        ProjectPiano {
            id: self.id,
            source: self
                .banks
                .get(self.bank_index)
                .map(|b| b.path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            preset: self.preset_index as u32,
            parameters: piano_params::SPECS
                .iter()
                .map(|s| self.params.get(s.id))
                .collect(),
        }
    }

    pub fn restore_project(state: &ProjectPiano) -> Result<Self, String> {
        if state.parameters.len() != piano_params::SPECS.len() {
            return Err("Versión de parámetros de piano no compatible".into());
        }
        let mut piano = Self::new_with_id(state.id);
        for (spec, value) in piano_params::SPECS.iter().zip(&state.parameters) {
            if !value.is_finite() || *value < spec.min || *value > spec.max {
                return Err(format!("Parámetro de piano inválido: {}", spec.label));
            }
            piano.params.set(spec.id, *value);
        }
        if !state.source.is_empty() {
            let path = crate::audio::piano::resolve_soundfont_path(&state.source)
                .ok_or_else(|| format!("Falta SoundFont: {}", state.source))?;
            let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let mut banks = discover_soundfonts();
            if let Some(idx) = banks.iter().position(|b| {
                std::fs::canonicalize(&b.path).unwrap_or_else(|_| b.path.clone()) == canonical
            }) {
                piano.banks = banks;
                piano.bank_index = idx;
            } else {
                banks.insert(
                    0,
                    SoundFontBank {
                        name: path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        path,
                        credits: String::new(),
                    },
                );
                piano.banks = banks;
                piano.bank_index = 0;
            }
        }
        piano.preset_index = state.preset as usize;
        InstrumentId::reserve_restored(state.id)?;
        Ok(piano)
    }
}
