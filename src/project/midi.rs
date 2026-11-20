//! Clips y notas MIDI persistidos por ID; las referencias a pistas usan TrackId.
use super::{
    ProjectTrack,
    track::{read_bool, read_i64, read_string},
};
use crate::musical::{
    clip::{ClipId, ClipProps, MidiClip},
    model::Pattern,
    track::{TrackId, TrackKind},
};
use loro::{LoroDoc, LoroMap, LoroMovableList, ToJson};
use std::collections::HashSet;

const CLIP_ORDER: &str = "midi_order";

pub(super) fn write_clips(
    doc: &LoroDoc,
    tracks: &[ProjectTrack],
    clips: &[MidiClip],
) -> Result<(), String> {
    let mut ids = HashSet::new();
    for clip in clips {
        if !ids.insert(clip.id.value()) {
            return Err("ID de clip MIDI duplicado".into());
        }
        let track = tracks.get(clip.track).ok_or("Carril MIDI inexistente")?;
        if track.kind == TrackKind::Group && track.hilo.is_none() {
            return Err("Un grupo sin Hilo no admite clips MIDI".into());
        }
        validate_clip(clip)?;
        if let Some(target) = clip.hilo_target
            && !tracks
                .iter()
                .any(|candidate| candidate.id == target && candidate.hilo.is_some())
        {
            return Err("Hilo de destino MIDI inexistente".into());
        }
    }
    let order = doc.get_movable_list(CLIP_ORDER);
    reconcile_order(
        &order,
        &clips.iter().map(|clip| clip.id.value()).collect::<Vec<_>>(),
    )?;
    for clip in clips {
        let map = doc.get_map(clip_key(clip.id.value()));
        map.insert("track", i64::from(tracks[clip.track].id.0))
            .map_err(err)?;
        map.insert(
            "hilo_target",
            clip.hilo_target.map_or(-1, |id| i64::from(id.0)),
        )
        .map_err(err)?;
        map.insert("name", clip.name.as_str()).map_err(err)?;
        map.insert("channel", i64::from(clip.channel))
            .map_err(err)?;
        map.insert("start", clip.start_beat).map_err(err)?;
        map.insert("trim_start", clip.trim_start).map_err(err)?;
        map.insert("trim_len", clip.trim_len.unwrap_or(-1.0))
            .map_err(err)?;
        map.insert("muted", clip.props.muted).map_err(err)?;
        map.insert("locked", clip.props.locked).map_err(err)?;
        map.insert("looped", clip.props.looped).map_err(err)?;
        map.insert("gain_db", f64::from(clip.props.gain_db))
            .map_err(err)?;
        map.insert("pitch", i64::from(clip.props.pitch_semitones))
            .map_err(err)?;
        map.insert("color", clip.props.color_index.map_or(-1, i64::from))
            .map_err(err)?;
        super::pattern::write(
            doc,
            "midi",
            clip.id.value(),
            &map,
            &clip.pattern,
            &clip.muted_notes,
        )?;
    }
    Ok(())
}

pub(super) fn read_clips(doc: &LoroDoc, tracks: &[ProjectTrack]) -> Result<Vec<MidiClip>, String> {
    let order = doc.get_movable_list(CLIP_ORDER);
    let mut clips = Vec::with_capacity(order.len());
    let mut ids = HashSet::new();
    for index in 0..order.len() {
        let id = order_id(&order, index).ok_or("ID de clip MIDI inválido")?;
        if !ids.insert(id) {
            return Err("ID de clip MIDI duplicado".into());
        }
        let map = doc.get_map(clip_key(id));
        let track_id = u32::try_from(read_i64(&map, "track")?)
            .map(TrackId)
            .map_err(|_| "TrackId MIDI inválido")?;
        let track = tracks
            .iter()
            .position(|track| {
                track.id == track_id && (track.kind != TrackKind::Group || track.hilo.is_some())
            })
            .ok_or("Pista del clip MIDI inexistente")?;
        let (pattern, muted_notes) = super::pattern::read(doc, "midi", id, &map)?;
        let trim_len = read_f64(&map, "trim_len")?;
        let color = read_i64(&map, "color")?;
        let clip = MidiClip {
            id: ClipId::restore(id)?,
            name: read_string(&map, "name")?,
            track,
            hilo_target: if map.get("hilo_target").is_none() || read_i64(&map, "hilo_target")? == -1
            {
                None
            } else {
                let target = u32::try_from(read_i64(&map, "hilo_target")?)
                    .map(TrackId)
                    .map_err(|_| "TrackId de Hilo inválido")?;
                if !tracks
                    .iter()
                    .any(|candidate| candidate.id == target && candidate.hilo.is_some())
                {
                    return Err("Hilo de destino MIDI inexistente".into());
                }
                Some(target)
            },
            start_beat: read_f64(&map, "start")?,
            pattern,
            channel: if map.get("channel").is_none() {
                1
            } else {
                u8::try_from(read_i64(&map, "channel")?).map_err(|_| "Canal MIDI inválido")?
            },
            props: ClipProps {
                muted: read_bool(&map, "muted")?,
                locked: read_bool(&map, "locked")?,
                looped: read_bool(&map, "looped")?,
                gain_db: read_f64(&map, "gain_db")? as f32,
                pitch_semitones: i8::try_from(read_i64(&map, "pitch")?)
                    .map_err(|_| "Afinación MIDI inválida")?,
                color_index: match color {
                    -1 => None,
                    value => Some(u8::try_from(value).map_err(|_| "Color MIDI inválido")?),
                },
            },
            muted_notes,
            trim_start: read_f64(&map, "trim_start")?,
            trim_len: if trim_len == -1.0 {
                None
            } else {
                Some(trim_len)
            },
        };
        validate_clip(&clip)?;
        clips.push(clip);
    }
    Ok(clips)
}

fn validate_clip(clip: &MidiClip) -> Result<(), String> {
    if !(1..=16).contains(&clip.channel)
        || clip.id.value() == 0
        || clip.id.value() >= i64::MAX as u64
        || !clip.start_beat.is_finite()
        || clip.start_beat < 0.0
        || !clip.trim_start.is_finite()
        || clip.trim_start < 0.0
        || clip
            .trim_len
            .is_some_and(|len| !len.is_finite() || len < 0.0)
        || !clip.props.gain_db.is_finite()
        || clip.pattern.next_id() > i64::MAX as u64
    {
        return Err("Propiedades de clip MIDI inválidas".into());
    }
    Pattern::restore_with_next_id(
        clip.pattern.length(),
        clip.pattern.notes().to_vec(),
        clip.pattern.next_id(),
    )
    .map_err(|error| error.to_string())?;
    if !clip
        .muted_notes
        .iter()
        .all(|id| clip.notes().iter().any(|note| note.id == *id))
    {
        return Err("Nota silenciada inexistente".into());
    }
    Ok(())
}

pub(super) fn reconcile_order(order: &LoroMovableList, desired: &[u64]) -> Result<(), String> {
    let keep: HashSet<_> = desired.iter().copied().collect();
    if keep.len() != desired.len() {
        return Err("IDs duplicados".into());
    }
    for index in (0..order.len()).rev() {
        if !order_id(order, index).is_some_and(|id| keep.contains(&id)) {
            order.delete(index, 1).map_err(err)?;
        }
    }
    for &id in desired {
        if id == 0 || id >= i64::MAX as u64 {
            return Err("ID fuera de rango".into());
        }
        if !(0..order.len()).any(|index| order_id(order, index) == Some(id)) {
            order.insert(order.len(), id as i64).map_err(err)?;
        }
    }
    for (target, &id) in desired.iter().enumerate() {
        let source = (target..order.len())
            .find(|&index| order_id(order, index) == Some(id))
            .ok_or("Orden incompleto")?;
        if source != target {
            order.mov(source, target).map_err(err)?;
        }
    }
    Ok(())
}

pub(super) fn read_f64(map: &LoroMap, key: &str) -> Result<f64, String> {
    map.get(key)
        .and_then(|value| value.get_deep_value().to_json_value().as_f64())
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("{key} MIDI inválido"))
}
pub(super) fn order_id(order: &LoroMovableList, index: usize) -> Option<u64> {
    order
        .get(index)
        .and_then(|value| value.get_deep_value().to_json_value().as_u64())
}
fn clip_key(id: u64) -> String {
    format!("midi:{id}")
}
fn err(error: loro::LoroError) -> String {
    error.to_string()
}
