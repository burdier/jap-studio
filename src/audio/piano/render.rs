//! Síntesis dentro del callback. Recursos preparados antes de abrir el stream.
use super::{InstrumentState, NoteCommand};
use cpal::{FromSample, SizedSample, traits::DeviceTrait};
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
) -> Result<cpal::Stream, String> {
    let mut settings = SynthesizerSettings::new(config.sample_rate as i32);
    settings.block_size = 64;
    settings.maximum_polyphony = 64;
    settings.enable_reverb_and_chorus = true;
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
                    }
                }
                let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                if volume.gain() != gain && volume.set_gain(gain).is_err() {
                    state.failed.store(true, Ordering::Relaxed);
                }
                volume.set_bypassed(state.bypass.load(Ordering::Relaxed));
                for output_block in output.chunks_mut(RENDER_CHUNK * 2) {
                    let frames = output_block.len() / 2;
                    synthesizer.render(&mut left[..frames], &mut right[..frames]);
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
