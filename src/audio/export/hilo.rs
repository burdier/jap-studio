//! Ramas MIDI de Hilo preparadas para render offline con el plan de Play.
use super::{TrackMix, sources::Mix};
use crate::audio::piano::{PreparedArrangementVoice, VoiceSource};
use elevate::{
    StereoFrame,
    musical::{
        arrangement_midi::PreparedMidiArrangement,
        clip::{ClipProps, MidiClip},
        model::InstrumentId,
        track::{HiloMode, Track},
        weave::WeaveRule,
    },
};

pub(crate) struct HiloExportBranch {
    pub track_index: usize,
    pub group_index: usize,
    pub branch_index: usize,
    pub instruments: Vec<InstrumentId>,
    pub mode: HiloMode,
    pub weave: Option<WeaveRule>,
    pub track: Track,
    pub source: VoiceSource,
}

pub(super) struct HiloSource {
    plan: PreparedMidiArrangement,
    voice: PreparedArrangementVoice,
    mix: Mix,
}

impl HiloSource {
    pub fn prepare(
        branch: &HiloExportBranch,
        clips: &[MidiClip],
        track: &TrackMix,
        rate: u32,
        bpm: f64,
    ) -> Result<Self, String> {
        let plan = PreparedMidiArrangement::compile_hilo_branch(
            clips,
            branch.group_index,
            &branch.track,
            branch.branch_index,
            &branch.instruments,
            branch.mode,
            branch.weave.as_ref(),
            rate,
            bpm,
        )?;
        let instrument = branch
            .track
            .instrument
            .ok_or("Rama de Hilo sin instrumento")?;
        let voice = PreparedArrangementVoice::prepare(instrument, branch.source.clone(), rate)?;
        let mix = Mix::new(track, ClipProps::default(), rate)?;
        Ok(Self { plan, voice, mix })
    }

    pub fn finish_input(&mut self) {
        self.voice.finish_input();
    }

    pub fn render(&mut self, position: u64, output: &mut [StereoFrame], tail: bool) {
        let mut scratch = [[0.0; 2]; 512];
        let scratch = &mut scratch[..output.len()];
        if tail {
            self.voice.render_tail(scratch);
        } else {
            self.voice.render_timed(&self.plan, position, scratch, None);
        }
        self.mix.add(scratch, output);
    }
}
