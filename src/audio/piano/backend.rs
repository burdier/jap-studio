//! Motor de síntesis compartido por audición y render offline. Sin dispositivo de salida.
use super::{PianoSettings, fluidsynth::FluidEngine};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::{path::Path, sync::Arc};

pub(crate) enum SynthBackend {
    Vst(crate::audio::vst3::InstrumentVoice),
    Sampler(crate::audio::sampler::SfizzEngine),
    Fluid(FluidEngine),
    Rusty(Synthesizer),
}

impl SynthBackend {
    pub fn scheduled(
        &mut self,
        channel: u8,
        event: elevate::musical::scheduler::ScheduledEvent,
        cycle: u64,
        reserved: u16,
    ) {
        use elevate::musical::scheduler::EventKind;
        if let EventKind::Control { lane, value } = event.kind {
            let (command, a, b) = lane.midi(value);
            self.midi(command | channel, a, b);
            return;
        }
        if !event.expression.plays(event.note, cycle) {
            return;
        }
        if event.initial_pitch_cents.is_some()
            && let Self::Vst(voice) = self
            && voice.note_pitch_event(event, reserved)
        {
            return;
        }
        match event.kind {
            EventKind::NoteOn { velocity } => {
                self.note_on_expression(channel, event.pitch, velocity, event.expression);
            }
            EventKind::NoteOff => self.midi(
                0x80 | channel,
                event.pitch,
                event.expression.release_velocity,
            ),
            _ => {}
        }
    }

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
    fn note_on(&mut self, channel: i32, note: u8, velocity: u8) {
        match self {
            Self::Vst(v) => v.midi(0x90 | channel as u8, note, velocity),
            Self::Sampler(s) => s.note_on(note, velocity),
            Self::Fluid(f) => f.note_on(channel, i32::from(note), i32::from(velocity)),
            Self::Rusty(r) => r.note_on(channel, i32::from(note), i32::from(velocity)),
        }
    }
    pub fn note_on_expression(
        &mut self,
        channel: u8,
        note: u8,
        velocity: u8,
        expression: elevate::musical::note_expression::NoteExpression,
    ) {
        if let Self::Fluid(fluid) = self {
            fluid.note_on_pan(
                i32::from(channel),
                i32::from(note),
                i32::from(velocity),
                expression.pan,
            );
        } else {
            // sfizz/RustySynth no exponen pan por voz en estos adaptadores.
            // No convertirlo a CC10: afectaría a las demás notas del canal.
            self.note_on(i32::from(channel), note, velocity);
        }
    }
    fn note_off(&mut self, channel: i32, note: u8, release_velocity: u8) {
        match self {
            Self::Vst(v) => v.midi(0x80 | channel as u8, note, release_velocity),
            Self::Sampler(s) => s.note_off_velocity(note, release_velocity),
            Self::Fluid(f) => f.note_off(channel, i32::from(note)),
            Self::Rusty(r) => r.note_off(channel, i32::from(note)),
        }
    }
    pub fn note_off_all(&mut self) {
        match self {
            Self::Vst(v) => v.panic(),
            Self::Sampler(s) => s.all_off(),
            Self::Fluid(f) => {
                for channel in 0..16 {
                    f.note_off_all(channel);
                }
            }
            Self::Rusty(r) => r.note_off_all(false),
        }
    }
    pub fn panic(&mut self) {
        if let Self::Sampler(s) = self {
            s.panic();
        } else {
            for channel in 0..16 {
                self.midi(0xb0 | channel, 64, 0);
            }
            self.note_off_all();
        }
    }
    pub fn midi(&mut self, command: u8, data1: u8, data2: u8) {
        let channel = i32::from(command & 0x0f);
        let command = command & 0xf0;
        if command == 0x90 && data2 > 0 {
            self.note_on(channel, data1, data2);
            return;
        }
        if command == 0x80 || command == 0x90 {
            self.note_off(channel, data1, if command == 0x90 { 64 } else { data2 });
            return;
        }
        match self {
            Self::Vst(v) => v.midi(command | channel as u8, data1, data2),
            Self::Sampler(s) => s.midi(command, data1, data2),
            Self::Fluid(f) => f.process_midi(channel, command, data1, data2),
            Self::Rusty(r) => r.process_midi_message(
                channel,
                i32::from(command),
                i32::from(data1),
                i32::from(data2),
            ),
        }
    }
    pub fn set_master_volume(&mut self, volume: f32) {
        match self {
            Self::Sampler(_) | Self::Vst(_) => {}
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
        self.render_timed(left, right, None);
    }
    pub fn render_timed(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        time: Option<crate::audio::vst3::TransportContext>,
    ) {
        match self {
            Self::Vst(v) => v.render(left, right, time),
            Self::Sampler(s) => s.render(left, right),
            Self::Fluid(f) => f.render(left, right),
            Self::Rusty(r) => r.render(left, right),
        }
    }

    pub fn failed(&self) -> bool {
        matches!(self, Self::Vst(v) if v.failed())
    }
}
