//! Callback de percusión: transporte por frames, loop y offsets de evento.
//! El plan y el banco de voces se preparan antes de abrir el stream.
use super::{DrumState, voice::DrumKit};
use cpal::{FromSample, SizedSample, traits::DeviceTrait};
use elevate::musical::{
    scheduler::{EventKind, PreparedPattern},
    time::{FrameWindow, Transport},
};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use std::sync::{Arc, atomic::Ordering};

const RENDER_CHUNK: usize = 512;

pub(super) fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    first_channel: u16,
    plan: Arc<PreparedPattern>,
    state: Arc<DrumState>,
) -> Result<cpal::Stream, String> {
    let sample_rate = f64::from(config.sample_rate);
    let dt = 1.0 / sample_rate;
    let loop_frames = plan.length_frames();
    let channels = usize::from(config.channels);
    let first = usize::from(first_channel);
    let mut volume = Volume::with_gain(
        ProcessSpec::new(sample_rate).map_err(|error| error.to_string())?,
        f32::from_bits(state.gain.load(Ordering::Relaxed)),
    )
    .map_err(|error| error.to_string())?;
    let mut utility = state
        .utility
        .prepare(ProcessSpec::new(sample_rate).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut transport = Transport::default();
    transport.play();
    let mut kit = DrumKit::new();
    let mut block = [[0.0_f32; 2]; RENDER_CHUNK];
    let error_state = state.clone();

    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                if volume.gain() != gain && volume.set_gain(gain).is_err() {
                    state.failed.store(true, Ordering::Relaxed);
                }

                for output_block in output.chunks_mut(RENDER_CHUNK * channels) {
                    let frames = output_block.len() / channels;
                    block[..frames].fill([0.0; 2]);
                    let window = match transport.advance(frames as u32) {
                        Ok(Some(window)) => Some(window),
                        Ok(None) => None,
                        Err(_) => {
                            state.failed.store(true, Ordering::Relaxed);
                            None
                        }
                    };
                    if let Some(window) = window {
                        render_window(
                            &plan,
                            &mut kit,
                            &mut block[..frames],
                            window.start(),
                            loop_frames,
                            dt,
                        );
                    }
                    volume.process(&mut block[..frames]);
                    utility.process(&mut block[..frames]);
                    super::super::output::write_stereo(
                        output_block,
                        &block[..frames],
                        channels,
                        first,
                    );
                }
                state.frame.store(transport.frame(), Ordering::Relaxed);
            },
            move |error| {
                eprintln!("Error de salida de percusión: {error}");
                error_state.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|error| error.to_string())
}

/// Divide el bloque en tramos que no cruzan el final del loop. Los NoteOff de
/// percusión se ignoran: cada voz es un one-shot con su propia envolvente.
fn render_window(
    plan: &PreparedPattern,
    kit: &mut DrumKit,
    frames: &mut [[f32; 2]],
    start_frame: u64,
    loop_frames: u64,
    dt: f64,
) {
    let mut local = start_frame % loop_frames;
    let mut written = 0;
    let mut remaining = frames.len();
    while remaining > 0 {
        let segment = (loop_frames - local).min(remaining as u64) as usize;
        let Ok(window) = FrameWindow::new(local, segment as u32) else {
            return;
        };
        let mut cursor = 0;
        for event in plan.events_in(window) {
            let offset = event.sample_offset as usize;
            if offset > cursor {
                kit.render(&mut frames[written + cursor..written + offset], dt);
                cursor = offset;
            }
            if let EventKind::NoteOn { velocity } = event.kind
                && event.expression.plays(
                    event.note,
                    start_frame.saturating_add(written as u64) / loop_frames,
                )
            {
                kit.trigger(event.instrument, velocity, event.note.0);
            }
        }
        if cursor < segment {
            kit.render(&mut frames[written + cursor..written + segment], dt);
        }
        written += segment;
        remaining -= segment;
        local = (local + segment as u64) % loop_frames;
    }
}
