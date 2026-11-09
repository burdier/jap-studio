//! Motor de síntesis compartido por audición y render offline. Sin dispositivo de salida.
use super::{PianoSettings, fluidsynth::FluidEngine};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::{path::Path, sync::Arc};

const MIDI_CHANNEL: i32 = 0;

pub(crate) enum SynthBackend {
    Sampler(crate::audio::sampler::SfizzEngine),
    Fluid(FluidEngine),
    Rusty(Synthesizer),
}

impl SynthBackend {
    pub fn new(
        sf_path: &Path,
        sound_font: &Arc<SoundFont>,
        sample_rate: u32,
        piano: PianoSettings,
    ) -> Result<Self, String> {
        if let Ok(fluid) = FluidEngine::new(
            sf_path,
            sample_rate,
            piano.maximum_polyphony,
            piano.reverb_and_chorus,
        ) {
            return Ok(Self::Fluid(fluid));
        }
        let mut settings = SynthesizerSettings::new(sample_rate as i32);
        settings.block_size = 64;
        settings.maximum_polyphony = piano.maximum_polyphony;
        settings.enable_reverb_and_chorus = piano.reverb_and_chorus;
        let mut synthesizer = Synthesizer::new(sound_font, &settings)
            .map_err(|error| format!("No se pudo crear el sintetizador: {error}"))?;
        synthesizer.set_master_volume(0.7);
        Ok(Self::Rusty(synthesizer))
    }
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        match self {
            Self::Sampler(s) => s.note_on(note, velocity),
            Self::Fluid(f) => f.note_on(MIDI_CHANNEL, i32::from(note), i32::from(velocity)),
            Self::Rusty(r) => r.note_on(MIDI_CHANNEL, i32::from(note), i32::from(velocity)),
        }
    }
    pub fn note_off(&mut self, note: u8) {
        match self {
            Self::Sampler(s) => s.note_off(note),
            Self::Fluid(f) => f.note_off(MIDI_CHANNEL, i32::from(note)),
            Self::Rusty(r) => r.note_off(MIDI_CHANNEL, i32::from(note)),
        }
    }
    pub fn note_off_all(&mut self) {
        match self {
            Self::Sampler(s) => s.all_off(),
            Self::Fluid(f) => f.note_off_all(MIDI_CHANNEL),
            Self::Rusty(r) => r.note_off_all(false),
        }
    }
    pub fn panic(&mut self) {
        if let Self::Sampler(s) = self {
            s.panic();
        } else {
            self.note_off_all();
        }
    }
    pub fn midi(&mut self, command: u8, data1: u8, data2: u8) {
        match self {
            Self::Sampler(s) => s.midi(command, data1, data2),
            Self::Fluid(f) => f.process_midi(MIDI_CHANNEL, command, data1, data2),
            Self::Rusty(r) => r.process_midi_message(
                MIDI_CHANNEL,
                i32::from(command),
                i32::from(data1),
                i32::from(data2),
            ),
        }
    }
    pub fn set_master_volume(&mut self, volume: f32) {
        match self {
            Self::Sampler(_) => {}
            Self::Fluid(f) => f.set_master_volume(volume),
            Self::Rusty(r) => r.set_master_volume(volume),
        }
    }
    pub fn set_polyphony(&mut self, polyphony: usize) {
        if let Self::Fluid(f) = self {
            f.set_polyphony(polyphony);
        }
    }
    pub fn set_effects(&mut self, enabled: bool) {
        if let Self::Fluid(f) = self {
            f.set_effects(enabled);
        }
    }
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        match self {
            Self::Sampler(s) => s.render(left, right),
            Self::Fluid(f) => f.render(left, right),
            Self::Rusty(r) => r.render(left, right),
        }
    }
}
