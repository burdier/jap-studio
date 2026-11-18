//! Captura del Hilo para exportación: mismos IDs, orden de ramas y regla que Play.
use super::DawApp;
use crate::audio::{export::HiloExportBranch, piano::VoiceSource};
use elevate::{
    export::ExportOptions,
    musical::{clip::MidiClip, hilo::HiloPlan, track::HiloMode},
};

impl DawApp {
    pub(super) fn capture_hilo_export(
        &self,
        midi: &[MidiClip],
        options: &ExportOptions,
    ) -> Result<Vec<HiloExportBranch>, String> {
        let mut branches = Vec::new();
        for (group_index, group) in self.tracks.iter().enumerate() {
            let Some(mode) = group.hilo else {
                continue;
            };
            if !midi
                .iter()
                .any(|clip| clip.targets_hilo(group_index, group.id) && !clip.props.muted)
            {
                continue;
            }
            if !self.tracks.iter().enumerate().any(|(index, track)| {
                options.includes_track(index)
                    && track.parent == Some(group.id)
                    && track.instrument.is_some()
            }) {
                continue;
            }
            let plan = HiloPlan::compile(&self.tracks, group.id, mode)?;
            let instruments: Vec<_> = plan.instruments().collect();
            for (branch_index, &instrument) in instruments.iter().enumerate() {
                let track_index = self
                    .tracks
                    .iter()
                    .position(|track| {
                        track.parent == Some(group.id) && track.instrument == Some(instrument)
                    })
                    .ok_or("Rama de Hilo inexistente")?;
                if !options.includes_track(track_index) {
                    continue;
                }
                let source = if self.is_piano_id(instrument) {
                    VoiceSource::Piano(self.piano_snapshot(instrument)?)
                } else if self.is_sampler_id(instrument) {
                    VoiceSource::Sampler(self.sampler_snapshot(instrument)?)
                } else if self.is_vst_id(instrument) {
                    return Err(
                        "Exportar ramas con instrumentos VST3 todavía no está disponible".into(),
                    );
                } else {
                    return Err("Falta el instrumento de una rama del Hilo".into());
                };
                let weave = if mode == HiloMode::RoundRobin {
                    self.weave_rule.as_ref().map_err(Clone::clone)?.clone()
                } else {
                    None
                };
                branches.push(HiloExportBranch {
                    track_index,
                    group_index,
                    branch_index,
                    instruments: instruments.clone(),
                    mode,
                    weave,
                    track: self.tracks[track_index].clone(),
                    source,
                });
            }
        }
        Ok(branches)
    }
}
