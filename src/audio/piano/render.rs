//! Reproductor común para SoundFont y sfizz; tiempos musicales compartidos.
//! Recursos preparados antes de abrir el stream; cola `rtrb` sin bloqueos.
use super::{InstrumentState, NoteCommand, SynthBackend, voice::PreparedVoices};
use cpal::{FromSample, SizedSample, traits::DeviceTrait};
use elevate::musical::model::InstrumentId;
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use rtrb::Consumer;
use std::sync::{Arc, atomic::Ordering};

const RENDER_CHUNK: usize = 512;

pub(super) fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    first_channel: u16,
    synthesizer: SynthBackend,
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
    let mut block = [[0.0_f32; 2]; RENDER_CHUNK];
    let mut voices = PreparedVoices::new();
    // ID local de la ruta exclusiva actual; PR-05 conectará los IDs reales de las pistas.
    let preview_id = InstrumentId(0);
    voices
        .insert(preview_id, synthesizer, state.clone())
        .map_err(|_| "Instancia duplicada")?;
    let error_state = state.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                for _ in 0..super::COMMAND_QUEUE_CAP {
                    let Ok(command) = commands.pop() else {
                        break;
                    };
                    if let NoteCommand::Filter(settings) = command {
                        filter.set(settings);
                    } else {
                        voices.command(preview_id, command);
                    }
                }
                let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                if volume.gain() != gain && volume.set_gain(gain).is_err() {
                    state.failed.store(true, Ordering::Relaxed);
                }
                for output_block in output.chunks_mut(RENDER_CHUNK * channels) {
                    let frames = output_block.len() / channels;
                    voices.render(&mut block[..frames]);
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
