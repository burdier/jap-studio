//! Síntesis dentro del callback. Recursos preparados antes de abrir el stream.
use super::{InstrumentState, NoteCommand, PianoSettings};
use cpal::{FromSample, SizedSample, traits::DeviceTrait};
use elevate::musical::{
    scheduler::{EventKind, PreparedPattern},
    time::FrameWindow,
};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::sync::{Arc, atomic::Ordering, mpsc};

const RENDER_CHUNK: usize = 512;
const MIDI_CHANNEL: i32 = 0;

pub(super) fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    sound_font: Arc<SoundFont>,
    commands: mpsc::Receiver<NoteCommand>,
    state: Arc<InstrumentState>,
    piano: PianoSettings,
) -> Result<cpal::Stream, String> {
    let mut settings = SynthesizerSettings::new(config.sample_rate as i32);
    settings.block_size = 64;
    settings.maximum_polyphony = piano.maximum_polyphony;
    settings.enable_reverb_and_chorus = piano.reverb_and_chorus;
    let mut synthesizer = Synthesizer::new(&sound_font, &settings)
        .map_err(|error| format!("No se pudo crear el sintetizador: {error}"))?;
    synthesizer.set_master_volume(0.7);
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
                while let Ok(command) = commands.try_recv() {
                    match command {
                        NoteCommand::On { note, velocity } => {
                            synthesizer.note_on(MIDI_CHANNEL, i32::from(note), i32::from(velocity))
                        }
                        NoteCommand::Off { note } => {
                            synthesizer.note_off(MIDI_CHANNEL, i32::from(note))
                        }
                        NoteCommand::AllOff => synthesizer.note_off_all(false),
                        NoteCommand::Midi {
                            command,
                            data1,
                            data2,
                        } => synthesizer.process_midi_message(
                            MIDI_CHANNEL,
                            i32::from(command),
                            i32::from(data1),
                            i32::from(data2),
                        ),
                        NoteCommand::MasterVolume(volume) => synthesizer.set_master_volume(volume),
                        NoteCommand::PlayPlan { plan, start_frame } => {
                            synthesizer.note_off_all(false);
                            active_plan = Some((plan, start_frame));
                        }
                        NoteCommand::StopPlan => {
                            if active_plan.take().is_some() {
                                synthesizer.note_off_all(false);
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
    synthesizer: &mut Synthesizer,
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
            EventKind::NoteOn { velocity } => {
                synthesizer.note_on(MIDI_CHANNEL, i32::from(event.pitch), i32::from(velocity))
            }
            EventKind::NoteOff => synthesizer.note_off(MIDI_CHANNEL, i32::from(event.pitch)),
        }
    }
    if cursor < total {
        synthesizer.render(&mut left[cursor..total], &mut right[cursor..total]);
    }
    *position = position.saturating_add(total as u64);
    if *position >= plan.length_frames() {
        synthesizer.note_off_all(false);
        return true;
    }
    false
}
