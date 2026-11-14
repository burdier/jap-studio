//! Suma por pista y master: recursos preparados y trabajo acotado por bloque.
use super::super::drums::{DrumInput, PreparedDrums};
use super::super::piano::{PreparedArrangementVoice, VoiceSource};
use super::*;
use elevate::{
    StereoFrame,
    musical::{
        arrangement_midi::PreparedMidiArrangement,
        clip::MidiClip,
        model::InstrumentId,
        track::{Track, TrackId},
    },
};
pub(crate) struct MidiTrackInput {
    pub index: usize,
    pub track: Track,
    pub instrument: InstrumentId,
    pub clips: Vec<MidiClip>,
    pub source: VoiceSource,
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
    drums: Option<PreparedDrums>,
    pub end: u64,
}
impl Mix {
    pub fn drum_control(&self) -> Option<Arc<AtomicU32>> {
        self.drums.as_ref().map(|drums| drums.control.clone())
    }
    pub fn prepare(
        inputs: Vec<TrackInput>,
        drums: Option<DrumInput>,
        rate: u32,
        bpm: f64,
    ) -> Result<Self, String> {
        if (inputs.is_empty() && drums.is_none())
            || inputs.len() > 256
            || inputs.iter().map(|t| t.clips.len()).sum::<usize>() > 4096
        {
            return Err("Mezcla requiere una fuente, hasta 256 pistas y 4096 clips".into());
        }
        let beats = Tempo::new(bpm)
            .map_err(|e| e.to_string())?
            .beats_per_second();
        let spec = ProcessSpec::new(rate as f64).map_err(|e| e.to_string())?;
        let mut tracks = Vec::with_capacity(inputs.len());
        let mut end = 0;
        for input in inputs {
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
                    let plan = PreparedMidiArrangement::compile_track(
                        &source.clips,
                        &source.track,
                        source.index,
                        rate,
                        bpm,
                    )?;
                    let voice =
                        PreparedArrangementVoice::prepare(source.instrument, source.source, rate)?;
                    end = end.max(plan.end_frame());
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
        Ok(Self { tracks, drums, end })
    }
    /// Sobrescribe sin limitar amplitud; protección solo al escribir el master físico.
    pub fn render(&mut self, position: u64, output: &mut [StereoFrame]) {
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
                if let Some((plan, voice)) = &mut track.midi {
                    voice.render(plan, at, &mut scratch[..block.len()]);
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
                track.fx.process(&track.control.fx, bus);
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
