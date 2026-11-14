//! Mezclador de clips: un stream y posición compartida en frames.
mod mix;
mod transport;
use super::drums::DrumInput;
use super::publication::Publication;
use super::{
    AudioClip, clip_source::ClipSource, output::choose_output_config, stereo_pan::StereoPan,
};
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use elevate::devices::utility_slot::UtilitySlot;
use elevate::musical::time::{Meter, Tempo};
use elevate::musical::track::TrackId;
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
pub(crate) use mix::{MidiTrackInput, TrackInput};
use mix::{Mix, TrackControl};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use transport::{Cursor, Request, beat_frame, loop_frames};

struct PlaybackState {
    request: Publication<Request>,
    applied_request: AtomicU64,
    tracks: Vec<Arc<TrackControl>>,
    drums: Option<Arc<AtomicU32>>,
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
    request: Request,
}

pub(crate) struct PreparedPlayback {
    device: cpal::Device,
    setup: super::output::StreamSetup,
    mix: Mix,
}
impl PreparedPlayback {
    /// Consulta y remuestreo costoso fuera de la UI y del callback.
    pub fn load(
        inputs: Vec<TrackInput>,
        drums: Option<DrumInput>,
        bpm: f64,
        meter: Meter,
        audio: super::device::AudioConfig,
    ) -> Result<Self, String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, &audio)?;
        let mix = Mix::prepare(inputs, drums, setup.config.sample_rate, bpm, meter)?;
        Ok(Self { device, setup, mix })
    }
    pub fn start(
        self,
        start_beat: f64,
        bpm: f64,
        utility: UtilitySlot,
        loop_range: Option<(f64, f64)>,
    ) -> Result<Player, String> {
        let Self { device, setup, mix } = self;
        let beats_per_second = Tempo::new(bpm)
            .map_err(|error| error.to_string())?
            .beats_per_second();
        let rate = setup.config.sample_rate;
        let request = Request {
            loop_range: loop_frames(loop_range, rate, beats_per_second)?,
            ..Default::default()
        };
        let start = beat_frame(start_beat, rate, beats_per_second)?;
        let state = Arc::new(PlaybackState {
            request: Publication::new(request),
            applied_request: AtomicU64::new(0),
            tracks: mix.tracks.iter().map(|t| t.control.clone()).collect(),
            drums: mix.drum_control(),
            utility,
            position: AtomicU64::new(start),
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let stream = match setup.format {
            SampleFormat::F32 => Player::stream::<f32>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                state.clone(),
            ),
            SampleFormat::I16 => Player::stream::<i16>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                state.clone(),
            ),
            SampleFormat::I32 => Player::stream::<i32>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                state.clone(),
            ),
            SampleFormat::U16 => Player::stream::<u16>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                state.clone(),
            ),
            _ => return Err("Formato de salida no compatible".into()),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Player {
            _stream: stream,
            state,
            sample_rate: rate,
            beats_per_second,
            request,
        })
    }
}

impl Player {
    pub fn set_drum_gain(&self, gain: f32) {
        if let Some(control) = &self.state.drums {
            control.store(gain.clamp(0., 2.).to_bits(), Ordering::Relaxed);
        }
    }
    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        first_channel: u16,
        mut mix: Mix,
        state: Arc<PlaybackState>,
    ) -> Result<cpal::Stream, String> {
        let channels = usize::from(config.channels);
        let first = usize::from(first_channel);
        let mut utility = state
            .utility
            .prepare(ProcessSpec::new(f64::from(config.sample_rate)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let clip_end = mix.end;
        let mut cursor = Cursor::new(state.position.load(Ordering::Relaxed), clip_end);
        let error_state = state.clone();
        device
            .build_output_stream(
                config,
                move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let request = state.request.read(|r| *r);
                    let mut block = [[0.0_f32; 2]; 256];
                    for output_block in output.chunks_mut(256 * channels) {
                        let count = output_block.len() / channels;
                        cursor.render(request, &mut block[..count], |at, frames| {
                            mix.render_timed(at, frames, request.loop_range)
                        });
                        utility.process(&mut block[..count]);
                        super::output::write_stereo(output_block, &block[..count], channels, first);
                    }
                    state.position.store(cursor.frame(), Ordering::Relaxed);
                    state.finished.store(cursor.finished(), Ordering::Relaxed);
                    state
                        .applied_request
                        .store(request.revision, Ordering::Release);
                },
                move |error| {
                    eprintln!("Error de salida de audio: {error}");
                    error_state.failed.store(true, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|error| error.to_string())
    }

    pub fn seek(&mut self, beat: f64) -> Result<(), String> {
        self.request.seek = beat_frame(beat, self.sample_rate, self.beats_per_second)?;
        self.request.generation = self.request.generation.wrapping_add(1);
        self.state.finished.store(false, Ordering::Relaxed);
        self.request.revision = self.request.revision.wrapping_add(1);
        self.state.request.publish(self.request);
        Ok(())
    }
    pub fn set_loop(&mut self, range: Option<(f64, f64)>) -> Result<(), String> {
        self.request.loop_range = loop_frames(range, self.sample_rate, self.beats_per_second)?;
        self.request.revision = self.request.revision.wrapping_add(1);
        self.state.request.publish(self.request);
        Ok(())
    }
    pub fn set_fx_rack(&self, track: TrackId, rack: Option<Arc<super::effects::Rack>>) {
        if let Some(t) = self.state.tracks.iter().find(|t| t.id == track) {
            t.fx.set_rack(rack, self.sample_rate);
        }
    }
    pub fn set_fx_gain(&self, track: TrackId, gain: f32) {
        if let Some(t) = self.state.tracks.iter().find(|t| t.id == track) {
            t.fx.set(gain);
        }
    }
    pub fn set_gain(&self, track: TrackId, gain: f32) {
        if let Some(t) = self.state.tracks.iter().find(|t| t.id == track) {
            t.gain
                .store(gain.clamp(0., 2.).to_bits(), Ordering::Relaxed);
        }
    }
    pub fn set_pan(&self, track: TrackId, pan: f32) {
        if let Some(t) = self.state.tracks.iter().find(|t| t.id == track) {
            t.pan.set(pan);
        }
    }
    pub fn beat(&self) -> f64 {
        self.state.position.load(Ordering::Relaxed) as f64 / f64::from(self.sample_rate)
            * self.beats_per_second
    }
    pub fn finished(&self) -> bool {
        self.state.applied_request.load(Ordering::Acquire) == self.request.revision
            && self.state.finished.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}
