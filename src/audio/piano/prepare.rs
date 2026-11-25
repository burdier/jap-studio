//! Preparación costosa en worker; el stream se abre solo cuando se solicita audio.
use super::{PianoPlayer, PianoSettings, PresetEntry, SynthBackend, soundfont};
use crate::audio::output::choose_output_config;
use elevate::devices::utility_slot::UtilitySlot;
use std::path::Path;

pub(crate) struct PreparedPiano {
    device: cpal::Device,
    setup: crate::audio::output::StreamSetup,
    backend: SynthBackend,
}

impl PreparedPiano {
    pub fn vst_editor(&self) -> Option<crate::audio::vst3::EditorHandle> {
        if let SynthBackend::Vst(voice) = &self.backend {
            Some(voice.shared.editor.clone())
        } else {
            None
        }
    }
    pub fn vst_shared(&self) -> Option<std::sync::Arc<crate::audio::vst3::SharedInstrument>> {
        if let SynthBackend::Vst(voice) = &self.backend {
            Some(voice.shared.clone())
        } else {
            None
        }
    }
    pub fn load_vst(
        path: &Path,
        audio: &crate::audio::device::AudioConfig,
        class: Option<String>,
    ) -> Result<(Self, String), String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let engine = crate::audio::vst3::VstEngine::load_instrument_class(
            path,
            setup.config.sample_rate,
            class,
        )?;
        let name = engine.name.clone();
        let _ = engine.editor.watch_state(None);
        let shared = crate::audio::vst3::SharedInstrument::new(engine, setup.config.sample_rate);
        Ok((
            Self {
                device,
                setup,
                backend: SynthBackend::Vst(crate::audio::vst3::InstrumentVoice::preview(shared)),
            },
            name,
        ))
    }
    pub fn load_vst_preset(
        path: std::path::PathBuf,
        class: String,
        preset: elevate::musical::fx::VstPreset,
        audio: &crate::audio::device::AudioConfig,
    ) -> Result<(Self, String), String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let state = crate::audio::vst3::PluginState::from_instrument_preset(path, class, &preset)?;
        let engine = state.instantiate(setup.config.sample_rate)?;
        let name = engine.name.clone();
        let _ = engine.editor.watch_state(Some(preset));
        let shared = crate::audio::vst3::SharedInstrument::new(engine, setup.config.sample_rate);
        Ok((
            Self {
                device,
                setup,
                backend: SynthBackend::Vst(crate::audio::vst3::InstrumentVoice::preview(shared)),
            },
            name,
        ))
    }
    pub fn load(
        path: &Path,
        settings: PianoSettings,
        audio: &crate::audio::device::AudioConfig,
    ) -> Result<(Self, Vec<PresetEntry>), String> {
        let font = soundfont::load_path(path)?;
        let presets = soundfont::presets_of(&font);
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let backend = SynthBackend::new(path, &font, setup.config.sample_rate, settings)?;
        Ok((
            Self {
                device,
                setup,
                backend,
            },
            presets,
        ))
    }

    pub fn start(
        self,
        gain: f32,
        utility: UtilitySlot,
        pan: f32,
        fx_gain: f32,
    ) -> Result<PianoPlayer, String> {
        PianoPlayer::start_backend(
            self.device,
            self.setup,
            self.backend,
            gain,
            utility,
            pan,
            fx_gain,
        )
    }
}
