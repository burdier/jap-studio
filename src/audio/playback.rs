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
use elevate::musical::{midi::MidiMessage, model::InstrumentId};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
pub(crate) use mix::{HiloBranchInput, MidiTrackInput, TrackInput};
use mix::{Mix, TrackControl};
use rtrb::{Producer, RingBuffer};
use std::sync::{
    Arc, Mutex,
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
    stream: Option<cpal::Stream>,
    vsts: Vec<Arc<super::vst3::SharedInstrument>>,
    live_midi: Mutex<Producer<(InstrumentId, MidiMessage)>>,
    instruments: Vec<InstrumentId>,
    all_off: Arc<AtomicBool>,
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
        vsts: Vec<Arc<super::vst3::SharedInstrument>>,
    ) -> Result<Self, String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, &audio)?;
        let mut mix = Mix::prepare(inputs, drums, setup.config.sample_rate, bpm, meter)?;
        for vst in vsts {
            if vst.rate != setup.config.sample_rate {
                return Err("La salida cambió de frecuencia; recarga el instrumento VST3".into());
            }
            if !mix.vsts.iter().any(|existing| Arc::ptr_eq(existing, &vst)) {
                mix.vsts.push(vst);
            }
        }
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
        let vsts = mix.vsts.clone();
        let instruments = mix.instrument_ids();
        let (live_midi, live_rx) = RingBuffer::new(512);
        let all_off = Arc::new(AtomicBool::new(false));
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
                live_rx,
                all_off.clone(),
                state.clone(),
            ),
            SampleFormat::I16 => Player::stream::<i16>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                live_rx,
                all_off.clone(),
                state.clone(),
            ),
            SampleFormat::I32 => Player::stream::<i32>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                live_rx,
                all_off.clone(),
                state.clone(),
            ),
            SampleFormat::U16 => Player::stream::<u16>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                live_rx,
                all_off.clone(),
                state.clone(),
            ),
            _ => return Err("Formato de salida no compatible".into()),
        }?;
        for (index, vst) in vsts.iter().enumerate() {
            if let Err(error) = vst.enter_arrangement() {
                for entered in &vsts[..index] {
                    entered.leave_arrangement();
                }
                return Err(error);
            }
        }
        if let Err(error) = stream.play() {
            drop(stream);
            for vst in &vsts {
                vst.leave_arrangement();
            }
            return Err(error.to_string());
        }
        Ok(Player {
            stream: Some(stream),
            vsts,
            live_midi: Mutex::new(live_midi),
            instruments,
            all_off,
            state,
            sample_rate: rate,
            beats_per_second,
            request,
        })
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Cerrar el callback común antes de devolver el procesador a la audición.
        drop(self.stream.take());
        for vst in &self.vsts {
            vst.leave_arrangement();
        }
    }
}

impl Player {
    pub fn has_instrument(&self, id: InstrumentId) -> bool {
        self.instruments.contains(&id)
    }
    pub fn send_live_midi(&self, id: InstrumentId, message: MidiMessage) -> bool {
        if !self.has_instrument(id) {
            return false;
        }
        let sent = self
            .live_midi
            .try_lock()
            .is_ok_and(|mut queue| queue.push((id, message)).is_ok());
        if !sent
            && matches!(
                message,
                MidiMessage::NoteOff { .. } | MidiMessage::AllNotesOff
            )
        {
            self.all_off.store(true, Ordering::Release);
        }
        sent
    }
    pub fn all_live_notes_off(&self) {
        self.all_off.store(true, Ordering::Release);
    }
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
        mut live_midi: rtrb::Consumer<(InstrumentId, MidiMessage)>,
        all_off: Arc<AtomicBool>,
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
                        for _ in 0..512 {
                            let Ok((id, message)) = live_midi.pop() else {
                                break;
                            };
                            mix.live_midi(id, message);
                        }
                        if all_off.swap(false, Ordering::AcqRel) {
                            mix.all_live_notes_off();
                        }
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
        self.state.failed.load(Ordering::Relaxed) || self.vsts.iter().any(|vst| vst.failed())
    }
}
