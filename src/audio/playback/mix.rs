//! Suma por pista y master: recursos preparados y trabajo acotado por bloque.
mod retirement;
use retirement::Retirement;

use super::super::drums::{DrumBlock, DrumInput, DrumPreview, LiveDrums, PreparedDrums};
use super::super::piano::{PreparedArrangementVoice, VoiceSource};
use super::super::vst3::TransportContext;
use super::*;
use elevate::{
    StereoFrame,
    musical::{
        clip::MidiClip,
        midi::MidiMessage,
        model::InstrumentId,
        time::Meter,
        track::{HiloMode, Track, TrackId},
        weave::WeaveRule,
    },
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Stem {
    Track(TrackId),
    Drum(InstrumentId),
}
pub(crate) type StemBuffers = std::collections::HashMap<Stem, [StereoFrame; 256]>;

#[derive(Clone)]
pub(crate) struct MidiTrackInput {
    pub index: usize,
    pub track: Track,
    pub instrument: InstrumentId,
    pub clips: Vec<MidiClip>,
    pub source: VoiceSource,
    pub hilo: Option<HiloBranchInput>,
}
#[derive(Clone)]
pub(crate) struct HiloBranchInput {
    pub clips: Vec<MidiClip>,
    pub group_index: usize,
    pub branch_index: usize,
    pub instruments: Vec<InstrumentId>,
    pub mode: HiloMode,
    pub weave: Option<WeaveRule>,
}
#[derive(Clone)]
pub(crate) struct TrackInput {
    pub id: TrackId,
    pub parent: Option<TrackId>,
    pub is_return: bool,
    pub sends: Vec<(TrackId, f32)>,
    pub source_audible: bool,
    pub audible: bool,
    pub clips: Vec<AudioClip>,
    pub gain: f32,
    pub pan: f32,
    pub fx_gain: f32,
    pub rack: Option<Arc<super::super::effects::Rack>>,
    pub midi: Option<MidiTrackInput>,
    pub automation: elevate::musical::track_automation::TrackAutomation,
}
pub(super) struct TrackControl {
    pub id: TrackId,
    pub parent: Option<usize>,
    pub is_return: bool,
    pub sends: Publication<Vec<(usize, f32)>>,
    pub source_audible: AtomicBool,
    pub audible: AtomicBool,
    pub rack_latency: AtomicU64,
    pub instrument: Option<InstrumentId>,
    pub gain: AtomicU32,
    pub peak: AtomicU32,
    pub peak_l: AtomicU32,
    pub peak_r: AtomicU32,
    pub rms_l: AtomicU32,
    pub rms_r: AtomicU32,
    pub peak_hold: AtomicU32,
    pub pan: StereoPan,
    pub fx: super::super::fx_gain::FxGain,
    pub automation: Publication<elevate::musical::track_automation::TrackAutomation>,
}
pub(super) struct PreparedTrack {
    midi: Option<PreparedArrangementVoice>,
    retirement: Retirement,
    pub control: Arc<TrackControl>,
    volume: Volume,
    fx: super::super::fx_gain::PreparedFxGain,
    bus: Box<[StereoFrame; 256]>,
    rendered_frames: usize,
}

#[derive(Clone, Copy)]
struct TrackRenderParams {
    at: u64,
    block_len: usize,
    tail: bool,
    drum_solo: bool,
    fade_length: u64,
    stop_fade: Option<(u64, u64)>,
    time: TransportContext,
    rate: u32,
    bpm: f64,
    peak_decay: f32,
}

impl PreparedTrack {
    fn render_into_bus(
        &mut self,
        index: usize,
        score: &score::Score,
        delays: &MasterDelays,
        group_input: Option<&[StereoFrame; 256]>,
        sidechain: Option<super::super::effects::SidechainInputs<'_>>,
        p: TrackRenderParams,
    ) {
        let remaining = self.retirement.remaining();
        let frames = remaining.map_or(p.block_len, |t| p.block_len.min(t as usize));
        self.rendered_frames = frames;
        if frames == 0 {
            self.control.peak.store(0, Ordering::Relaxed);
            self.control.peak_l.store(0, Ordering::Relaxed);
            self.control.peak_r.store(0, Ordering::Relaxed);
            self.control.rms_l.store(0, Ordering::Relaxed);
            self.control.rms_r.store(0, Ordering::Relaxed);
            return;
        }
        let bus = &mut self.bus[..frames];
        bus.fill([0.; 2]);
        let mut scratch = [[0.; 2]; 256];
        let lane = score.lanes.get(&self.control.id);
        if remaining.is_none()
            && (!p.tail || p.stop_fade.is_some())
            && let Some(lane) = lane
        {
            for (source, gain) in &lane.sources {
                if p.stop_fade
                    .is_some_and(|(stop, _)| source.start_frame() >= stop)
                {
                    continue;
                }
                source.render(p.at, &mut scratch[..frames]);
                for (sum, frame) in bus.iter_mut().zip(&scratch).take(frames) {
                    sum[0] += frame[0] * gain;
                    sum[1] += frame[1] * gain;
                }
            }
            if let Some((_, elapsed)) = p.stop_fade {
                for (offset, frame) in bus.iter_mut().enumerate() {
                    let weight = p.fade_length.saturating_sub(elapsed + offset as u64) as f32
                        / p.fade_length as f32;
                    *frame = frame.map(|sample| sample * weight);
                }
            }
        }
        let beat = p.time.frame as f64 / f64::from(p.rate) * (p.bpm / 60.0);
        if let Some(voice) = &mut self.midi {
            let mut inst_params = [(0u32, 0.0f64); 32];
            let param_count = self
                .control
                .automation
                .read(|auto| auto.collect_instrument_params(beat, &mut inst_params));
            let active_params = &inst_params[..param_count];
            if p.tail || remaining.is_some() {
                voice.render_tail_timed_params(&mut scratch[..frames], Some(p.time), active_params);
            } else if let Some(plan) = lane.and_then(|l| l.midi.as_ref()) {
                voice.render_timed_params(
                    plan,
                    p.at,
                    &mut scratch[..frames],
                    Some(p.time),
                    active_params,
                );
            }
            for (sum, frame) in bus.iter_mut().zip(&scratch).take(frames) {
                sum[0] += frame[0];
                sum[1] += frame[1];
            }
        }
        if !self.control.source_audible.load(Ordering::Relaxed) {
            bus.fill([0.; 2]);
        }
        let (input_samples, input_delay) = &delays.input[index];
        if *input_samples != 0 {
            input_delay.process(bus);
        }
        if let Some(group) = group_input {
            for (frame, child) in bus.iter_mut().zip(group.iter()) {
                frame[0] += child[0];
                frame[1] += child[1];
            }
        }
        let base_gain = f32::from_bits(self.control.gain.load(Ordering::Relaxed));
        let audible = self.control.audible.load(Ordering::Relaxed);
        let (gain, auto_pan) = self.control.automation.read(|auto| {
            let g = auto.evaluate_gain(beat, base_gain, audible);
            let pan = auto
                .pan
                .evaluate(beat)
                .map(elevate::musical::track_automation::AutomationParam::pan_to_position);
            self.fx.process_timed_automated_sidechain(
                &self.control.fx,
                bus,
                Some(p.time),
                Some((auto, beat)),
                sidechain,
            );
            (g, pan)
        });
        if self.volume.gain() != gain {
            let _ = self.volume.set_gain(gain);
        }
        if !self.control.is_return {
            let (samples, delay) = &delays.output[index];
            if *samples != 0 {
                delay.process(bus);
            }
        }
        self.volume.process(bus);
        if let Some(pan_pos) = auto_pan {
            StereoPan::apply_position(bus, pan_pos);
        } else {
            self.control.pan.apply(bus);
        }
        self.retirement.finish_block(bus);

        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        let mut sum_sq_l = 0.0_f32;
        let mut sum_sq_r = 0.0_f32;
        if !p.drum_solo {
            for frame in bus.iter() {
                let l = frame[0].abs();
                let r = frame[1].abs();
                peak_l = peak_l.max(l);
                peak_r = peak_r.max(r);
                sum_sq_l += l * l;
                sum_sq_r += r * r;
            }
        }
        let peak = peak_l.max(peak_r);
        let rms_l = (sum_sq_l / frames as f32).sqrt();
        let rms_r = (sum_sq_r / frames as f32).sqrt();
        let decay = (1.0 - p.peak_decay * frames as f32).max(0.0);
        let prev_peak = f32::from_bits(self.control.peak.load(Ordering::Relaxed));
        let prev_l = f32::from_bits(self.control.peak_l.load(Ordering::Relaxed));
        let prev_r = f32::from_bits(self.control.peak_r.load(Ordering::Relaxed));
        let prev_rms_l = f32::from_bits(self.control.rms_l.load(Ordering::Relaxed));
        let prev_rms_r = f32::from_bits(self.control.rms_r.load(Ordering::Relaxed));
        let prev_hold = f32::from_bits(self.control.peak_hold.load(Ordering::Relaxed));
        self.control
            .peak
            .store(peak.max(prev_peak * decay).to_bits(), Ordering::Relaxed);
        self.control
            .peak_l
            .store(peak_l.max(prev_l * decay).to_bits(), Ordering::Relaxed);
        self.control
            .peak_r
            .store(peak_r.max(prev_r * decay).to_bits(), Ordering::Relaxed);
        self.control
            .rms_l
            .store(rms_l.max(prev_rms_l * decay).to_bits(), Ordering::Relaxed);
        self.control
            .rms_r
            .store(rms_r.max(prev_rms_r * decay).to_bits(), Ordering::Relaxed);
        if peak > prev_hold {
            self.control
                .peak_hold
                .store(peak.to_bits(), Ordering::Relaxed);
        }
    }
}

/// Acumula los envíos post-fader de `track` hacia los buses Return, evaluando curvas de
/// automatización `Send(return_id)` sin asignaciones RT (incluso si el nivel estático es 0%).
fn accumulate_sends(
    track: &PreparedTrack,
    bus: &[StereoFrame],
    return_order: &[usize],
    tracks: &[PreparedTrack],
    return_audio: &mut [Option<Box<[StereoFrame; 256]>>],
    beat: f64,
) {
    if return_order.is_empty() {
        return;
    }
    track.control.sends.read(|sends| {
        track.control.automation.read(|auto| {
            for &dest in return_order {
                let dest_id = tracks[dest].control.id;
                let static_gain = sends
                    .iter()
                    .find(|&&(idx, _)| idx == dest)
                    .map_or(0.0, |&(_, g)| g);
                let send_gain = auto.evaluate_send(dest_id, beat, static_gain);
                if send_gain <= 0.0 {
                    continue;
                }
                if let Some(buf) = return_audio[dest].as_mut() {
                    for (dst, src) in buf.iter_mut().zip(bus.iter()) {
                        dst[0] += src[0] * send_gain;
                        dst[1] += src[1] * send_gain;
                    }
                }
            }
        });
    });
}

struct SyncTrackPtr(*mut PreparedTrack);
unsafe impl Sync for SyncTrackPtr {}
impl SyncTrackPtr {
    #[inline]
    unsafe fn get_mut(&self, index: usize) -> &mut PreparedTrack {
        unsafe { &mut *self.0.add(index) }
    }
}
pub(super) struct MasterDelays {
    pub longest: u64,
    input: Vec<DelayLine>,
    output: Vec<DelayLine>,
}

type DelayLine = (u64, Arc<crate::audio::effects::CompensationDelay>);

impl MasterDelays {
    pub fn prepare(
        latencies: &[u64],
        parents: &[Option<usize>],
        previous: Option<&Self>,
    ) -> Result<Self, String> {
        let (longest, input, output) = compensation(latencies, parents);
        if input.iter().chain(&output).copied().sum::<u64>() > 16 * 1024 * 1024 {
            return Err(
                "La compensación entre pistas supera la memoria disponible para retardos".into(),
            );
        }
        fn lines(latencies: Vec<u64>, previous: Option<&[DelayLine]>) -> Vec<DelayLine> {
            latencies
                .into_iter()
                .enumerate()
                .map(|(i, samples)| {
                    let line = previous
                        .and_then(|old| old.get(i))
                        .filter(|(n, _)| *n == samples)
                        .map(|(_, line)| line.clone())
                        .unwrap_or_else(|| {
                            Arc::new(crate::audio::effects::CompensationDelay::new(
                                samples as usize,
                            ))
                        });
                    (samples, line)
                })
                .collect()
        }
        Ok(Self {
            longest,
            input: lines(input, previous.map(|p| p.input.as_slice())),
            output: lines(output, previous.map(|p| p.output.as_slice())),
        })
    }
}

/// Alinea las pistas hijas en la entrada de su grupo y los grupos en el máster.
pub(super) fn compensation(
    latencies: &[u64],
    parents: &[Option<usize>],
) -> (u64, Vec<u64>, Vec<u64>) {
    let mut input = vec![0; latencies.len()];
    for (&latency, parent) in latencies.iter().zip(parents) {
        if let Some(parent) = *parent {
            input[parent] = input[parent].max(latency);
        }
    }
    let longest = parents
        .iter()
        .enumerate()
        .filter(|(_, parent)| parent.is_none())
        .map(|(i, _)| input[i].saturating_add(latencies[i]))
        .max()
        .unwrap_or(0);
    let output = parents
        .iter()
        .enumerate()
        .map(|(i, parent)| {
            parent.map_or_else(
                || {
                    longest
                        .saturating_sub(input[i])
                        .saturating_sub(latencies[i])
                },
                |p| input[p] - latencies[i],
            )
        })
        .collect();
    (longest, input, output)
}

pub(super) fn track_parents(inputs: &[TrackInput]) -> Result<Vec<Option<usize>>, String> {
    inputs
        .iter()
        .map(|track| {
            track
                .parent
                .map(|parent| {
                    inputs
                        .iter()
                        .position(|candidate| candidate.id == parent && candidate.parent.is_none())
                        .ok_or_else(|| "Falta el bus de una pista".to_owned())
                })
                .transpose()
        })
        .collect()
}
pub(crate) struct Mix {
    pub(super) delays: Arc<Publication<MasterDelays>>,
    pub(super) tracks: Vec<PreparedTrack>,
    pub vsts: Vec<Arc<super::super::vst3::SharedInstrument>>,
    leaf_order: Vec<usize>,
    leaf_parallel: Vec<usize>,
    leaf_serial: Vec<usize>,
    group_order: Vec<usize>,
    return_order: Vec<usize>,
    pool: Option<super::pool::AudioPool>,
    group_audio: Vec<Option<Box<[StereoFrame; 256]>>>,
    return_audio: Vec<Option<Box<[StereoFrame; 256]>>>,
    sidechain_buses: Vec<(TrackId, Box<[StereoFrame; 256]>)>,
    drum_bus: Box<[StereoFrame; 256]>,
    drums: Option<PreparedDrums>,
    drum_io: Option<(LiveDrums, Producer<DrumPreview>)>,
    pub end: u64,
    pub(super) score: Arc<score::Score>,
    timing: TransportContext,
    stopped_at: Option<u64>,
    peak_decay: f32,
}
impl TrackInput {
    pub(super) fn rack_latency(&self) -> u64 {
        self.rack.as_ref().map_or(0, |rack| rack.latency_samples())
    }
}
impl Mix {
    pub(super) fn timing_bpm(&self) -> f64 {
        self.timing.bpm
    }
    pub(super) fn set_timing_bpm(&mut self, bpm: f64) {
        if bpm.is_finite() && bpm > 0.0 {
            self.timing.bpm = bpm;
        }
    }
    pub(super) fn timing_meter(&self) -> Meter {
        self.timing.meter
    }
    #[cfg(test)]
    pub fn disable_pool(&mut self) {
        self.pool = None;
    }
    #[cfg(test)]
    pub fn instrument_ids(&self) -> Vec<InstrumentId> {
        self.tracks
            .iter()
            .filter_map(|track| track.control.instrument)
            .collect()
    }

    pub fn begin_retirements(&mut self, retire: &[bool; MAX_TRACKS]) -> u64 {
        let mut longest = 0;
        for (track, &retire) in self.tracks.iter_mut().zip(retire) {
            if retire
                && track.retirement.begin(self.timing.rate)
                && let Some(voice) = &mut track.midi
            {
                voice.finish_input();
            }
            longest = longest.max(track.retirement.remaining().unwrap_or(0));
        }
        longest
    }

    pub fn live_midi(&mut self, id: InstrumentId, message: MidiMessage) {
        for track in &mut self.tracks {
            if track.retirement.remaining().is_none()
                && let Some(voice) = &mut track.midi
                && voice.id() == id
            {
                voice.live_midi(message);
                break;
            }
        }
    }
    pub fn all_live_notes_off(&mut self) {
        for track in &mut self.tracks {
            if track.retirement.remaining().is_none()
                && let Some(voice) = &mut track.midi
            {
                voice.live_midi(MidiMessage::AllNotesOff);
            }
        }
    }
    pub fn take_drum_io(&mut self) -> Option<(LiveDrums, Producer<DrumPreview>)> {
        self.drum_io.take()
    }

    pub fn live_drums(&mut self) -> Option<&mut LiveDrums> {
        self.drum_io.as_mut().map(|(drums, _)| drums)
    }
    pub fn prepare(
        inputs: Vec<TrackInput>,
        drums: Option<DrumInput>,
        rate: u32,
        bpm: f64,
        meter: Meter,
    ) -> Result<Self, String> {
        if (inputs.is_empty() && drums.is_none())
            || inputs.len() > MAX_TRACKS
            || inputs.iter().map(|t| t.clips.len()).sum::<usize>() > 4096
        {
            return Err("Mezcla requiere una fuente, hasta 256 pistas y 4096 clips".into());
        }
        let timing = TransportContext::new(0, rate, bpm, meter, None)?;
        let score = Arc::new(score::Score::prepare(&inputs, rate, bpm, None)?);
        let spec = ProcessSpec::new(rate as f64).map_err(|e| e.to_string())?;
        let mut tracks = Vec::with_capacity(inputs.len());
        let mut vsts = Vec::new();
        let end = score.end;
        let latencies: Vec<_> = inputs.iter().map(TrackInput::rack_latency).collect();
        let parents = track_parents(&inputs)?;
        let delays = Arc::new(Publication::new(MasterDelays::prepare(
            &latencies, &parents, None,
        )?));
        let mut group_audio = vec![None; inputs.len()];
        for &parent in parents.iter().flatten() {
            group_audio[parent].get_or_insert_with(|| Box::new([[0.; 2]; 256]));
        }
        let id_to_index: std::collections::HashMap<TrackId, usize> = inputs
            .iter()
            .enumerate()
            .map(|(index, track)| (track.id, index))
            .collect();
        let is_return_flags: Vec<bool> = inputs.iter().map(|track| track.is_return).collect();
        let mut return_audio = vec![None; inputs.len()];
        let mut leaf_order = Vec::with_capacity(inputs.len());
        let mut group_order = Vec::new();
        let mut return_order = Vec::new();
        for index in 0..inputs.len() {
            if is_return_flags[index] {
                return_audio[index] = Some(Box::new([[0.; 2]; 256]));
                return_order.push(index);
            } else if group_audio[index].is_some() {
                group_order.push(index);
            } else {
                leaf_order.push(index);
            }
        }
        for ((input, latency), parent) in inputs.into_iter().zip(latencies).zip(parents) {
            if let Some(source) = &input.midi
                && let VoiceSource::Vst(shared) = &source.source
            {
                if vsts.iter().any(|other| Arc::ptr_eq(other, shared)) {
                    return Err("Una instancia VST3 no puede alimentar dos pistas a la vez".into());
                }
                vsts.push(shared.clone());
            }
            if input
                .rack
                .as_ref()
                .is_some_and(|rack| rack.rate != 0 && rack.rate != rate)
            {
                return Err(
                    "La salida cambió de frecuencia; recarga los efectos VST3 antes de reproducir"
                        .into(),
                );
            }
            let resolved_sends: Vec<(usize, f32)> = if input.is_return {
                Vec::new()
            } else {
                input
                    .sends
                    .iter()
                    .filter_map(|&(target, gain)| {
                        if !gain.is_finite() || gain <= 0.0 || target == input.id {
                            return None;
                        }
                        let &dest = id_to_index.get(&target)?;
                        is_return_flags
                            .get(dest)
                            .copied()
                            .filter(|&is_ret| is_ret)
                            .map(|_| (dest, gain.clamp(0.0, 2.0)))
                    })
                    .collect()
            };
            let control = Arc::new(TrackControl {
                id: input.id,
                parent,
                is_return: input.is_return,
                sends: Publication::new(resolved_sends),
                source_audible: AtomicBool::new(input.source_audible),
                audible: AtomicBool::new(input.audible),
                rack_latency: AtomicU64::new(latency),
                instrument: input.midi.as_ref().map(|source| source.instrument),
                gain: AtomicU32::new(input.gain.to_bits()),
                peak: AtomicU32::new(0),
                peak_l: AtomicU32::new(0),
                peak_r: AtomicU32::new(0),
                rms_l: AtomicU32::new(0),
                rms_r: AtomicU32::new(0),
                peak_hold: AtomicU32::new(0),
                pan: StereoPan::new(input.pan),
                fx: super::super::fx_gain::FxGain::new(input.fx_gain),
                automation: Publication::new(input.automation),
            });
            control.fx.set_rack(input.rack, rate);
            let midi = input
                .midi
                .map(|source| {
                    PreparedArrangementVoice::prepare(source.instrument, source.source, rate)
                })
                .transpose()?;
            let volume = Volume::with_gain(spec, input.gain).map_err(|e| e.to_string())?;
            let fx = control.fx.prepare();
            tracks.push(PreparedTrack {
                midi,
                retirement: Retirement::default(),
                control,
                volume,
                fx,
                bus: Box::new([[0.; 2]; 256]),
                rendered_frames: 0,
            });
        }
        let mut leaf_parallel = Vec::with_capacity(leaf_order.len());
        let mut leaf_serial = Vec::new();
        for &index in &leaf_order {
            if tracks[index]
                .midi
                .as_ref()
                .is_none_or(PreparedArrangementVoice::parallel_safe)
            {
                leaf_parallel.push(index);
            } else {
                leaf_serial.push(index);
            }
        }
        let pool = super::pool::AudioPool::new(
            leaf_parallel
                .len()
                .max(group_order.len())
                .max(return_order.len()),
        );
        let (drums, drum_io) = match drums {
            Some(input) => {
                let (prepared, live, previews) = PreparedDrums::prepare(input, rate, bpm)?;
                (Some(prepared), Some((live, previews)))
            }
            None => (None, None),
        };
        let sidechain_buses: Vec<(TrackId, Box<[StereoFrame; 256]>)> = tracks
            .iter()
            .map(|t| (t.control.id, Box::new([[0.; 2]; 256])))
            .collect();
        let drum_bus = Box::new([[0.; 2]; 256]);
        Ok(Self {
            delays,
            tracks,
            vsts,
            leaf_order,
            leaf_parallel,
            leaf_serial,
            group_order,
            return_order,
            pool,
            group_audio,
            return_audio,
            sidechain_buses,
            drum_bus,
            drums,
            drum_io,
            end,
            score,
            timing,
            stopped_at: None,
            peak_decay: 1.0 / (0.24 * rate as f32),
        })
    }
    pub(super) fn replace_score(&mut self, next: Arc<score::Score>, at: u64) -> Arc<score::Score> {
        for track in &mut self.tracks {
            if track.retirement.remaining().is_some() {
                continue;
            }
            let id = track.control.id;
            if let (Some(voice), Some(lane)) = (&mut track.midi, next.lanes.get(&id)) {
                for release in [true, false] {
                    let plan = if release {
                        self.score.lanes.get(&id).and_then(|l| l.midi.as_ref())
                    } else {
                        lane.midi.as_ref()
                    };
                    for change in &lane.changes {
                        if let Some(event) = change.event(at, release) {
                            voice.change_note(
                                event,
                                plan.map_or(0, |p| p.conventional_channels(event.instrument)),
                            );
                        }
                    }
                }
            }
        }
        // La edición puede acortar el último clip mientras el cursor lo recorre.
        // Conserva el tramo que ya se estaba reproduciendo hasta su fin natural.
        self.end = self.end.max(next.end);
        std::mem::replace(&mut self.score, next)
    }

    /// Sobrescribe sin limitar amplitud; protección solo al escribir el master físico.
    #[cfg(test)]
    pub fn render(&mut self, position: u64, output: &mut [StereoFrame]) {
        self.render_timed(position, output, None);
    }
    #[cfg(test)]
    pub fn render_timed(
        &mut self,
        position: u64,
        output: &mut [StereoFrame],
        loop_range: Option<(u64, u64)>,
    ) {
        if let Some(state) = self.drum_io.as_ref().map(|(live, _)| live.state.clone()) {
            state.read(|block| self.render_with_drums(position, output, loop_range, Some(block)));
        } else {
            self.render_with_drums(position, output, loop_range, None);
        }
    }

    pub fn has_drum_one_shot_tail(&self) -> bool {
        self.drums
            .as_ref()
            .is_some_and(PreparedDrums::has_one_shot_tail)
    }

    pub fn finish_input(&mut self) {
        for track in &mut self.tracks {
            if let Some(voice) = &mut track.midi {
                voice.finish_input();
            }
        }
        if let Some(drums) = &mut self.drums {
            drums.finish_input();
        }
    }

    pub(super) fn stop_transport(&mut self, at: u64) -> u64 {
        self.stopped_at = Some(at);
        self.finish_input();
        let instrument_tail = self
            .vsts
            .iter()
            .map(|vst| vst.tail_frames)
            .max()
            .unwrap_or(0);
        let fx_tail = self
            .tracks
            .iter()
            .map(|track| {
                track.control.fx.tail_frames().saturating_add(
                    track
                        .control
                        .parent
                        .map_or(0, |parent| self.tracks[parent].control.fx.tail_frames()),
                )
            })
            .max()
            .unwrap_or(0);
        let return_tail = self
            .return_order
            .iter()
            .map(|&index| self.tracks[index].control.fx.tail_frames())
            .max()
            .unwrap_or(0);
        instrument_tail
            .saturating_add(fx_tail)
            .saturating_add(return_tail)
            .saturating_add(self.delays.read(|delays| delays.longest))
    }

    pub fn render_with_drums(
        &mut self,
        position: u64,
        output: &mut [StereoFrame],
        loop_range: Option<(u64, u64)>,
        drum_block: Option<&DrumBlock>,
    ) {
        for (index, block) in output.chunks_mut(256).enumerate() {
            self.render_block(
                position.saturating_add(index as u64 * 256),
                block,
                loop_range,
                drum_block,
                false,
                None,
            );
        }
    }

    pub fn render_block(
        &mut self,
        at: u64,
        block: &mut [StereoFrame],
        loop_range: Option<(u64, u64)>,
        drum_block: Option<&DrumBlock>,
        tail: bool,
        mut taps: Option<&mut StemBuffers>,
    ) {
        debug_assert!(block.len() <= 256);
        block.fill([0.; 2]);
        let drum_solo = drum_block.is_some_and(|state| state.bus_solo);
        let fade_length = u64::from((self.timing.rate / 200).max(1));
        let stop_fade = self.stopped_at.and_then(|stop| {
            let elapsed = at.saturating_sub(stop);
            (elapsed < fade_length).then_some((stop, elapsed))
        });
        let time = TransportContext {
            frame: self.stopped_at.unwrap_or(at),
            playing: self.stopped_at.is_none(),
            loop_range,
            ..self.timing
        };
        for group in self.group_audio.iter_mut().flatten() {
            group[..block.len()].fill([0.; 2]);
        }
        for ret in self.return_audio.iter_mut().flatten() {
            ret[..block.len()].fill([0.; 2]);
        }
        let params = TrackRenderParams {
            at,
            block_len: block.len(),
            tail,
            drum_solo,
            fade_length,
            stop_fade,
            time,
            rate: self.timing.rate,
            bpm: self.timing.bpm,
            peak_decay: self.peak_decay,
        };
        let len = block.len();
        self.drum_bus[..len].fill([0.; 2]);
        let has_drums = if let (Some(drums), Some(drum_block)) = (&mut self.drums, drum_block) {
            drums.render_block(
                at,
                &mut self.drum_bus[..len],
                drum_block,
                tail,
                taps.as_deref_mut(),
            );
            true
        } else {
            false
        };
        let drum_sc: Option<&[StereoFrame]> = has_drums.then_some(&self.drum_bus[..len]);
        let score = &self.score;
        let tracks = &mut self.tracks;
        let leaf_order = &self.leaf_order;
        let leaf_parallel = &self.leaf_parallel;
        let leaf_serial = &self.leaf_serial;
        let group_order = &self.group_order;
        let return_order = &self.return_order;
        let group_audio = &mut self.group_audio;
        let return_audio = &mut self.return_audio;
        let sidechain_buses = &mut self.sidechain_buses;
        let pool = self.pool.as_ref();

        let copy_bus =
            |index: usize,
             tracks: &[PreparedTrack],
             sidechain_buses: &mut [(TrackId, Box<[StereoFrame; 256]>)]| {
                let frames = tracks[index].rendered_frames;
                if frames > 0 {
                    sidechain_buses[index].1[..frames]
                        .copy_from_slice(&tracks[index].bus[..frames]);
                } else {
                    sidechain_buses[index].1[..len].fill([0.; 2]);
                }
            };

        self.delays.read(|delays| {
            // Fase 1a: pistas hoja seguras para paralelo; las que NO consumen sidechain de
            // otra pista se renderizan primero para entregar su bloque actual con latencia 0.
            let mut leaf_sources = [0usize; MAX_TRACKS];
            let mut src_count = 0usize;
            let mut leaf_consumers = [0usize; MAX_TRACKS];
            let mut cons_count = 0usize;
            for &index in leaf_parallel {
                if tracks[index].control.fx.has_track_sidechain() {
                    leaf_consumers[cons_count] = index;
                    cons_count += 1;
                } else {
                    leaf_sources[src_count] = index;
                    src_count += 1;
                }
            }
            let src_slice = &leaf_sources[..src_count];
            let cons_slice = &leaf_consumers[..cons_count];
            {
                let sc = super::super::effects::SidechainInputs {
                    drums: drum_sc,
                    tracks: sidechain_buses.as_slice(),
                    offset: 0,
                    len,
                };
                if src_slice.len() >= 2
                    && let Some(pool) = pool
                {
                    let ptr = SyncTrackPtr(tracks.as_mut_ptr());
                    pool.execute(src_slice.len(), |slot| {
                        let index = src_slice[slot];
                        // SAFETY: `src_slice` contiene índices únicos < `tracks.len()`.
                        let track = unsafe { ptr.get_mut(index) };
                        track.render_into_bus(index, score, delays, None, Some(sc), params);
                    });
                } else {
                    for &index in src_slice {
                        tracks[index].render_into_bus(index, score, delays, None, Some(sc), params);
                    }
                }
            }
            for &index in src_slice {
                copy_bus(index, tracks, sidechain_buses);
            }
            if !cons_slice.is_empty() {
                let sc = super::super::effects::SidechainInputs {
                    drums: drum_sc,
                    tracks: sidechain_buses.as_slice(),
                    offset: 0,
                    len,
                };
                if cons_slice.len() >= 2
                    && let Some(pool) = pool
                {
                    let ptr = SyncTrackPtr(tracks.as_mut_ptr());
                    pool.execute(cons_slice.len(), |slot| {
                        let index = cons_slice[slot];
                        // SAFETY: `cons_slice` contiene índices únicos < `tracks.len()`.
                        let track = unsafe { ptr.get_mut(index) };
                        track.render_into_bus(index, score, delays, None, Some(sc), params);
                    });
                } else {
                    for &index in cons_slice {
                        tracks[index].render_into_bus(index, score, delays, None, Some(sc), params);
                    }
                }
                for &index in cons_slice {
                    copy_bus(index, tracks, sidechain_buses);
                }
            }
            // Fase 1b: pistas hoja con motores C con estado compartido global (ej. FluidSynth).
            for &index in leaf_serial {
                let sc = super::super::effects::SidechainInputs {
                    drums: drum_sc,
                    tracks: sidechain_buses.as_slice(),
                    offset: 0,
                    len,
                };
                tracks[index].render_into_bus(index, score, delays, None, Some(sc), params);
                copy_bus(index, tracks, sidechain_buses);
            }
            let block_beat =
                params.time.frame as f64 / f64::from(params.rate) * (params.bpm / 60.0);
            // Suma determinista de pistas hoja a su grupo o al máster en orden fijo.
            for &index in leaf_order {
                let track = &tracks[index];
                let frames = track.rendered_frames;
                if frames == 0 {
                    continue;
                }
                let bus = &track.bus[..frames];
                if !drum_solo {
                    let output = track.control.parent.map_or(&mut *block, |parent| {
                        group_audio[parent].as_mut().unwrap().as_mut_slice()
                    });
                    for (sum, frame) in output.iter_mut().zip(bus.iter()) {
                        sum[0] += frame[0];
                        sum[1] += frame[1];
                    }
                    accumulate_sends(track, bus, return_order, tracks, return_audio, block_beat);
                }
                if let Some(stem) = taps
                    .as_deref_mut()
                    .and_then(|s| s.get_mut(&Stem::Track(track.control.id)))
                {
                    if drum_solo {
                        stem[..frames].fill([0.; 2]);
                    } else {
                        stem[..frames].copy_from_slice(bus);
                    }
                }
            }

            // Fase 2: buses de grupo (ya recibieron todas sus pistas hijas).
            {
                let sc = super::super::effects::SidechainInputs {
                    drums: drum_sc,
                    tracks: sidechain_buses.as_slice(),
                    offset: 0,
                    len,
                };
                if group_order.len() >= 2
                    && let Some(pool) = pool
                {
                    let ptr = SyncTrackPtr(tracks.as_mut_ptr());
                    let group_in: &[Option<Box<[StereoFrame; 256]>>] = group_audio.as_slice();
                    pool.execute(group_order.len(), |slot| {
                        let index = group_order[slot];
                        // SAFETY: `group_order` contiene índices únicos < `tracks.len()`.
                        let track = unsafe { ptr.get_mut(index) };
                        track.render_into_bus(
                            index,
                            score,
                            delays,
                            group_in[index].as_deref(),
                            Some(sc),
                            params,
                        );
                    });
                } else {
                    for &index in group_order {
                        tracks[index].render_into_bus(
                            index,
                            score,
                            delays,
                            group_audio[index].as_deref(),
                            Some(sc),
                            params,
                        );
                    }
                }
            }
            for &index in group_order {
                copy_bus(index, tracks, sidechain_buses);
            }
            // Suma determinista de grupos al máster en orden fijo.
            for &index in group_order {
                let track = &tracks[index];
                let frames = track.rendered_frames;
                if frames == 0 {
                    continue;
                }
                let bus = &track.bus[..frames];
                if !drum_solo {
                    for (sum, frame) in block.iter_mut().zip(bus.iter()) {
                        sum[0] += frame[0];
                        sum[1] += frame[1];
                    }
                    accumulate_sends(track, bus, return_order, tracks, return_audio, block_beat);
                }
                if let Some(stem) = taps
                    .as_deref_mut()
                    .and_then(|s| s.get_mut(&Stem::Track(track.control.id)))
                {
                    if drum_solo {
                        stem[..frames].fill([0.; 2]);
                    } else {
                        stem[..frames].copy_from_slice(bus);
                    }
                }
            }

            // Fase 3: buses Return / Aux (reciben los envíos post-fader de pistas y grupos).
            {
                let sc = super::super::effects::SidechainInputs {
                    drums: drum_sc,
                    tracks: sidechain_buses.as_slice(),
                    offset: 0,
                    len,
                };
                if return_order.len() >= 2
                    && let Some(pool) = pool
                {
                    let ptr = SyncTrackPtr(tracks.as_mut_ptr());
                    let ret_in: &[Option<Box<[StereoFrame; 256]>>] = return_audio.as_slice();
                    pool.execute(return_order.len(), |slot| {
                        let index = return_order[slot];
                        // SAFETY: `return_order` contiene índices únicos < `tracks.len()`.
                        let track = unsafe { ptr.get_mut(index) };
                        track.render_into_bus(
                            index,
                            score,
                            delays,
                            ret_in[index].as_deref(),
                            Some(sc),
                            params,
                        );
                    });
                } else {
                    for &index in return_order {
                        tracks[index].render_into_bus(
                            index,
                            score,
                            delays,
                            return_audio[index].as_deref(),
                            Some(sc),
                            params,
                        );
                    }
                }
            }
            for &index in return_order {
                copy_bus(index, tracks, sidechain_buses);
            }
            // Suma determinista de buses Return al máster en orden fijo.
            for &index in return_order {
                let track = &tracks[index];
                let frames = track.rendered_frames;
                if frames == 0 {
                    continue;
                }
                let bus = &track.bus[..frames];
                if !drum_solo {
                    for (sum, frame) in block.iter_mut().zip(bus.iter()) {
                        sum[0] += frame[0];
                        sum[1] += frame[1];
                    }
                }
                if let Some(stem) = taps
                    .as_deref_mut()
                    .and_then(|s| s.get_mut(&Stem::Track(track.control.id)))
                {
                    if drum_solo {
                        stem[..frames].fill([0.; 2]);
                    } else {
                        stem[..frames].copy_from_slice(bus);
                    }
                }
            }
        });
        if has_drums {
            for (sum, frame) in block.iter_mut().zip(self.drum_bus[..len].iter()) {
                sum[0] += frame[0];
                sum[1] += frame[1];
            }
        }
    }
}

#[cfg(test)]
mod tests;
