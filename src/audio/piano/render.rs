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
    first_channel: u16,
    mut synthesizer: SynthBackend,
    mut commands: Consumer<NoteCommand>,
    mut spectrum: rtrb::Producer<[f32; 2]>,
    state: Arc<InstrumentState>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    let first = usize::from(first_channel);
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
    let mut filter = super::filter::StereoFilter::new(config.sample_rate, Default::default());
    let mut left = vec![0.0_f32; RENDER_CHUNK];
    let mut right = vec![0.0_f32; RENDER_CHUNK];
    let mut block = [[0.0_f32; 2]; RENDER_CHUNK];
    let mut active_plan: Option<(Arc<PreparedPattern>, u64, bool, u64)> = None;
    let mut channel = 0;
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
                        NoteCommand::Channel(value) => {
                            synthesizer.panic();
                            channel = value;
                        }
                        NoteCommand::Filter(settings) => filter.set(settings),
                        NoteCommand::SamplerControl(cc, v) => {
                            if let SynthBackend::Sampler(s) = &mut synthesizer {
                                s.control(cc, v);
                            }
                        }
                        NoteCommand::On { note, velocity } => {
                            synthesizer.midi(0x90 | channel, note, velocity)
                        }
                        NoteCommand::Off { note } => synthesizer.midi(0x80 | channel, note, 0),
                        NoteCommand::AllOff => synthesizer.panic(),
                        NoteCommand::Midi {
                            command,
                            data1,
                            data2,
                        } => synthesizer.midi((command & 0xf0) | channel, data1, data2),
                        NoteCommand::MasterVolume(vol) => synthesizer.set_master_volume(vol),
                        NoteCommand::Polyphony(poly) => synthesizer.set_polyphony(poly),
                        NoteCommand::Effects(on) => synthesizer.set_effects(on),
                        NoteCommand::PlayPlan {
                            plan,
                            start_frame,
                            looped,
                        } => {
                            synthesizer.panic();
                            active_plan = Some((plan, start_frame, looped, 0));
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
                for output_block in output.chunks_mut(RENDER_CHUNK * channels) {
                    let frames = output_block.len() / channels;
                    if let Some((plan, position, looped, cycle)) = active_plan.as_mut() {
                        let finished = render_plan_block(
                            &mut synthesizer,
                            plan,
                            position,
                            *looped,
                            channel,
                            cycle,
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
                    if synthesizer.failed() {
                        state.failed.store(true, Ordering::Relaxed);
                    }
                    for (frame, (&left, &right)) in block[..frames]
                        .iter_mut()
                        .zip(left.iter().zip(right.iter()))
                    {
                        *frame = [left, right];
                    }
                    filter.process(&mut block[..frames]);
                    fx.process(&state.fx, &mut block[..frames]);
                    volume.process(&mut block[..frames]);
                    state.pan.apply(&mut block[..frames]);
                    utility.process(&mut block[..frames]);
                    if state.spectrum_enabled.load(Ordering::Relaxed) {
                        for frame in &block[..frames] {
                            let _ = spectrum.push(*frame);
                        }
                    }
                    super::super::output::write_stereo(
                        output_block,
                        &block[..frames],
                        channels,
                        first,
                    );
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
    channel: u8,
    cycle: u64,
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
        play_event(synthesizer, event, channel, cycle);
    }
    if cursor < planned {
        synthesizer.render(&mut left[cursor..planned], &mut right[cursor..planned]);
    }
    *position = position.saturating_add(total as u64);
    if *position >= plan.length_frames() {
        // Los Off exactamente en el final quedan fuera de la ventana semiabierta.
        if let Ok(end) = FrameWindow::new(plan.length_frames(), 1) {
            for event in plan
                .events_in(end)
                .filter(|event| matches!(event.kind, EventKind::NoteOff))
            {
                play_event(synthesizer, event, channel, cycle);
            }
        }
        synthesizer.midi(0xb0 | channel, 64, 0);
        synthesizer.note_off_all();
        if planned < total {
            synthesizer.render(&mut left[planned..], &mut right[planned..]);
        }
        return true;
    }
    false
}

/// Divide el bloque en los límites del ciclo. Una vuelta consume al menos un frame;
/// el trabajo queda acotado por el tamaño del bloque, incluso con patrones diminutos.
fn render_plan_block(
    synthesizer: &mut SynthBackend,
    plan: &PreparedPattern,
    position: &mut u64,
    looped: bool,
    channel: u8,
    cycle: &mut u64,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    if !looped || plan.length_frames() == 0 {
        return render_plan_segment(synthesizer, plan, position, channel, *cycle, left, right);
    }
    let mut written = 0;
    while written < left.len() {
        *position %= plan.length_frames();
        let count = (plan.length_frames() - *position).min((left.len() - written) as u64) as usize;
        let end = written + count;
        render_plan_segment(
            synthesizer,
            plan,
            position,
            channel,
            *cycle,
            &mut left[written..end],
            &mut right[written..end],
        );
        written = end;
        if *position >= plan.length_frames() {
            *position = 0;
            *cycle = cycle.wrapping_add(1);
        }
    }
    false
}

fn play_event(
    synthesizer: &mut SynthBackend,
    event: elevate::musical::scheduler::ScheduledEvent,
    channel: u8,
    cycle: u64,
) {
    match event.kind {
        EventKind::Control { lane, value } => {
            let (command, a, b) = lane.midi(value);
            synthesizer.midi(command | channel, a, b);
        }
        EventKind::NoteOn { velocity } if event.expression.plays(event.note, cycle) => {
            synthesizer.note_on_expression(channel, event.pitch, velocity, event.expression)
        }
        EventKind::NoteOff if event.expression.plays(event.note, cycle) => synthesizer.midi(
            0x80 | channel,
            event.pitch,
            event.expression.release_velocity,
        ),
        _ => {}
    }
}
