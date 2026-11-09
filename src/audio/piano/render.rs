//! Reproductor común para SoundFont y sfizz; tiempos musicales compartidos.
//! Recursos preparados antes de abrir el stream; cola `rtrb` sin bloqueos.
use super::{InstrumentState, NoteCommand, SynthBackend};
use cpal::{FromSample, SizedSample, traits::DeviceTrait};
use elevate::musical::{
    scheduler::{EventKind, PreparedPattern},
    time::FrameWindow,
};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use rtrb::Consumer;
use std::sync::{Arc, atomic::Ordering};

const RENDER_CHUNK: usize = 512;

pub(super) fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut synthesizer: SynthBackend,
    mut commands: Consumer<NoteCommand>,
    state: Arc<InstrumentState>,
) -> Result<cpal::Stream, String> {
    let mut volume = Volume::with_gain(
        ProcessSpec::new(f64::from(config.sample_rate)).map_err(|error| error.to_string())?,
        f32::from_bits(state.gain.load(Ordering::Relaxed)),
    )
    .map_err(|error| error.to_string())?;
    let mut utility = state
        .utility
        .prepare(ProcessSpec::new(f64::from(config.sample_rate)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut fx = state.fx.prepare();
    let mut left = vec![0.0_f32; RENDER_CHUNK];
    let mut right = vec![0.0_f32; RENDER_CHUNK];
    let mut block = [[0.0_f32; 2]; RENDER_CHUNK];
    let mut active_plan: Option<(Arc<PreparedPattern>, u64)> = None;
    let error_state = state.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                for _ in 0..super::COMMAND_QUEUE_CAP {
                    let Ok(command) = commands.pop() else {
                        break;
                    };
                    match command {
                        NoteCommand::SamplerControl(cc, v) => {
                            if let SynthBackend::Sampler(s) = &mut synthesizer {
                                s.control(cc, v);
                            }
                        }
                        NoteCommand::On { note, velocity } => synthesizer.note_on(note, velocity),
                        NoteCommand::Off { note } => synthesizer.note_off(note),
                        NoteCommand::AllOff => synthesizer.panic(),
                        NoteCommand::Midi {
                            command,
                            data1,
                            data2,
                        } => synthesizer.midi(command, data1, data2),
                        NoteCommand::MasterVolume(vol) => synthesizer.set_master_volume(vol),
                        NoteCommand::Polyphony(poly) => synthesizer.set_polyphony(poly),
                        NoteCommand::Effects(on) => synthesizer.set_effects(on),
                        NoteCommand::PlayPlan { plan, start_frame } => {
                            synthesizer.panic();
                            active_plan = Some((plan, start_frame));
                        }
                        NoteCommand::StopPlan => {
                            if active_plan.take().is_some() {
                                synthesizer.panic();
                            }
                        }
                    }
                }
                let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                if volume.gain() != gain && volume.set_gain(gain).is_err() {
                    state.failed.store(true, Ordering::Relaxed);
                }
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
                    fx.process(&state.fx, &mut block[..frames]);
                    volume.process(&mut block[..frames]);
                    state.pan.apply(&mut block[..frames]);
                    utility.process(&mut block[..frames]);
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

pub(super) fn render_plan_segment(
    synthesizer: &mut SynthBackend,
    plan: &PreparedPattern,
    position: &mut u64,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    let total = left.len();
    let planned = plan
        .length_frames()
        .saturating_sub(*position)
        .min(total as u64) as usize;
    let Ok(window) = FrameWindow::new(*position, planned as u32) else {
        synthesizer.render(left, right);
        return true;
    };
    let mut cursor = 0;
    for event in plan.events_in(window) {
        let offset = (event.sample_offset as usize).min(planned);
        if offset > cursor {
            synthesizer.render(&mut left[cursor..offset], &mut right[cursor..offset]);
            cursor = offset;
        }
        match event.kind {
            EventKind::NoteOn { velocity } => synthesizer.note_on(event.pitch, velocity),
            EventKind::NoteOff => synthesizer.note_off(event.pitch),
        }
    }
    if cursor < planned {
        synthesizer.render(&mut left[cursor..planned], &mut right[cursor..planned]);
    }
    *position = position.saturating_add(total as u64);
    if *position >= plan.length_frames() {
        synthesizer.note_off_all();
        if planned < total {
            synthesizer.render(&mut left[planned..], &mut right[planned..]);
        }
        return true;
    }
    false
}
