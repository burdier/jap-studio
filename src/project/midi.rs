//! Clips y notas MIDI persistidos por ID; las referencias a pistas usan TrackId.
use super::{
    ProjectTrack,
    track::{read_bool, read_i64, read_string},
};
use crate::musical::{
    clip::{ClipId, ClipProps, MidiClip},
    model::{InstrumentId, NoteEvent, NoteId, Pattern},
    time::Tick,
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
        map.insert("name", clip.name.as_str()).map_err(err)?;
        map.insert("channel", i64::from(clip.channel))
            .map_err(err)?;
        map.insert("start", clip.start_beat).map_err(err)?;
        map.insert("trim_start", clip.trim_start).map_err(err)?;
        map.insert("trim_len", clip.trim_len.unwrap_or(-1.0))
            .map_err(err)?;
        map.insert("pattern_len", clip.pattern.length().0)
            .map_err(err)?;
        map.insert(
            "next_note_id",
            i64::try_from(clip.pattern.next_id()).map_err(|_| "ID de nota fuera de rango")?,
        )
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
        super::midi_controls::write(doc, clip.id.value(), &map, &clip.pattern)?;
        let notes = clip.pattern.notes();
        let note_order = doc.get_movable_list(note_order_key(clip.id.value()));
        reconcile_order(
            &note_order,
            &notes.iter().map(|note| note.id.0).collect::<Vec<_>>(),
        )?;
        for note in notes {
            let map = doc.get_map(note_key(clip.id.value(), note.id.0));
            map.insert("instrument", i64::from(note.instrument.0))
                .map_err(err)?;
            map.insert("start", note.start.0).map_err(err)?;
            map.insert("duration", note.duration.0).map_err(err)?;
            map.insert("pitch", i64::from(note.pitch)).map_err(err)?;
            map.insert(
                "release_velocity",
                i64::from(note.expression.release_velocity),
            )
            .map_err(err)?;
            map.insert("probability", i64::from(note.expression.probability))
                .map_err(err)?;
            map.insert("pan", i64::from(note.expression.pan))
                .map_err(err)?;
            map.insert("velocity", i64::from(note.velocity))
                .map_err(err)?;
            map.insert("muted", clip.muted_notes.contains(&note.id))
                .map_err(err)?;
        }
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
        let note_order = doc.get_movable_list(note_order_key(id));
        let mut notes = Vec::with_capacity(note_order.len());
        let mut note_ids = HashSet::new();
        let mut muted_notes = HashSet::new();
        for index in 0..note_order.len() {
            let note_id = order_id(&note_order, index).ok_or("ID de nota inválido")?;
            if !note_ids.insert(note_id) {
                return Err("ID de nota duplicado".into());
            }
            let note = doc.get_map(note_key(id, note_id));
            if read_bool(&note, "muted")? {
                muted_notes.insert(NoteId(note_id));
            }
            notes.push(NoteEvent {
                id: NoteId(note_id),
                instrument: InstrumentId(
                    u32::try_from(read_i64(&note, "instrument")?)
                        .map_err(|_| "Instrumento MIDI inválido")?,
                ),
                start: Tick(read_i64(&note, "start")?),
                duration: Tick(read_i64(&note, "duration")?),
                pitch: u8::try_from(read_i64(&note, "pitch")?)
                    .map_err(|_| "Pitch MIDI inválido")?,
                expression: crate::musical::note_expression::NoteExpression {
                    pan: if note.get("pan").is_none() {
                        0
                    } else {
                        i8::try_from(read_i64(&note, "pan")?).map_err(|_| "Pan de nota inválido")?
                    },
                    release_velocity: if note.get("release_velocity").is_none() {
                        64
                    } else {
                        u8::try_from(read_i64(&note, "release_velocity")?)
                            .map_err(|_| "Release velocity inválida")?
                    },
                    probability: if note.get("probability").is_none() {
                        100
                    } else {
                        u8::try_from(read_i64(&note, "probability")?)
                            .map_err(|_| "Probabilidad inválida")?
                    },
                },
                velocity: u8::try_from(read_i64(&note, "velocity")?)
                    .map_err(|_| "Velocity MIDI inválida")?,
            });
        }
        let next_note_id = u64::try_from(read_i64(&map, "next_note_id")?)
            .map_err(|_| "Siguiente ID de nota inválido")?;
        let mut pattern = Pattern::restore_with_next_id(
            Tick(read_i64(&map, "pattern_len")?),
            notes,
            next_note_id,
        )
        .map_err(|error| error.to_string())?;
        super::midi_controls::read(doc, id, &map, &mut pattern)?;
        let trim_len = read_f64(&map, "trim_len")?;
        let color = read_i64(&map, "color")?;
        let clip = MidiClip {
            id: ClipId::restore(id)?,
            name: read_string(&map, "name")?,
            track,
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
        return Err("IDs MIDI duplicados".into());
    }
    for index in (0..order.len()).rev() {
        if !order_id(order, index).is_some_and(|id| keep.contains(&id)) {
            order.delete(index, 1).map_err(err)?;
        }
    }
    for &id in desired {
        if id == 0 || id >= i64::MAX as u64 {
            return Err("ID MIDI fuera de rango".into());
        }
        if !(0..order.len()).any(|index| order_id(order, index) == Some(id)) {
            order.insert(order.len(), id as i64).map_err(err)?;
        }
    }
    for (target, &id) in desired.iter().enumerate() {
        let source = (target..order.len())
            .find(|&index| order_id(order, index) == Some(id))
            .ok_or("Orden MIDI incompleto")?;
        if source != target {
            order.mov(source, target).map_err(err)?;
        }
    }
    Ok(())
}

fn read_f64(map: &LoroMap, key: &str) -> Result<f64, String> {
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
fn note_order_key(id: u64) -> String {
    format!("midi-notes:{id}")
}
fn note_key(clip: u64, note: u64) -> String {
    format!("midi-note:{clip}:{note}")
}
fn err(error: loro::LoroError) -> String {
    error.to_string()
}
