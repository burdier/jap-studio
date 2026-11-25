//! Mezclador de clips: un stream y posición compartida en frames.
mod mix;
mod pool;
mod preplay;
mod score;
pub(crate) use preplay::apply_preplay;
mod transport;
use super::analyzer::{
    SpectrumAnalyzer,
    loudness::{Loudness, MasterTelemetry},
    tap::{Samples, Tap},
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ChannelMeter {
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub hold: f32,
}
use super::drums::{DrumBlock, DrumInput, DrumPreview, DrumScore, LiveDrums};
use super::metronome::{MetronomeControl, MetronomeSynth};
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
use elevate::musical::{drums::DrumKit, midi::MidiMessage, model::InstrumentId};
use elevate::{Primitive, ProcessSpec, primitives::amplitude::Volume};
use mix::TrackControl;
pub(crate) use mix::{HiloBranchInput, MidiTrackInput, TrackInput};
pub(crate) use mix::{Mix, Stem, StemBuffers};
use rtrb::{Producer, RingBuffer};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use transport::{Cursor, Request, beat_frame, loop_frames};

const MAX_TRACKS: usize = 256;

struct PlaybackState {
    delays: Arc<Publication<mix::MasterDelays>>,
    request: Publication<Request>,
    applied_request: AtomicU64,
    tracks: Vec<Arc<TrackControl>>,
    retire: Publication<[bool; MAX_TRACKS]>,
    drums: Option<Arc<Publication<DrumBlock>>>,
    metronome: Arc<MetronomeControl>,
    master_fx: super::fx_gain::FxGain,
    utility: UtilitySlot,
    master_tap: Arc<Tap>,
    position: AtomicU64,
    bpm: AtomicU64,
    counting_in: AtomicBool,
    finished: AtomicBool,
    stopping: AtomicBool,
    failed: AtomicBool,
}

pub(crate) struct Player {
    edits: score::Edits,
    master: MasterMeter,
    stream: Option<cpal::Stream>,
    vsts: Vec<Arc<super::vst3::SharedInstrument>>,
    live_midi: Mutex<Producer<(InstrumentId, MidiMessage)>>,
    all_off: Arc<AtomicBool>,
    state: Arc<PlaybackState>,
    track_indices: HashMap<TrackId, usize>,
    pub sample_rate: u32,
    beats_per_second: f64,
    request: Request,
    drums: Option<(LiveDrums, Producer<DrumPreview>)>,
}

struct MasterMeter {
    samples: rtrb::Consumer<Samples>,
    loudness: Loudness,
    spectrum: SpectrumAnalyzer,
    dropped: u64,
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
    pub fn update_drums(&mut self, kit: &DrumKit, score: Option<&DrumScore>) -> Result<(), String> {
        if let Some(drums) = self.mix.live_drums() {
            drums.edit(kit, score)?;
        }
        Ok(())
    }

    pub fn start(
        self,
        start_beat: f64,
        bpm: f64,
        meter: Meter,
        utility: UtilitySlot,
        master_fx_gain: f32,
        master_rack: Option<Arc<super::effects::Rack>>,
        loop_range: Option<(f64, f64)>,
        metronome: Arc<MetronomeControl>,
        count_in_bars: u32,
    ) -> Result<Player, String> {
        let Self {
            device,
            setup,
            mut mix,
        } = self;
        let vsts = mix.vsts.clone();
        let track_indices = mix
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.control.id, index))
            .collect();
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
        let drums = mix.take_drum_io();
        let (master_tap, master_samples) = Tap::new();
        master_tap.activate(true);
        let metro_synth = MetronomeSynth::new(rate, beats_per_second, meter, count_in_bars);
        let starts_counting_in = metro_synth.in_count_in();
        if !starts_counting_in {
            metronome.clear_count_in();
        }
        if master_rack
            .as_ref()
            .is_some_and(|rack| rack.rate != 0 && rack.rate != rate)
        {
            return Err(
                "La salida cambió de frecuencia; recarga los efectos VST3 del Master antes de reproducir"
                    .into(),
            );
        }
        let master_fx = super::fx_gain::FxGain::new(master_fx_gain);
        master_fx.set_rack(master_rack, rate);
        let state = Arc::new(PlaybackState {
            master_tap,
            delays: mix.delays.clone(),
            request: Publication::new(request),
            applied_request: AtomicU64::new(0),
            tracks: mix.tracks.iter().map(|t| t.control.clone()).collect(),
            retire: Publication::new([false; MAX_TRACKS]),
            drums: drums.as_ref().map(|(drums, _)| drums.state.clone()),
            metronome,
            master_fx,
            utility,
            position: AtomicU64::new(start),
            bpm: AtomicU64::new(bpm.to_bits()),
            counting_in: AtomicBool::new(starts_counting_in),
            finished: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let (edits, audio_edits) = score::Edits::new(mix.score.clone());
        let stream = match setup.format {
            SampleFormat::F32 => Player::stream::<f32>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                audio_edits,
                live_rx,
                all_off.clone(),
                state.clone(),
                metro_synth,
            ),
            SampleFormat::I16 => Player::stream::<i16>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                audio_edits,
                live_rx,
                all_off.clone(),
                state.clone(),
                metro_synth,
            ),
            SampleFormat::I32 => Player::stream::<i32>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                audio_edits,
                live_rx,
                all_off.clone(),
                state.clone(),
                metro_synth,
            ),
            SampleFormat::U16 => Player::stream::<u16>(
                &device,
                setup.config,
                setup.first_channel,
                mix,
                audio_edits,
                live_rx,
                all_off.clone(),
                state.clone(),
                metro_synth,
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
            edits,
            master: MasterMeter {
                samples: master_samples,
                loudness: Loudness::new(rate),
                spectrum: SpectrumAnalyzer::new(rate),
                dropped: 0,
            },
            stream: Some(stream),
            vsts,
            live_midi: Mutex::new(live_midi),
            all_off,
            state,
            track_indices,
            sample_rate: rate,
            beats_per_second,
            request,
            drums,
        })
    }
}

impl Player {
    pub fn poll_master_meter(&mut self, spectrum_visible: bool) {
        let dropped = self.state.master_tap.dropped.load(Ordering::Relaxed);
        if self.master.dropped != dropped {
            self.master.dropped = dropped;
            self.master.loudness = Loudness::new(self.sample_rate);
            self.master.spectrum.clear();
            for _ in 0..self.master.samples.slots() {
                let _ = self.master.samples.pop();
            }
            return;
        }
        for _ in 0..self.master.samples.slots() {
            let Ok(block) = self.master.samples.pop() else {
                break;
            };
            self.master.loudness.push_block(&block.frames[..block.len]);
            if spectrum_visible {
                for frame in &block.frames[..block.len] {
                    let clean = frame.map(|sample| {
                        if sample.is_finite() {
                            sample.clamp(-1., 1.)
                        } else {
                            0.
                        }
                    });
                    self.master.spectrum.push_stereo(clean);
                }
            }
        }
        if !spectrum_visible {
            self.master.spectrum.silence();
        }
    }

    pub fn master_loudness(&self) -> [Option<f64>; 2] {
        self.master.loudness.levels()
    }

    pub fn master_telemetry(&self) -> MasterTelemetry {
        self.master.loudness.telemetry()
    }

    pub fn reset_master_meter(&mut self) {
        self.master.loudness.reset();
    }

    pub fn master_spectrum(&self) -> &SpectrumAnalyzer {
        &self.master.spectrum
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
        self.state.retire.read(|retired| {
            self.state
                .tracks
                .iter()
                .zip(retired)
                .any(|(track, retired)| !*retired && track.instrument == Some(id))
        })
    }
    /// Publica en bloque las ramas retiradas. El audio conserva sus recursos hasta Stop.
    pub fn retire_tracks(&self, ids: &[TrackId]) {
        let mut retired = self.state.retire.read(|mask| *mask);
        for (track, removed) in self.state.tracks.iter().zip(&mut retired) {
            *removed |= ids.contains(&track.id);
        }
        self.state.retire.publish(retired);
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
    pub fn stop_with_tail(&self) {
        self.state.stopping.store(true, Ordering::Release);
    }
    pub fn update_drums(&mut self, kit: &DrumKit, score: Option<&DrumScore>) -> Result<(), String> {
        if let Some((drums, _)) = &mut self.drums {
            drums.edit(kit, score)?;
        }
        Ok(())
    }

    pub fn update_drum_kit(&mut self, kit: &DrumKit) {
        if let Some((drums, _)) = &mut self.drums {
            drums.edit_kit(kit);
        }
    }

    pub fn drum_bus_solo(&self) -> bool {
        self.state
            .drums
            .as_ref()
            .is_some_and(|state| state.read(|block| block.bus_solo))
    }

    pub fn has_drum_kit(&self) -> bool {
        self.drums.is_some()
    }

    pub fn queue_arrangement(&mut self, inputs: Vec<TrackInput>) {
        self.edits.queue(inputs);
    }

    pub fn poll_edits(&mut self) -> Result<(), String> {
        self.edits
            .poll(self.sample_rate, self.beats_per_second * 60.)?;
        self.drums
            .as_mut()
            .map_or(Ok(()), |(drums, _)| drums.poll())
    }

    pub fn preview_drum(&mut self, note: DrumPreview) -> bool {
        self.drums
            .as_mut()
            .is_some_and(|(_, previews)| previews.push(note).is_ok())
    }

    pub fn drum_piece_peak(&self, id: InstrumentId) -> f32 {
        self.drums
            .as_ref()
            .map_or(0.0, |(drums, _)| drums.peak(Some(id)))
    }

    pub fn drum_bus_peak(&self) -> f32 {
        self.drums
            .as_ref()
            .map_or(0.0, |(drums, _)| drums.peak(None))
    }

    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        first_channel: u16,
        mut mix: Mix,
        mut edits: score::AudioEdits,
        mut live_midi: rtrb::Consumer<(InstrumentId, MidiMessage)>,
        all_off: Arc<AtomicBool>,
        state: Arc<PlaybackState>,
        mut metro: MetronomeSynth,
    ) -> Result<cpal::Stream, String> {
        let channels = usize::from(config.channels);
        let first = usize::from(first_channel);
        let mut utility = state
            .utility
            .prepare(ProcessSpec::new(f64::from(config.sample_rate)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut master_fx = state.master_fx.prepare();
        let mut bpm = mix.timing_bpm();
        let meter = mix.timing_meter();
        let rate = config.sample_rate;
        let clip_end = mix.end;
        let mut cursor = Cursor::new(state.position.load(Ordering::Relaxed), clip_end);
        let mut stop_tail = None;
        let error_state = state.clone();
        device
            .build_output_stream(
                config,
                move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let live_bpm = f64::from_bits(state.bpm.load(Ordering::Relaxed));
                    if live_bpm.is_finite() && live_bpm > 0.0 && (live_bpm - bpm).abs() > 1e-6 {
                        bpm = live_bpm;
                        mix.set_timing_bpm(bpm);
                        metro.set_bpm(bpm, meter);
                    }
                    let request = state.request.read(|r| *r);
                    let mut block = [[0.0_f32; 2]; 256];
                    for output_block in output.chunks_mut(256 * channels) {
                        let count = output_block.len() / channels;
                        if state.stopping.load(Ordering::Acquire) {
                            let tail: &mut transport::StopTail =
                                stop_tail.get_or_insert_with(|| {
                                    let at = cursor.frame();
                                    let frames = mix
                                        .stop_transport(at)
                                        .saturating_add(state.master_fx.tail_frames());
                                    transport::StopTail::new(at, rate, frames)
                                });
                            let mut render = |drums: Option<&DrumBlock>| {
                                tail.render(
                                    &mut block[..count],
                                    mix.has_drum_one_shot_tail(),
                                    |at, frames| {
                                        mix.render_block(at, frames, None, drums, true, None);
                                        let ctx = super::vst3::TransportContext::new(
                                            at, rate, bpm, meter, None,
                                        )
                                        .ok()
                                        .map(|mut c| {
                                            c.playing = false;
                                            c
                                        });
                                        master_fx.process_timed(&state.master_fx, frames, ctx);
                                        utility.process(frames);
                                    },
                                );
                            };
                            if let Some(drums) = &state.drums {
                                drums.read(|drums| render(Some(drums)));
                            } else {
                                render(None);
                            }
                            state.master_tap.capture(&block[..count], rate);
                            super::output::write_stereo(
                                output_block,
                                &block[..count],
                                channels,
                                first,
                            );
                            continue;
                        }
                        edits.apply(&mut mix, cursor.frame());
                        let retire = state.retire.read(|mask| *mask);
                        let tail = mix.begin_retirements(&retire);
                        for _ in 0..512 {
                            let Ok((id, message)) = live_midi.pop() else {
                                break;
                            };
                            mix.live_midi(id, message);
                        }
                        if all_off.swap(false, Ordering::AcqRel) {
                            mix.all_live_notes_off();
                        }
                        let mut ci = 0usize;
                        if metro.in_count_in() {
                            block[..count].fill([0.0; 2]);
                            let mut click_buf = [[0.0_f32; 2]; 256];
                            ci = metro.render_count_in(&mut click_buf[..count], &state.metronome);
                            // Mantener audibles los instrumentos live durante la pre-cuenta sin disparar clips.
                            mix.render_block(
                                cursor.frame(),
                                &mut block[..ci],
                                None,
                                None,
                                true,
                                None,
                            );
                            utility.process(&mut block[..ci]);
                            for i in 0..ci {
                                block[i][0] = (block[i][0] + click_buf[i][0]).clamp(-1.0, 1.0);
                                block[i][1] = (block[i][1] + click_buf[i][1]).clamp(-1.0, 1.0);
                            }
                            state
                                .counting_in
                                .store(metro.in_count_in(), Ordering::Release);
                            if ci == count {
                                super::output::write_stereo(
                                    output_block,
                                    &block[..count],
                                    channels,
                                    first,
                                );
                                continue;
                            }
                        } else {
                            state.counting_in.store(false, Ordering::Release);
                        }
                        let rem = count - ci;
                        let keep_open =
                            state.metronome.is_enabled() || state.metronome.keep_running();
                        let mut metro_click_block = [[0.0_f32; 2]; 256];
                        let mut metro_offset = 0usize;
                        let block_start = cursor.frame();
                        let mut render = |drums: Option<&DrumBlock>| {
                            let end = if drums.is_some_and(DrumBlock::loops) || keep_open {
                                u64::MAX
                            } else {
                                let drum_end = drums.map_or(0, DrumBlock::arrangement_end);
                                let longest = mix.delays.read(|d| d.longest);
                                let end = mix
                                    .end
                                    .saturating_sub(mix.score.latency)
                                    .saturating_add(longest)
                                    .max(drum_end);
                                let block_end = cursor.frame().saturating_add(rem as u64);
                                // El fin del arreglo espera cualquier one-shot que siga sonando.
                                if drum_end > 0
                                    && request.loop_range.is_none()
                                    && block_end >= end
                                    && mix.has_drum_one_shot_tail()
                                {
                                    end.max(block_end.saturating_add(1))
                                } else {
                                    end
                                }
                            };
                            cursor.set_end(end);
                            cursor.render_with_tail(
                                request,
                                tail,
                                &mut block[ci..count],
                                |at, frames| {
                                    let len = frames.len();
                                    mix.render_with_drums(at, frames, request.loop_range, drums);
                                    metro.mix_segment(
                                        at,
                                        &mut metro_click_block[metro_offset..metro_offset + len],
                                        &state.metronome,
                                    );
                                    metro_offset += len;
                                },
                            );
                        };
                        if let Some(drums) = &state.drums {
                            drums.read(|drums| render(Some(drums)));
                        } else {
                            render(None);
                        }
                        let master_ctx = super::vst3::TransportContext::new(
                            block_start,
                            rate,
                            bpm,
                            meter,
                            request.loop_range,
                        )
                        .ok();
                        master_fx.process_timed(
                            &state.master_fx,
                            &mut block[ci..count],
                            master_ctx,
                        );
                        utility.process(&mut block[ci..count]);
                        state.master_tap.capture(&block[ci..count], rate);
                        // El clic se suma tras la captura del máster para no alterar LUFS ni espectro.
                        for i in 0..rem {
                            let c = metro_click_block[i];
                            if c[0] != 0.0 || c[1] != 0.0 {
                                block[ci + i][0] = (block[ci + i][0] + c[0]).clamp(-1.0, 1.0);
                                block[ci + i][1] = (block[ci + i][1] + c[1]).clamp(-1.0, 1.0);
                            }
                        }
                        super::output::write_stereo(output_block, &block[..count], channels, first);
                    }
                    state.position.store(cursor.frame(), Ordering::Relaxed);
                    state.finished.store(
                        stop_tail
                            .as_ref()
                            .map_or_else(|| cursor.finished(), transport::StopTail::finished),
                        Ordering::Relaxed,
                    );
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

    /// Cambia el BPM en tiempo real sin pausar la reproducción ni cerrar el stream.
    /// Conserva el beat musical actual, ajusta el bucle, el metrónomo, Drums y los FX Sync.
    pub fn set_bpm(&mut self, bpm: f64, loop_range: Option<(f64, f64)>) -> Result<(), String> {
        let new_bps = Tempo::new(bpm)
            .map_err(|error| error.to_string())?
            .beats_per_second();
        let current_beat = self.beat();
        self.beats_per_second = new_bps;
        self.state.bpm.store(bpm.to_bits(), Ordering::Relaxed);
        self.request.loop_range = loop_frames(loop_range, self.sample_rate, new_bps)?;
        self.request.seek = beat_frame(current_beat, self.sample_rate, new_bps)?;
        self.request.generation = self.request.generation.wrapping_add(1);
        self.request.revision = self.request.revision.wrapping_add(1);
        self.state.request.publish(self.request);
        if let Some((drums, _)) = &mut self.drums {
            drums.set_bpm(bpm)?;
        }
        Ok(())
    }
    pub fn set_loop(&mut self, range: Option<(f64, f64)>) -> Result<(), String> {
        self.request.loop_range = loop_frames(range, self.sample_rate, self.beats_per_second)?;
        self.request.revision = self.request.revision.wrapping_add(1);
        self.state.request.publish(self.request);
        Ok(())
    }
    fn track_control(&self, track: TrackId) -> Option<&TrackControl> {
        self.track_indices
            .get(&track)
            .map(|&index| self.state.tracks[index].as_ref())
    }

    /// Pico real del bus tras FX, fader y paneo, antes de sumar al master.
    pub fn track_peak(&self, track: TrackId) -> f32 {
        self.track_control(track).map_or(0.0, |control| {
            f32::from_bits(control.peak.load(Ordering::Relaxed))
        })
    }

    /// Medidor estéreo de precisión (Peak L/R, RMS L/R y Peak Hold) por pista.
    pub fn track_meter(&self, track: TrackId) -> ChannelMeter {
        self.track_control(track)
            .map_or_default(|control| ChannelMeter {
                peak: [
                    f32::from_bits(control.peak_l.load(Ordering::Relaxed)),
                    f32::from_bits(control.peak_r.load(Ordering::Relaxed)),
                ],
                rms: [
                    f32::from_bits(control.rms_l.load(Ordering::Relaxed)),
                    f32::from_bits(control.rms_r.load(Ordering::Relaxed)),
                ],
                hold: f32::from_bits(control.peak_hold.load(Ordering::Relaxed)),
            })
    }

    pub fn reset_track_hold(&self, track: TrackId) {
        if let Some(control) = self.track_control(track) {
            control.peak_hold.store(0, Ordering::Relaxed);
        }
    }

    pub fn set_fx_rack(&self, track: TrackId, rack: Option<Arc<super::effects::Rack>>) -> bool {
        if let Some(t) = self.track_control(track) {
            let latency = rack.as_ref().map_or(0, |r| r.latency_samples());
            t.fx.set_rack(rack, self.sample_rate);
            return t.rack_latency.swap(latency, Ordering::Relaxed) != latency;
        }
        false
    }

    pub fn refresh_track_delays(&self) -> Result<(), String> {
        let latencies: Vec<_> = self
            .state
            .tracks
            .iter()
            .map(|t| t.rack_latency.load(Ordering::Relaxed))
            .collect();
        let parents: Vec<_> = self.state.tracks.iter().map(|t| t.parent).collect();
        let plan = self
            .state
            .delays
            .read(|old| mix::MasterDelays::prepare(&latencies, &parents, Some(old)))?;
        self.state.delays.publish(plan);
        Ok(())
    }
    pub fn set_fx_gain(&self, track: TrackId, gain: f32) {
        if let Some(t) = self.track_control(track) {
            t.fx.set(gain);
        }
    }
    pub fn set_master_fx_rack(&self, rack: Option<Arc<super::effects::Rack>>) {
        self.state.master_fx.set_rack(rack, self.sample_rate);
    }
    pub fn set_master_fx_gain(&self, gain: f32) {
        self.state.master_fx.set(gain);
    }
    pub fn set_source_audible(&self, track: TrackId, audible: bool) {
        if let Some(t) = self.track_control(track) {
            t.source_audible.store(audible, Ordering::Relaxed);
        }
    }
    pub fn set_audible(&self, track: TrackId, audible: bool) {
        if let Some(t) = self.track_control(track) {
            t.audible.store(audible, Ordering::Relaxed);
        }
    }
    pub fn set_track_automation(
        &self,
        track: TrackId,
        automation: elevate::musical::track_automation::TrackAutomation,
    ) {
        if let Some(t) = self.track_control(track) {
            t.automation.publish(automation);
        }
    }
    pub fn set_track_sends(&self, track: TrackId, sends: &[(TrackId, f32)]) {
        let Some(control) = self.track_control(track) else {
            return;
        };
        if control.is_return {
            return;
        }
        let resolved: Vec<(usize, f32)> = sends
            .iter()
            .filter_map(|&(target, gain)| {
                if !gain.is_finite() || gain <= 0.0 || target == track {
                    return None;
                }
                let idx = *self.track_indices.get(&target)?;
                self.state
                    .tracks
                    .get(idx)
                    .filter(|dest| dest.is_return)
                    .map(|_| (idx, gain.clamp(0.0, 2.0)))
            })
            .collect();
        control.sends.publish(resolved);
    }
    pub fn set_gain(&self, track: TrackId, gain: f32) {
        if let Some(t) = self.track_control(track) {
            t.gain
                .store(gain.clamp(0., 2.).to_bits(), Ordering::Relaxed);
        }
    }
    pub fn set_pan(&self, track: TrackId, pan: f32) {
        if let Some(t) = self.track_control(track) {
            t.pan.set(pan);
        }
    }
    pub fn beat(&self) -> f64 {
        self.state.position.load(Ordering::Relaxed) as f64 / f64::from(self.sample_rate)
            * self.beats_per_second
    }
    pub fn is_counting_in(&self) -> bool {
        self.state.counting_in.load(Ordering::Acquire)
    }
    pub fn finished(&self) -> bool {
        self.state.applied_request.load(Ordering::Acquire) == self.request.revision
            && self.state.finished.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed) || self.vsts.iter().any(|vst| vst.failed())
    }
}
