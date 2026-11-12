//! Reproductor de UN clip. Adaptador provisional: no es el motor multipista.
use super::{AudioClip, output::choose_output_config, stereo_pan::StereoPan};
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use elevate::devices::utility_slot::UtilitySlot;
use elevate::musical::time::Tempo;
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

struct PlaybackState {
    gain: AtomicU32,
    pan: StereoPan,
    fx: super::fx_gain::FxGain,
    utility: UtilitySlot,
    position: AtomicU64,
    finished: AtomicBool,
    failed: AtomicBool,
}

pub(crate) struct Player {
    _stream: cpal::Stream,
    state: Arc<PlaybackState>,
    pub sample_rate: u32,
    beats_per_second: f64,
}

pub(crate) struct PreparedPlayback {
    device: cpal::Device,
    setup: super::output::StreamSetup,
    clip: AudioClip,
    converted: Option<Arc<Vec<[f32; 2]>>>,
    rack: Option<Arc<super::effects::Rack>>,
}
impl PreparedPlayback {
    /// Consulta y remuestreo costoso fuera de la UI y del callback.
    pub fn load(clip: AudioClip, audio: super::device::AudioConfig) -> Result<Self, String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, &audio)?;
        let rate = setup.config.sample_rate;
        let converted = if rate == clip.file.sample_rate {
            None
        } else {
            Some(Arc::new(super::export::resample(
                &clip.file.frames,
                clip.file.sample_rate,
                rate,
            )?))
        };
        Ok(Self {
            device,
            setup,
            clip,
            converted,
            rack: None,
        })
    }
    pub fn with_rack(mut self, rack: Option<Arc<super::effects::Rack>>) -> Self {
        self.rack = rack;
        self
    }
    pub fn start(
        self,
        start_beat: f64,
        bpm: f64,
        gain: f32,
        utility: UtilitySlot,
        pan: f32,
        fx_gain: f32,
    ) -> Result<Player, String> {
        let Self {
            device,
            setup,
            clip,
            converted,
            rack,
        } = self;
        let beats_per_second = Tempo::new(bpm)
            .map_err(|error| error.to_string())?
            .beats_per_second();
        let rate = setup.config.sample_rate;
        let state = Arc::new(PlaybackState {
            gain: AtomicU32::new(gain.to_bits()),
            pan: StereoPan::new(pan),
            fx: super::fx_gain::FxGain::new(fx_gain),
            utility,
            position: AtomicU64::new(
                (start_beat / beats_per_second * f64::from(rate)).round() as u64
            ),
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        state.fx.set_rack(rack, rate);
        let stream = match setup.format {
            SampleFormat::F32 => Player::stream::<f32>(
                &device,
                setup.config,
                setup.first_channel,
                &clip,
                converted,
                state.clone(),
                beats_per_second,
            ),
            SampleFormat::I16 => Player::stream::<i16>(
                &device,
                setup.config,
                setup.first_channel,
                &clip,
                converted,
                state.clone(),
                beats_per_second,
            ),
            SampleFormat::I32 => Player::stream::<i32>(
                &device,
                setup.config,
                setup.first_channel,
                &clip,
                converted,
                state.clone(),
                beats_per_second,
            ),
            SampleFormat::U16 => Player::stream::<u16>(
                &device,
                setup.config,
                setup.first_channel,
                &clip,
                converted,
                state.clone(),
                beats_per_second,
            ),
            _ => return Err("Formato de salida no compatible".into()),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Player {
            _stream: stream,
            state,
            sample_rate: rate,
            beats_per_second,
        })
    }
}

impl Player {
    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        first_channel: u16,
        clip: &AudioClip,
        converted: Option<Arc<Vec<[f32; 2]>>>,
        state: Arc<PlaybackState>,
        beats_per_second: f64,
    ) -> Result<cpal::Stream, String> {
        let channels = usize::from(config.channels);
        let first = usize::from(first_channel);
        let rate = f64::from(config.sample_rate);
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
        let file = clip.file.clone();
        let clip_start = (clip.start_beat / beats_per_second * rate).round() as u64;
        let source_offset = (clip.trim_start * rate).round() as u64;
        let visible = (clip.length_seconds() * rate).round() as u64;
        let clip_end = clip_start.saturating_add(visible);
        let mut position = state.position.load(Ordering::Relaxed);
        let error_state = state.clone();
        device
            .build_output_stream(
                config,
                move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let gain = f32::from_bits(state.gain.load(Ordering::Relaxed));
                    if volume.gain() != gain && volume.set_gain(gain).is_err() {
                        state.failed.store(true, Ordering::Relaxed);
                    }
                    let mut block = [[0.0_f32; 2]; 256];
                    for output_block in output.chunks_mut(256 * channels) {
                        let count = output_block.len() / channels;
                        for frame in block.iter_mut().take(count) {
                            *frame = position
                                .checked_sub(clip_start)
                                .filter(|index| *index < visible)
                                .and_then(|index| {
                                    converted
                                        .as_deref()
                                        .unwrap_or(&file.frames)
                                        .get((index + source_offset) as usize)
                                })
                                .copied()
                                .unwrap_or([0.0; 2]);
                            position = position.saturating_add(1).min(clip_end);
                        }
                        fx.process(&state.fx, &mut block[..count]);
                        volume.process(&mut block[..count]);
                        state.pan.apply(&mut block[..count]);
                        utility.process(&mut block[..count]);
                        super::output::write_stereo(output_block, &block[..count], channels, first);
                    }
                    state.position.store(position, Ordering::Relaxed);
                    if position >= clip_end {
                        state.finished.store(true, Ordering::Relaxed);
                    }
                },
                move |error| {
                    eprintln!("Error de salida de audio: {error}");
                    error_state.failed.store(true, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|error| error.to_string())
    }

    pub fn set_fx_rack(&self, rack: Option<std::sync::Arc<crate::audio::effects::Rack>>) {
        self.state.fx.set_rack(rack, self.sample_rate);
    }
    pub fn set_fx_gain(&self, gain: f32) {
        self.state.fx.set(gain);
    }
    pub fn set_gain(&self, gain: f32) {
        self.state
            .gain
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }
    pub fn set_pan(&self, pan: f32) {
        self.state.pan.set(pan);
    }
    pub fn beat(&self) -> f64 {
        self.state.position.load(Ordering::Relaxed) as f64 / f64::from(self.sample_rate)
            * self.beats_per_second
    }
    pub fn finished(&self) -> bool {
        self.state.finished.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}
