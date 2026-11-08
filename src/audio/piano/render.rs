//! Síntesis dentro del callback (`libfluidsynth` C aislado con respaldo `RustySynth`).
//! Recursos preparados antes de abrir el stream; cola `rtrb` sin bloqueos.
use super::{InstrumentState, NoteCommand, PianoSettings, fluidsynth::FluidEngine};
use cpal::{FromSample, SizedSample, traits::DeviceTrait};
use elevate::musical::{
    scheduler::{EventKind, PreparedPattern},
    time::FrameWindow,
};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use rtrb::Consumer;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::{
    path::Path,
    sync::{Arc, atomic::Ordering},
};

const RENDER_CHUNK: usize = 512;
const MIDI_CHANNEL: i32 = 0;

enum SynthBackend {
    Fluid(FluidEngine),
    Rusty(Synthesizer),
}

impl SynthBackend {
    fn new(
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

    fn note_on(&mut self, note: u8, velocity: u8) {
        match self {
            Self::Fluid(f) => f.note_on(MIDI_CHANNEL, i32::from(note), i32::from(velocity)),
            Self::Rusty(r) => r.note_on(MIDI_CHANNEL, i32::from(note), i32::from(velocity)),
        }
    }

    fn note_off(&mut self, note: u8) {
        match self {
            Self::Fluid(f) => f.note_off(MIDI_CHANNEL, i32::from(note)),
            Self::Rusty(r) => r.note_off(MIDI_CHANNEL, i32::from(note)),
        }
    }

    fn note_off_all(&mut self) {
        match self {
            Self::Fluid(f) => f.note_off_all(MIDI_CHANNEL),
            Self::Rusty(r) => r.note_off_all(false),
        }
    }

    fn midi(&mut self, command: u8, data1: u8, data2: u8) {
        match self {
            Self::Fluid(f) => f.process_midi(MIDI_CHANNEL, command, data1, data2),
            Self::Rusty(r) => r.process_midi_message(
                MIDI_CHANNEL,
                i32::from(command),
                i32::from(data1),
                i32::from(data2),
            ),
        }
    }

    fn set_master_volume(&mut self, volume: f32) {
        match self {
            Self::Fluid(f) => f.set_master_volume(volume),
            Self::Rusty(r) => r.set_master_volume(volume),
        }
    }

    fn set_polyphony(&mut self, polyphony: usize) {
        if let Self::Fluid(f) = self {
            f.set_polyphony(polyphony);
        }
    }

    fn set_effects(&mut self, enabled: bool) {
        if let Self::Fluid(f) = self {
            f.set_effects(enabled);
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        match self {
            Self::Fluid(f) => f.render(left, right),
            Self::Rusty(r) => r.render(left, right),
        }
    }
}

pub(super) fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    sf_path: &Path,
    sound_font: Arc<SoundFont>,
    mut commands: Consumer<NoteCommand>,
    state: Arc<InstrumentState>,
    piano: PianoSettings,
) -> Result<cpal::Stream, String> {
    let mut synthesizer = SynthBackend::new(sf_path, &sound_font, config.sample_rate, piano)?;
    let mut volume = Volume::with_gain(
        ProcessSpec::new(f64::from(config.sample_rate)).map_err(|error| error.to_string())?,
        f32::from_bits(state.gain.load(Ordering::Relaxed)),
    )
    .map_err(|error| error.to_string())?;
    volume.set_bypassed(state.bypass.load(Ordering::Relaxed));
    let mut left = vec![0.0_f32; RENDER_CHUNK];
    let mut right = vec![0.0_f32; RENDER_CHUNK];
    let mut block = [[0.0_f32; 2]; RENDER_CHUNK];
    let mut active_plan: Option<(Arc<PreparedPattern>, u64)> = None;
    let error_state = state.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                while let Ok(command) = commands.pop() {
                    match command {
                        NoteCommand::On { note, velocity } => synthesizer.note_on(note, velocity),
                        NoteCommand::Off { note } => synthesizer.note_off(note),
                        NoteCommand::AllOff => synthesizer.note_off_all(),
                        NoteCommand::Midi {
                            command,
                            data1,
                            data2,
                        } => synthesizer.midi(command, data1, data2),
                        NoteCommand::MasterVolume(vol) => synthesizer.set_master_volume(vol),
                        NoteCommand::Polyphony(poly) => synthesizer.set_polyphony(poly),
                        NoteCommand::Effects(on) => synthesizer.set_effects(on),
                        NoteCommand::PlayPlan { plan, start_frame } => {
                            synthesizer.note_off_all();
                            active_plan = Some((plan, start_frame));
                        }
                        NoteCommand::StopPlan => {
                            if active_plan.take().is_some() {
                                synthesizer.note_off_all();
                            }
                        }
                    }
                }
                let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                if volume.gain() != gain && volume.set_gain(gain).is_err() {
                    state.failed.store(true, Ordering::Relaxed);
                }
                volume.set_bypassed(state.bypass.load(Ordering::Relaxed));
                for output_block in output.chunks_mut(RENDER_CHUNK * 2) {
                    let frames = output_block.len() / 2;
                    if let Some((plan, position)) = active_plan.as_mut() {
                        let finished = render_plan_segment(
                            &mut synthesizer,
                            plan,
                            position,
                            &mut left[..frames],
                            &mut right[..frames],
                        );
                        state.plan_frame.store(*position, Ordering::Relaxed);
                        if finished {
                            active_plan = None;
                            state.plan_playing.store(false, Ordering::Relaxed);
                            state.plan_finished.store(true, Ordering::Relaxed);
                        }
                    } else {
                        synthesizer.render(&mut left[..frames], &mut right[..frames]);
                    }
                    for (frame, (&left, &right)) in block[..frames]
                        .iter_mut()
                        .zip(left.iter().zip(right.iter()))
                    {
                        *frame = [left, right];
                    }
                    volume.process(&mut block[..frames]);
                    for (destination, frame) in output_block.chunks_exact_mut(2).zip(block.iter()) {
                        destination[0] = T::from_sample(frame[0].clamp(-1.0, 1.0));
                        destination[1] = T::from_sample(frame[1].clamp(-1.0, 1.0));
                    }
                }
            },
            move |error| {
                eprintln!("Error de salida del piano: {error}");
                error_state.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|error| error.to_string())
}

fn render_plan_segment(
    synthesizer: &mut SynthBackend,
    plan: &PreparedPattern,
    position: &mut u64,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    let total = left.len();
    let Ok(window) = FrameWindow::new(*position, total as u32) else {
        synthesizer.render(left, right);
        return true;
    };
    let mut cursor = 0;
    for event in plan.events_in(window) {
        let offset = (event.sample_offset as usize).min(total);
        if offset > cursor {
            synthesizer.render(&mut left[cursor..offset], &mut right[cursor..offset]);
            cursor = offset;
        }
        match event.kind {
            EventKind::NoteOn { velocity } => synthesizer.note_on(event.pitch, velocity),
            EventKind::NoteOff => synthesizer.note_off(event.pitch),
        }
    }
    if cursor < total {
        synthesizer.render(&mut left[cursor..total], &mut right[cursor..total]);
    }
    *position = position.saturating_add(total as u64);
    if *position >= plan.length_frames() {
        synthesizer.note_off_all();
        return true;
    }
    false
}
