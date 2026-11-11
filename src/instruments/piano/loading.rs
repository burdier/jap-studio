//! Carga diferida y serializada. Los resultados antiguos nunca cambian el banco elegido.
use super::Piano;
use crate::{
    audio::piano::{PianoSettings, PreparedPiano, PresetEntry, SoundFontBank, discover_soundfonts},
    instruments::piano_params::ParamId,
};
use elevate::{devices::utility_slot::UtilitySlot, musical::clip::MidiClip};
use std::sync::mpsc::{self, Receiver, TryRecvError};

type Loaded = (
    Vec<SoundFontBank>,
    usize,
    Result<(PreparedPiano, Vec<PresetEntry>), String>,
);

#[derive(Default)]
pub(super) struct Loading {
    generation: u64,
    job: Option<(u64, Receiver<Result<Loaded, String>>)>,
    pub prepared: Option<PreparedPiano>,
    pub pending: Option<PendingPlayback>,
    pub error: Option<String>,
    rescan: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::musical::{model::Pattern, time::Tick};

    #[test]
    fn release_and_stop_cancel_audio_requested_before_resources_are_ready() {
        let mut piano = Piano::new();
        piano.held.insert(60);
        piano.loading.pending = Some(PendingPlayback::Notes {
            notes: [(60, 100)].into(),
            gain: 1.,
            utility: UtilitySlot::new(),
        });
        assert!(piano.note_off(60));
        assert!(piano.loading.pending.is_none());
        let clip = MidiClip::new("Pendiente", 0, 0., Pattern::new(Tick(960)).unwrap());
        piano.loading.pending = Some(PendingPlayback::Clip {
            clip,
            beat: 0.,
            bpm: 120.,
            gain: 1.,
            utility: UtilitySlot::new(),
        });
        assert!(piano.clip_playing());
        piano.stop_clip();
        assert!(!piano.clip_playing());
        assert!(piano.loading.pending.is_none());
    }

    #[test]
    fn failed_load_keeps_bank_choices_and_clears_pending_notes() {
        let mut piano = Piano::new();
        piano.held.insert(60);
        piano.mouse = Some(60);
        let bank = SoundFontBank {
            path: "missing.sf2".into(),
            name: "Ausente".into(),
            credits: String::new(),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        piano.loading.job = Some((0, rx));
        assert!(
            tx.send(Ok((vec![bank], 0, Err("No se pudo cargar".into()))))
                .is_ok()
        );
        assert!(piano.poll_resources().unwrap().is_err());
        assert_eq!(piano.banks().len(), 1);
        assert!(!piano.held(60));
        assert_eq!(piano.take_mouse_note(), None);
        piano.request_resources();
        assert!(
            !piano.loading(),
            "un error no debe iniciar reintentos por frame"
        );
    }
}

pub(super) enum PendingPlayback {
    Notes {
        notes: std::collections::HashMap<u8, u8>,
        gain: f32,
        utility: UtilitySlot,
    },
    Clip {
        clip: MidiClip,
        beat: f64,
        bpm: f64,
        gain: f32,
        utility: UtilitySlot,
    },
}

impl Piano {
    pub fn configure_audio(&mut self, config: crate::audio::device::AudioConfig) {
        self.stop();
        self.invalidate_resources();
        self.output_config = config;
    }

    pub fn loading(&self) -> bool {
        self.loading.job.is_some()
    }

    pub fn resource_status(&self) -> &str {
        if self.loading() {
            "Preparando SoundFont…"
        } else if let Some(error) = &self.loading.error {
            error
        } else if self.player.is_some() || self.loading.prepared.is_some() {
            "Listo · SoundFont"
        } else {
            "Carga bajo demanda"
        }
    }

    pub(super) fn invalidate_resources(&mut self) {
        self.loading.generation += 1;
        self.loading.prepared = None;
        self.loading.error = None;
    }

    pub fn request_resources(&mut self) {
        if self.player.is_some()
            || self.loading.prepared.is_some()
            || self.loading.job.is_some()
            || self.loading.error.is_some()
        {
            return;
        }
        let mut banks = self.banks.clone();
        let path = banks.get(self.bank_index).map(|bank| bank.path.clone());
        if std::mem::take(&mut self.loading.rescan) {
            banks.clear();
        }
        let settings = PianoSettings {
            maximum_polyphony: self.params.get(ParamId::Polyphony) as usize,
            reverb_and_chorus: self.params.get(ParamId::Effects) >= 0.5,
        };
        let audio = self.output_config.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        self.loading.job = Some((self.loading.generation, rx));
        std::thread::spawn(move || {
            let result = (|| {
                let banks = if banks.is_empty() {
                    discover_soundfonts()
                } else {
                    banks
                };
                let index = path
                    .as_ref()
                    .and_then(|path| banks.iter().position(|bank| &bank.path == path))
                    .unwrap_or(0);
                let prepared = banks
                    .get(index)
                    .ok_or_else(|| "No hay bancos SoundFont disponibles".to_string())
                    .and_then(|bank| PreparedPiano::load(&bank.path, settings, &audio));
                Ok((banks, index, prepared))
            })();
            let _ = tx.send(result);
        });
    }

    pub fn poll_resources(&mut self) -> Option<Result<(), String>> {
        let (generation, receiver) = self.loading.job.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                Err("La preparación del piano se interrumpió".into())
            }
        };
        let current = *generation == self.loading.generation;
        self.loading.job = None;
        if !current {
            self.request_resources();
            return None;
        }
        let result = self.finish_loading(result);
        if let Err(error) = &result {
            self.loading.error = Some(error.clone());
            self.loading.pending = None;
            self.held.clear();
            self.mouse = None;
        }
        Some(result)
    }

    fn finish_loading(&mut self, result: Result<Loaded, String>) -> Result<(), String> {
        let (banks, index, prepared) = result?;
        if self.banks.get(self.bank_index).map(|b| &b.path) != banks.get(index).map(|b| &b.path) {
            self.preset_index = 0;
        }
        self.banks = banks;
        self.bank_index = index;
        let (prepared, presets) = prepared?;
        self.loading.prepared = Some(prepared);
        self.presets = presets;
        self.preset_index = self.preset_index.min(self.presets.len().saturating_sub(1));
        match self.loading.pending.take() {
            Some(PendingPlayback::Notes {
                notes,
                gain,
                utility,
            }) if !notes.is_empty() => {
                self.ensure_started(gain, utility)?;
                if let Some(player) = &self.player {
                    for (note, velocity) in notes {
                        if self.held.contains(&note) {
                            player.note_on(note, velocity);
                        }
                    }
                }
            }
            Some(PendingPlayback::Clip {
                clip,
                beat,
                bpm,
                gain,
                utility,
            }) => {
                self.play_clip(&clip, beat, bpm, gain, utility)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn refresh_banks(&mut self) {
        self.stop();
        self.invalidate_resources();
        self.loading.rescan = true;
        self.request_resources();
    }
}
