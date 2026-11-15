//! Plan musical de un Hilo. Las pistas hijas son ramas con IDs estables.
//! Su ruta fija tiene entrada de eventos, instrumento, FX de la hija y salida de audio.
use super::{
    model::InstrumentId,
    track::{HiloMode, Track, TrackId, TrackKind},
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    Events,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HiloBranchKind {
    Instrument(InstrumentId),
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiloBranch {
    pub track: TrackId,
    pub kind: HiloBranchKind,
    pub input: PortType,
    pub output: PortType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiloPlan {
    pub group: TrackId,
    pub mode: HiloMode,
    pub branches: Vec<HiloBranch>,
}

impl HiloPlan {
    /// Compila la ruta fuera del callback. El orden visible de las hijas determina
    /// el round robin; el modelo de un nivel de grupos impide ciclos.
    pub fn compile(tracks: &[Track], group: TrackId, mode: HiloMode) -> Result<Self, String> {
        let parent = tracks
            .iter()
            .find(|track| track.id == group)
            .ok_or("Grupo del Hilo inexistente")?;
        if parent.kind != TrackKind::Group || parent.parent.is_some() || parent.instrument.is_some()
        {
            return Err("El contenedor del Hilo debe ser un grupo raíz sin instrumento".into());
        }
        let mut ids = HashSet::new();
        let mut branches = Vec::new();
        for child in tracks.iter().filter(|track| track.parent == Some(group)) {
            if child.kind == TrackKind::Group {
                return Err("Rama de Hilo inválida o instrumento duplicado".into());
            }
            let kind = if let Some(instrument) = child.instrument {
                if !ids.insert(instrument) {
                    return Err("Rama de Hilo inválida o instrumento duplicado".into());
                }
                HiloBranchKind::Instrument(instrument)
            } else {
                HiloBranchKind::Audio
            };
            branches.push(HiloBranch {
                track: child.id,
                kind,
                input: if matches!(kind, HiloBranchKind::Audio) {
                    PortType::Audio
                } else {
                    PortType::Events
                },
                output: PortType::Audio,
            });
        }
        if ids.len() < 2 {
            return Err("El Hilo necesita al menos dos pistas hijas con instrumento".into());
        }
        Ok(Self {
            group,
            mode,
            branches,
        })
    }

    pub fn branch_index(&self, track: TrackId) -> Option<usize> {
        self.branches
            .iter()
            .filter(|branch| matches!(branch.kind, HiloBranchKind::Instrument(_)))
            .position(|branch| branch.track == track)
    }

    pub fn instruments(&self) -> impl Iterator<Item = InstrumentId> + '_ {
        self.branches.iter().filter_map(|branch| match branch.kind {
            HiloBranchKind::Instrument(id) => Some(id),
            HiloBranchKind::Audio => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::track::{TrackEdit, edit_tracks};

    #[test]
    fn fixed_route_has_typed_ports_and_rejects_invalid_branches() {
        let group = Track::new("Hilo", TrackKind::Group);
        let mut first = Track::new("A", TrackKind::Synth);
        first.parent = Some(group.id);
        first.instrument = Some(InstrumentId(1));
        let mut second = Track::new("B", TrackKind::Synth);
        second.parent = Some(group.id);
        second.instrument = Some(InstrumentId(2));
        let mut audio = Track::new("WAV", TrackKind::Audio);
        audio.parent = Some(group.id);
        let tracks = [group.clone(), first.clone(), second.clone(), audio.clone()];
        let plan = HiloPlan::compile(&tracks, group.id, HiloMode::RoundRobin).unwrap();
        assert_eq!(plan.branch_index(second.id), Some(1));
        assert_eq!(plan.branches[2].kind, HiloBranchKind::Audio);
        assert_eq!(plan.branches[2].input, PortType::Audio);
        assert_eq!(
            plan.instruments().collect::<Vec<_>>(),
            vec![InstrumentId(1), InstrumentId(2)]
        );
        second.instrument = first.instrument;
        assert!(HiloPlan::compile(&[group, first, second, audio], plan.group, plan.mode).is_err());
        assert_eq!(plan.branches.len(), 3);
    }

    #[test]
    fn removing_a_required_branch_rejects_the_edit_and_preserves_the_route() {
        let mut group = Track::new("Hilo", TrackKind::Group);
        group.hilo = Some(HiloMode::Layer);
        let mut first = Track::new("A", TrackKind::Synth);
        first.parent = Some(group.id);
        first.instrument = Some(InstrumentId(41));
        let mut second = Track::new("B", TrackKind::Synth);
        second.parent = Some(group.id);
        second.instrument = Some(InstrumentId(42));
        let tracks = vec![group.clone(), first, second.clone()];
        assert!(edit_tracks(&tracks, &[second.id], TrackEdit::Delete, 2).is_err());
        assert_eq!(
            HiloPlan::compile(&tracks, group.id, HiloMode::Layer)
                .unwrap()
                .branches
                .len(),
            2
        );
    }
}
