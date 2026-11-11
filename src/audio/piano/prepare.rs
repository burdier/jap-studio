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
        if let SynthBackend::Vst(engine) = &self.backend { Some(engine.editor.clone()) } else { None }
    }
    pub fn load_vst(path: &Path, audio: &crate::audio::device::AudioConfig, class: Option<String>) -> Result<(Self, String), String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        let engine = crate::audio::vst3::VstEngine::load_instrument_class(path, setup.config.sample_rate, class)?;
        let name = engine.name.clone();
        Ok((Self { device, setup, backend: SynthBackend::Vst(engine) }, name))
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
