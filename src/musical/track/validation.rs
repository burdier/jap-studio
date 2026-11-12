//! Una regla de jerarquía para la UI, los lotes y el documento Loro.
use super::{Track, TrackId, TrackKind};
use std::collections::HashMap;

pub fn validate_track_structure(tracks: &[Track]) -> Result<(), String> {
    validate_track_layout(
        tracks
            .iter()
            .map(|track| (track.id, track.kind, track.parent)),
    )
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
