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
        arrangement_midi::PreparedMidiArrangement,
        clip::MidiClip,
        midi::MidiMessage,
        model::InstrumentId,
        time::Meter,
        track::{HiloMode, Track, TrackId},
        weave::WeaveRule,
    },
};
pub(crate) struct MidiTrackInput {
    pub index: usize,
    pub track: Track,
    pub instrument: InstrumentId,
    pub clips: Vec<MidiClip>,
    pub source: VoiceSource,
    pub hilo: Option<HiloBranchInput>,
}
pub(crate) struct HiloBranchInput {
    pub clips: Vec<MidiClip>,
    pub group_index: usize,
    pub branch_index: usize,
    pub instruments: Vec<InstrumentId>,
    pub mode: HiloMode,
    pub weave: Option<WeaveRule>,
}
pub(crate) struct TrackInput {
    pub id: TrackId,
    pub clips: Vec<AudioClip>,
    pub gain: f32,
    pub pan: f32,
    pub fx_gain: f32,
    pub rack: Option<Arc<super::super::effects::Rack>>,
    pub midi: Option<MidiTrackInput>,
}
pub(super) struct TrackControl {
    pub id: TrackId,
    pub instrument: Option<InstrumentId>,
    pub gain: AtomicU32,
    pub peak: AtomicU32,
    pub pan: StereoPan,
    pub fx: super::super::fx_gain::FxGain,
}
pub(super) struct PreparedTrack {
    sources: Vec<(ClipSource, f32)>,
    midi: Option<(PreparedMidiArrangement, PreparedArrangementVoice)>,
    retirement: Retirement,
    pub control: Arc<TrackControl>,
    volume: Volume,
    fx: super::super::fx_gain::PreparedFxGain,
}
pub(super) struct Mix {
    pub tracks: Vec<PreparedTrack>,
    pub vsts: Vec<Arc<super::super::vst3::SharedInstrument>>,
    drums: Option<PreparedDrums>,
    drum_io: Option<(LiveDrums, Producer<DrumPreview>)>,
    pub end: u64,
    timing: TransportContext,
    peak_decay: f32,
}
impl Mix {
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
                && let Some((_, voice)) = &mut track.midi
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
                && let Some((_, voice)) = &mut track.midi
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
                && let Some((_, voice)) = &mut track.midi
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
        let beats = timing.bpm / 60.;
        let spec = ProcessSpec::new(rate as f64).map_err(|e| e.to_string())?;
        let mut tracks = Vec::with_capacity(inputs.len());
        let mut vsts = Vec::new();
        let mut end = 0;
        for input in inputs {
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
            let control = Arc::new(TrackControl {
                id: input.id,
                instrument: input.midi.as_ref().map(|source| source.instrument),
                gain: AtomicU32::new(input.gain.to_bits()),
                peak: AtomicU32::new(0),
                pan: StereoPan::new(input.pan),
                fx: super::super::fx_gain::FxGain::new(input.fx_gain),
            });
            control.fx.set_rack(input.rack, rate);
            let mut sources = Vec::with_capacity(input.clips.len());
            for clip in input.clips {
                if clip.props.muted {
                    continue;
                }
                let converted = if rate == clip.file.sample_rate {
                    None
                } else {
                    Some(Arc::new(super::super::export::resample(
                        &clip.file.frames,
                        clip.file.sample_rate,
                        rate,
                    )?))
                };
                let source = ClipSource::new(&clip, converted, rate, beats);
                end = end.max(source.end_frame());
                sources.push((source, clip.props.gain_linear()));
            }
            let midi = input
                .midi
                .map(|source| {
                    let tail = match &source.source {
                        VoiceSource::Vst(shared) => shared.tail_frames,
                        _ => 0,
                    };
                    let mut plan = PreparedMidiArrangement::compile_track(
                        &source.clips,
                        &source.track,
                        source.index,
                        rate,
                        bpm,
                    )?;
                    if let Some(hilo) = &source.hilo {
                        plan.append(PreparedMidiArrangement::compile_hilo_branch(
                            &hilo.clips,
                            hilo.group_index,
                            &source.track,
                            hilo.branch_index,
                            &hilo.instruments,
                            hilo.mode,
                            hilo.weave.as_ref(),
                            rate,
                            bpm,
                        )?)?;
                    }
                    let voice =
                        PreparedArrangementVoice::prepare(source.instrument, source.source, rate)?;
                    if !source.clips.is_empty()
                        || source
                            .hilo
                            .as_ref()
                            .is_some_and(|hilo| !hilo.clips.is_empty())
                    {
                        end = end.max(plan.end_frame().saturating_add(tail));
                    }
                    Ok::<_, String>((plan, voice))
                })
                .transpose()?;
            let volume = Volume::with_gain(spec, input.gain).map_err(|e| e.to_string())?;
            let fx = control.fx.prepare();
            tracks.push(PreparedTrack {
                sources,
                midi,
                retirement: Retirement::default(),
                control,
                volume,
                fx,
            });
        }
        let (drums, drum_io) = match drums {
            Some(input) => {
                let (prepared, live, previews) = PreparedDrums::prepare(input, rate, bpm)?;
                (Some(prepared), Some((live, previews)))
            }
            None => (None, None),
        };
        Ok(Self {
            tracks,
            vsts,
            drums,
            drum_io,
            end,
            timing,
            peak_decay: 1.0 / (0.24 * rate as f32),
        })
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

    pub fn render_with_drums(
        &mut self,
        position: u64,
        output: &mut [StereoFrame],
        loop_range: Option<(u64, u64)>,
        drum_block: Option<&DrumBlock>,
    ) {
        for (index, block) in output.chunks_mut(256).enumerate() {
            block.fill([0.; 2]);
            let at = position.saturating_add(index as u64 * 256);
            for track in &mut self.tracks {
                let remaining = track.retirement.remaining();
                let frames = remaining.map_or(block.len(), |tail| block.len().min(tail as usize));
                if frames == 0 {
                    track.control.peak.store(0, Ordering::Relaxed);
                    continue;
                }
                let mut bus = [[0.; 2]; 256];
                let mut scratch = [[0.; 2]; 256];
                if remaining.is_none() {
                    for (source, gain) in &track.sources {
                        source.render(at, &mut scratch[..frames]);
                        for (sum, frame) in bus.iter_mut().zip(&scratch).take(frames) {
                            sum[0] += frame[0] * gain;
                            sum[1] += frame[1] * gain;
                        }
                    }
                }
                let time = TransportContext {
                    frame: at,
                    loop_range,
                    ..self.timing
                };
                if let Some((plan, voice)) = &mut track.midi {
                    if remaining.is_some() {
                        voice.render_tail_timed(&mut scratch[..frames], Some(time));
                    } else {
                        voice.render_timed(plan, at, &mut scratch[..frames], Some(time));
                    }
                    for (sum, frame) in bus.iter_mut().zip(&scratch).take(frames) {
                        sum[0] += frame[0];
                        sum[1] += frame[1];
                    }
                }
                let gain = f32::from_bits(track.control.gain.load(Ordering::Relaxed));
                if track.volume.gain() != gain {
                    let _ = track.volume.set_gain(gain);
                }
                let bus = &mut bus[..frames];
                track.fx.process_timed(&track.control.fx, bus, Some(time));
                track.volume.process(bus);
                track.control.pan.apply(bus);
                track.retirement.finish_block(bus);
                let mut peak = 0.0_f32;
                for (sum, frame) in block.iter_mut().zip(bus) {
                    sum[0] += frame[0];
                    sum[1] += frame[1];
                    peak = peak.max(frame[0].abs()).max(frame[1].abs());
                }
                let previous = f32::from_bits(track.control.peak.load(Ordering::Relaxed));
                let decay = (1.0 - self.peak_decay * frames as f32).max(0.0);
                track
                    .control
                    .peak
                    .store(peak.max(previous * decay).to_bits(), Ordering::Relaxed);
            }
            if let (Some(drums), Some(drum_block)) = (&mut self.drums, drum_block) {
                drums.render(at, block, drum_block);
            }
        }
    }
}

#[cfg(test)]
mod tests;
