//! Snapshot de configuración y consumidor offline del mismo scheduler/motor que la audición.
use super::{PianoSettings, SynthBackend, load_soundfont, render::render_plan_segment};
use elevate::{
    StereoFrame,
    musical::{model::Pattern, scheduler::PreparedPattern, time::TimeBase},
};
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct PianoSnapshot {
    pub path: PathBuf,
    pub settings: PianoSettings,
    pub midi: Vec<(u8, u8, u8)>,
    pub master_volume: f32,
    pub filter: elevate::devices::filter::FilterSettings,
}

pub(crate) struct OfflinePiano {
    synth: SynthBackend,
    plan: PreparedPattern,
    position: u64,
    finished: bool,
    filter: super::filter::StereoFilter,
}

impl OfflinePiano {
    pub fn new(
        snapshot: &PianoSnapshot,
        pattern: &Pattern,
        sample_rate: u32,
        bpm: f64,
    ) -> Result<Self, String> {
        let font = load_soundfont(&snapshot.path)?;
        let mut synth = SynthBackend::new(&snapshot.path, &font, sample_rate, snapshot.settings)?;
        if !snapshot.midi.iter().any(|(command, _, _)| *command == 0xC0)
            && let Some(preset) = super::soundfont::presets_of(&font).first()
        {
            synth.midi(0xB0, 0, preset.bank);
            synth.midi(0xC0, preset.patch, 0);
        }
        synth.set_master_volume(snapshot.master_volume);
        for &(command, data1, data2) in &snapshot.midi {
            synth.midi(command, data1, data2);
        }
        let mut piano = Self::from_backend(synth, pattern, sample_rate, bpm)?;
        piano.filter = super::filter::StereoFilter::new(sample_rate, snapshot.filter);
        Ok(piano)
    }
    pub fn from_backend(
        synth: SynthBackend,
        pattern: &Pattern,
        sample_rate: u32,
        bpm: f64,
    ) -> Result<Self, String> {
        let base = TimeBase::new(sample_rate, bpm).map_err(|e| e.to_string())?;
        let plan = PreparedPattern::compile(pattern, base).map_err(|e| e.to_string())?;
        Ok(Self {
            synth,
            plan,
            position: 0,
            finished: false,
            filter: super::filter::StereoFilter::new(sample_rate, Default::default()),
        })
    }

    pub fn render(&mut self, frames: &mut [StereoFrame]) {
        let mut left = [0.0; 512];
        let mut right = [0.0; 512];
        for block in frames.chunks_mut(512) {
            let count = block.len();
            if self.finished {
                self.synth.render(&mut left[..count], &mut right[..count]);
            } else {
                self.finished = render_plan_segment(
                    &mut self.synth,
                    &self.plan,
                    &mut self.position,
                    &mut left[..count],
                    &mut right[..count],
                );
            }
            for (i, frame) in block.iter_mut().enumerate() {
                *frame = [left[i], right[i]];
            }
            self.filter.process(block);
        }
    }

    /// Fin del rango: liberar sustain/notas y renderizar solamente sus colas.
    pub fn finish_input(&mut self) {
        self.synth.midi(0xB0, 64, 0);
        self.synth.note_off_all();
        self.finished = true;
    }
}
