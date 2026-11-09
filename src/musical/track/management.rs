//! Ediciones estructurales de pistas. Un nivel de grupos, orden estable e IDs no reutilizados.
use super::{Track, TrackId, TrackKind};

#[derive(Clone, Copy)]
pub enum TrackEdit {
    Insert,
    Delete,
    Duplicate,
    Group,
    Ungroup,
}

pub fn edit_tracks(
    tracks: &[Track],
    selected: &[TrackId],
    edit: TrackEdit,
    cursor: usize,
) -> Result<(Vec<Track>, Vec<Option<usize>>), String> {
    let selected: Vec<_> = tracks
        .iter()
        .filter(|t| {
            selected.contains(&t.id) || t.parent.is_some_and(|parent| selected.contains(&parent))
        })
        .map(|t| t.id)
        .collect();
    let mut output = tracks.to_vec();
    let mut duplicates = Vec::new();
    match edit {
        TrackEdit::Insert => {
            let mut track = Track::new("Nueva pista", TrackKind::Audio);
            let after = tracks.get(cursor);
            track.parent = after.and_then(|t| {
                if t.kind == TrackKind::Group {
                    Some(t.id)
                } else {
                    t.parent
                }
            });
            output.insert((cursor + 1).min(output.len()), track);
        }
        TrackEdit::Delete => output.retain(|t| !selected.contains(&t.id)),
        TrackEdit::Duplicate => {
            let mut copies = Vec::new();
            let mut groups = Vec::new();
            for original in tracks.iter().filter(|t| selected.contains(&t.id)) {
                let mut copy = original.clone();
                copy.id = Track::new("", original.kind).id;
                duplicates.push((copy.id, original.id));
                copy.name = format!("{} copia", original.name).into();
                // La única instancia de instrumento sigue asignada a su pista original.
                copy.instrument = None;
                copy.chain = original.chain.duplicate();
                if copy.kind == TrackKind::Group {
                    groups.push((original.id, copy.id));
                }
                copy.parent = copy.parent.map(|id| {
                    groups
                        .iter()
                        .find(|(old, _)| *old == id)
                        .map_or(id, |(_, new)| *new)
                });
                copies.push(copy);
            }
            let position = tracks
                .iter()
                .rposition(|t| selected.contains(&t.id))
                .map_or(output.len(), |i| i + 1);
            output.splice(position..position, copies);
        }
        TrackEdit::Group => {
            let members: Vec<_> = tracks
                .iter()
                .filter(|t| selected.contains(&t.id) && t.accepts_clips())
                .cloned()
                .collect();
            if members.len() < 2 {
                return Err("Selecciona al menos dos pistas con Ctrl+clic o Shift+clic".into());
            }
            let position = tracks
                .iter()
                .position(|t| selected.contains(&t.id))
                .unwrap_or(0);
            let group = Track::new("Grupo", TrackKind::Group);
            let id = group.id;
            output.retain(|t| !selected.contains(&t.id));
            let position = position.min(output.len());
            output.splice(
                position..position,
                std::iter::once(group).chain(members.into_iter().map(|mut t| {
                    t.parent = Some(id);
                    t
                })),
            );
        }
        TrackEdit::Ungroup => {
            let groups: Vec<_> = tracks
                .iter()
                .filter(|t| selected.contains(&t.id))
                .filter_map(|t| {
                    if t.kind == TrackKind::Group {
                        Some(t.id)
                    } else {
                        t.parent
                    }
                })
                .collect();
            output.retain(|t| !groups.contains(&t.id));
            for t in &mut output {
                if t.parent.is_some_and(|id| groups.contains(&id)) {
                    t.parent = None;
                }
            }
        }
    }
    // Mantener todos los hijos contiguos incluso al reagrupar o duplicar una selección dispersa.
    let mut ordered = Vec::new();
    for track in output.iter().filter(|t| t.parent.is_none()) {
        ordered.push(track.clone());
        if track.kind == TrackKind::Group {
            ordered.extend(
                output
                    .iter()
                    .filter(|t| t.parent == Some(track.id))
                    .cloned(),
            );
        }
    }
    let sources = ordered
        .iter()
        .map(|t| {
            let id = duplicates
                .iter()
                .find(|(new, _)| *new == t.id)
                .map_or(t.id, |(_, old)| *old);
            tracks.iter().position(|old| old.id == id)
        })
        .collect();
    Ok((ordered, sources))
}
