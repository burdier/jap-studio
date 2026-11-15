//! Una edición sobre un candidato privado; estructura y recursos se validan por paso.
use super::super::{
    TrackBatchEdit, TrackBatchError,
    validation::{error, index, name, new_handle, reference, validate_fx, validate_instruments},
};
use crate::musical::track::{
    Track, TrackEdit, TrackId, TrackKind, TrackMix, TrackSnapshot, TrackSnapshotEntry, edit_tracks,
    validate_track_structure,
};
use std::collections::BTreeMap;

const MAX_NEW_TRACKS: usize = 64;

pub(super) fn apply_edit(
    candidate: &mut TrackSnapshot,
    handles: &mut BTreeMap<String, TrackId>,
    created: &mut usize,
    edit: &TrackBatchEdit,
) -> Result<(), TrackBatchError> {
    match edit {
        TrackBatchEdit::InsertAudio {
            handle,
            name: label,
            after,
            parent,
        } => {
            new_handle(handle, &handles)?;
            name(label)?;
            *created += 1;
            if *created > MAX_NEW_TRACKS {
                return Err(error("limite", "Máximo 64 pistas nuevas por lote"));
            }
            let parent = parent
                .as_ref()
                .map(|r| reference(r, &handles))
                .transpose()?;
            if let Some(id) = parent {
                let row = index(&candidate.entries, id)?;
                if candidate.entries[row].track.kind != TrackKind::Group {
                    return Err(error("estructura_invalida", "El padre no es un grupo"));
                }
            }
            let position = if let Some(after) = after {
                let id = reference(after, &handles)?;
                let row = index(&candidate.entries, id)?;
                if candidate.entries[row].track.parent != parent {
                    return Err(error(
                        "estructura_invalida",
                        "La posición y el grupo no coinciden",
                    ));
                }
                let mut position = row + 1;
                if candidate.entries[row].track.kind == TrackKind::Group {
                    while position < candidate.entries.len()
                        && candidate.entries[position].track.parent == Some(id)
                    {
                        position += 1;
                    }
                }
                position
            } else if let Some(id) = parent {
                let row = index(&candidate.entries, id)?;
                let mut position = row + 1;
                while position < candidate.entries.len()
                    && candidate.entries[position].track.parent == Some(id)
                {
                    position += 1;
                }
                position
            } else {
                candidate.entries.len()
            };
            let mut track = Track::new(label.clone(), TrackKind::Audio);
            track.parent = parent;
            handles.insert(handle.clone(), track.id);
            candidate.entries.insert(
                position,
                TrackSnapshotEntry {
                    track,
                    mix: TrackMix { level: 75, pan: 50 },
                },
            );
        }
        TrackBatchEdit::Group {
            handle,
            name: label,
            members,
        } => {
            new_handle(handle, &handles)?;
            name(label)?;
            *created += 1;
            if *created > MAX_NEW_TRACKS {
                return Err(error("limite", "Máximo 64 pistas nuevas por lote"));
            }
            let selected = members
                .iter()
                .map(|r| reference(r, &handles))
                .collect::<Result<Vec<_>, _>>()?;
            if selected.len() < 2
                || selected
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != selected.len()
            {
                return Err(error(
                    "operacion_invalida",
                    "El grupo necesita dos miembros distintos",
                ));
            }
            for id in &selected {
                let row = index(&candidate.entries, *id)?;
                if candidate.entries[row].track.parent.is_some()
                    || candidate.entries[row].track.kind == TrackKind::Group
                    || !candidate.entries[row].track.accepts_clips()
                {
                    return Err(error(
                        "estructura_invalida",
                        "Solo se agrupan pistas raíz que aceptan clips",
                    ));
                }
            }
            let before: BTreeMap<_, _> = candidate
                .entries
                .iter()
                .map(|entry| (entry.track.id, entry.mix))
                .collect();
            let tracks: Vec<_> = candidate
                .entries
                .iter()
                .map(|entry| entry.track.clone())
                .collect();
            let (mut tracks, _) = edit_tracks(&tracks, &selected, TrackEdit::Group, 0)
                .map_err(|e| error("estructura_invalida", e))?;
            let id = tracks
                .iter()
                .find(|track| track.kind == TrackKind::Group && !before.contains_key(&track.id))
                .map(|track| track.id)
                .ok_or_else(|| error("estructura_invalida", "No se creó el grupo"))?;
            tracks.iter_mut().find(|track| track.id == id).unwrap().name = label.clone().into();
            candidate.entries = tracks
                .into_iter()
                .map(|track| TrackSnapshotEntry {
                    mix: before
                        .get(&track.id)
                        .copied()
                        .unwrap_or(TrackMix { level: 75, pan: 50 }),
                    track,
                })
                .collect();
            handles.insert(handle.clone(), id);
        }
        TrackBatchEdit::Hilo { track, mode } => {
            let id = reference(track, &handles)?;
            let row = index(&candidate.entries, id)?;
            if candidate.entries[row].track.kind != TrackKind::Group {
                return Err(error(
                    "estructura_invalida",
                    "Hilo requiere una pista grupo",
                ));
            }
            if let Some(mode) = mode {
                let tracks: Vec<_> = candidate
                    .entries
                    .iter()
                    .map(|entry| entry.track.clone())
                    .collect();
                crate::musical::hilo::HiloPlan::compile(&tracks, id, *mode)
                    .map_err(|message| error("estructura_invalida", message))?;
            }
            candidate.entries[row].track.hilo = *mode;
        }
        TrackBatchEdit::Rename { track, name: label } => {
            name(label)?;
            let id = reference(track, &handles)?;
            let row = index(&candidate.entries, id)?;
            candidate.entries[row].track.name = label.clone().into();
        }
        TrackBatchEdit::Move { track, after } => {
            let id = reference(track, &handles)?;
            let destination = reference(after, &handles)?;
            let row = index(&candidate.entries, id)?;
            let target = index(&candidate.entries, destination)?;
            if candidate.entries[row].track.parent != candidate.entries[target].track.parent
                || id == destination
            {
                return Err(error(
                    "estructura_invalida",
                    "Mueve pistas distintas dentro del mismo grupo o nivel",
                ));
            }
            let before: BTreeMap<_, _> = candidate
                .entries
                .iter()
                .map(|entry| (entry.track.id, entry.mix))
                .collect();
            let tracks: Vec<_> = candidate
                .entries
                .iter()
                .map(|entry| entry.track.clone())
                .collect();
            let (tracks, _) = edit_tracks(
                &tracks,
                &[id],
                TrackEdit::Move {
                    target: destination,
                    after: true,
                },
                row,
            )
            .map_err(|e| error("estructura_invalida", e))?;
            candidate.entries = tracks
                .into_iter()
                .map(|track| TrackSnapshotEntry {
                    mix: before[&track.id],
                    track,
                })
                .collect();
        }
        TrackBatchEdit::Mute { track, value } | TrackBatchEdit::Solo { track, value } => {
            let id = reference(track, &handles)?;
            let row = index(&candidate.entries, id)?;
            if matches!(edit, TrackBatchEdit::Mute { .. }) {
                candidate.entries[row].track.muted = *value;
            } else {
                candidate.entries[row].track.solo = *value;
            }
        }
        TrackBatchEdit::Level { track, value } | TrackBatchEdit::Pan { track, value } => {
            if !(0..=100).contains(value) {
                return Err(error(
                    "operacion_invalida",
                    "Nivel y panorama deben estar entre 0 y 100",
                ));
            }
            let id = reference(track, &handles)?;
            let row = index(&candidate.entries, id)?;
            if matches!(edit, TrackBatchEdit::Level { .. }) {
                candidate.entries[row].mix.level = *value;
            } else {
                candidate.entries[row].mix.pan = *value;
            }
        }
        TrackBatchEdit::Fx {
            track,
            edit: fx_edit,
        } => {
            let id = reference(track, &handles)?;
            let row = index(&candidate.entries, id)?;
            validate_fx(fx_edit, &candidate.entries[row])?;
            candidate.entries[row]
                .track
                .chain
                .apply(fx_edit.clone())
                .map_err(|e| error("estructura_invalida", e))?;
        }
    }
    validate_track_structure(
        &candidate
            .entries
            .iter()
            .map(|entry| entry.track.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(|e| error("estructura_invalida", e))?;
    validate_instruments(&candidate.entries)?;
    Ok(())
}
