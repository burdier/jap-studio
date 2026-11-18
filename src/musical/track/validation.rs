//! Una regla de jerarquía para la UI, los lotes y el documento Loro.
use super::{Track, TrackId, TrackKind};
use std::collections::HashMap;

pub fn validate_track_structure(tracks: &[Track]) -> Result<(), String> {
    if tracks
        .iter()
        .any(|track| track.hilo.is_some() && track.kind != TrackKind::Group)
    {
        return Err("Solo un grupo puede ser Hilo".into());
    }
    if tracks
        .iter()
        .any(|track| track.kind == TrackKind::Group && track.instrument.is_some())
    {
        return Err("Los instrumentos del Hilo pertenecen a sus pistas hijas".into());
    }
    validate_track_layout(
        tracks
            .iter()
            .map(|track| (track.id, track.kind, track.parent)),
    )?;
    for group in tracks.iter().filter(|track| track.hilo.is_some()) {
        let plan = super::super::hilo::HiloPlan::compile(tracks, group.id, group.hilo.unwrap())?;
        if group
            .weave_rule
            .as_ref()
            .is_some_and(|rule| !rule.valid_for(plan.instruments().count()))
        {
            return Err("Tejer: la regla menciona una rama inexistente".into());
        }
    }
    if tracks
        .iter()
        .any(|track| track.weave_rule.is_some() && track.hilo.is_none())
    {
        return Err("Una regla Tejer requiere un Hilo activo".into());
    }
    Ok(())
}

/// El orden es el orden visible: grupo seguido inmediatamente por sus hijos.
pub fn validate_track_layout(
    tracks: impl IntoIterator<Item = (TrackId, TrackKind, Option<TrackId>)>,
) -> Result<(), String> {
    let tracks: Vec<_> = tracks.into_iter().collect();
    let mut ids = HashMap::with_capacity(tracks.len());
    for (index, (id, _, _)) in tracks.iter().enumerate() {
        if ids.insert(*id, index).is_some() {
            return Err("ID de pista duplicado".into());
        }
    }
    let mut active_group = None;
    for (id, kind, parent) in &tracks {
        match parent {
            None => active_group = (*kind == TrackKind::Group).then_some(*id),
            Some(parent) => {
                let Some(&index) = ids.get(parent) else {
                    return Err("Falta el grupo de una pista".into());
                };
                let (_, parent_kind, grandparent) = tracks[index];
                if parent_kind != TrackKind::Group {
                    return Err("El padre de una pista no es un grupo".into());
                }
                if grandparent.is_some() || *kind == TrackKind::Group {
                    return Err("Solo se admite un nivel de grupos".into());
                }
                if active_group != Some(*parent) {
                    return Err("Los hijos deben seguir a su grupo sin interrupciones".into());
                }
            }
        }
    }
    Ok(())
}
