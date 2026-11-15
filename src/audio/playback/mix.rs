//! Suma por pista y master: recursos preparados y trabajo acotado por bloque.
use super::super::drums::{DrumInput, PreparedDrums};
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
    pub gain: AtomicU32,
    pub pan: StereoPan,
    pub fx: super::super::fx_gain::FxGain,
}
pub(super) struct PreparedTrack {
    sources: Vec<(ClipSource, f32)>,
    midi: Option<(PreparedMidiArrangement, PreparedArrangementVoice)>,
    pub control: Arc<TrackControl>,
    volume: Volume,
    fx: super::super::fx_gain::PreparedFxGain,
}
pub(super) struct Mix {
    pub tracks: Vec<PreparedTrack>,
    pub vsts: Vec<Arc<super::super::vst3::SharedInstrument>>,
    drums: Option<PreparedDrums>,
    pub end: u64,
    timing: TransportContext,
}
impl Mix {
    pub fn instrument_ids(&self) -> Vec<InstrumentId> {
        self.tracks
            .iter()
            .filter_map(|track| track.midi.as_ref().map(|(_, voice)| voice.id()))
            .collect()
    }

    pub fn live_midi(&mut self, id: InstrumentId, message: MidiMessage) {
        for track in &mut self.tracks {
            if let Some((_, voice)) = &mut track.midi
                && voice.id() == id
            {
                voice.live_midi(message);
                break;
            }
        }
    }
    pub fn all_live_notes_off(&mut self) {
        for track in &mut self.tracks {
            if let Some((_, voice)) = &mut track.midi {
                voice.live_midi(MidiMessage::AllNotesOff);
            }
        }
    }
    pub fn drum_control(&self) -> Option<Arc<AtomicU32>> {
        self.drums.as_ref().map(|drums| drums.control.clone())
    }
    pub fn prepare(
        inputs: Vec<TrackInput>,
        drums: Option<DrumInput>,
        rate: u32,
        bpm: f64,
        meter: Meter,
    ) -> Result<Self, String> {
        if (inputs.is_empty() && drums.is_none())
            || inputs.len() > 256
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
                gain: AtomicU32::new(input.gain.to_bits()),
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
                control,
                volume,
                fx,
            });
        }
        let drums = drums
            .map(|input| PreparedDrums::prepare(input, rate, bpm))
            .transpose()?;
        if drums.is_some() {
            end = u64::MAX;
        }
        Ok(Self {
            tracks,
            vsts,
            drums,
            end,
            timing,
        })
    }
    /// Sobrescribe sin limitar amplitud; protección solo al escribir el master físico.
    #[cfg(test)]
    pub fn render(&mut self, position: u64, output: &mut [StereoFrame]) {
        self.render_timed(position, output, None);
    }
    pub fn render_timed(
        &mut self,
        position: u64,
        output: &mut [StereoFrame],
        loop_range: Option<(u64, u64)>,
    ) {
        for (index, block) in output.chunks_mut(256).enumerate() {
            block.fill([0.; 2]);
            let at = position.saturating_add(index as u64 * 256);
            for track in &mut self.tracks {
                let mut bus = [[0.; 2]; 256];
                let mut scratch = [[0.; 2]; 256];
                for (source, gain) in &track.sources {
                    source.render(at, &mut scratch[..block.len()]);
                    for (sum, frame) in bus.iter_mut().zip(&scratch).take(block.len()) {
                        sum[0] += frame[0] * gain;
                        sum[1] += frame[1] * gain;
                    }
                }
                let time = TransportContext {
                    frame: at,
                    loop_range,
                    ..self.timing
                };
                if let Some((plan, voice)) = &mut track.midi {
                    voice.render_timed(plan, at, &mut scratch[..block.len()], Some(time));
                    for (sum, frame) in bus.iter_mut().zip(&scratch).take(block.len()) {
                        sum[0] += frame[0];
                        sum[1] += frame[1];
                    }
                }
                let gain = f32::from_bits(track.control.gain.load(Ordering::Relaxed));
                if track.volume.gain() != gain {
                    let _ = track.volume.set_gain(gain);
                }
                let bus = &mut bus[..block.len()];
                track.fx.process_timed(&track.control.fx, bus, Some(time));
                track.volume.process(bus);
                track.control.pan.apply(bus);
                for (sum, frame) in block.iter_mut().zip(bus) {
                    sum[0] += frame[0];
                    sum[1] += frame[1];
                }
            }
            if let Some(drums) = &mut self.drums {
                drums.render(at, block);
            }
        }
    }
}

#[cfg(test)]
mod tests;
