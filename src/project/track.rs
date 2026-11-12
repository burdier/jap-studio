//! Adaptador entre pistas musicales y el esquema CRDT.
use super::ProjectTrack;
use crate::musical::track::{TrackId, TrackKind, validate_track_layout};
use loro::ToJson;

pub(super) fn read_string(map: &loro::LoroMap, key: &str) -> Result<String, String> {
    map.get(key)
        .and_then(|v| {
            v.get_deep_value()
                .to_json_value()
                .as_str()
                .map(str::to_owned)
        })
        .ok_or_else(|| format!("Falta {key} en el proyecto"))
}
pub(super) fn read_i64(map: &loro::LoroMap, key: &str) -> Result<i64, String> {
    map.get(key)
        .and_then(|v| v.get_deep_value().to_json_value().as_i64())
        .ok_or_else(|| format!("Falta {key} en el proyecto"))
}
pub(super) fn read_bool(map: &loro::LoroMap, key: &str) -> Result<bool, String> {
    map.get(key)
        .and_then(|v| v.get_deep_value().to_json_value().as_bool())
        .ok_or_else(|| format!("Falta {key} en el proyecto"))
}
pub(super) fn read_optional_id(map: &loro::LoroMap, key: &str) -> Result<Option<u32>, String> {
    match read_i64(map, key)? {
        -1 => Ok(None),
        value => u32::try_from(value)
            .map(Some)
            .map_err(|_| format!("{key} fuera de rango")),
    }
}
pub(super) fn order_id(order: &loro::LoroMovableList, index: usize) -> Option<TrackId> {
    order
        .get(index)
        .and_then(|value| value.get_deep_value().to_json_value().as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .map(TrackId)
}
pub(super) fn validate_tracks(tracks: &[ProjectTrack]) -> Result<(), String> {
    validate_track_layout(tracks.iter().map(|track| (track.id, track.kind, track.parent)))
}
pub(super) fn kind_code(kind: TrackKind) -> i64 {
    match kind {
        TrackKind::Drum => 0,
        TrackKind::Synth => 1,
        TrackKind::Effect => 2,
        TrackKind::Audio => 3,
        TrackKind::Group => 4,
    }
}
pub(super) fn kind_from_code(code: i64) -> Result<TrackKind, String> {
    match code {
        0 => Ok(TrackKind::Drum),
        1 => Ok(TrackKind::Synth),
        2 => Ok(TrackKind::Effect),
        3 => Ok(TrackKind::Audio),
        4 => Ok(TrackKind::Group),
        _ => Err("Tipo de pista inválido".into()),
    }
}
