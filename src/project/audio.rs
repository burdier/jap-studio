//! Referencias editables a clips de audio; las muestras y el decodificador quedan fuera de Loro.
use super::{
    ProjectDoc, ProjectTrack, midi,
    track::{read_bool, read_i64, read_string},
};
use crate::musical::{
    clip::{ClipId, ClipProps},
    track::{TrackId, TrackKind},
};
use loro::{LoroDoc, LoroMap};
use std::collections::HashSet;

use crate::musical::sample_edit::{Bell, ClipFade, Crossfade, Process, RangeEdit, SampleEdit};

const CLIP_ORDER: &str = "audio_order";

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectAudioClip {
    pub id: ClipId,
    pub track: TrackId,
    /// Ruta del archivo original, guardada como UTF-8; puede ser absoluta o relativa.
    pub source: String,
    pub edit: SampleEdit,
    pub name: String,
    pub start_beat: f64,
    pub trim_start: f64,
    pub trim_len: Option<f64>,
    pub props: ClipProps,
}

impl ProjectDoc {
    /// Mueve una instancia sin reescribir sus demás campos CRDT.
    pub fn set_audio_clip_start(&self, id: ClipId, beat: f64) -> Result<(), String> {
        if !beat.is_finite() || beat < 0.0 {
            return Err("Posición de audio inválida".into());
        }
        self.require_audio_clip(id)?;
        self.doc
            .get_map(format!("audio:{}", id.value()))
            .insert("start", beat)
            .map_err(err)?;
        Ok(())
    }

    /// Ajusta la ganancia sin reescribir posición, recorte o ruta.
    pub fn set_audio_clip_gain(&self, id: ClipId, gain_db: f32) -> Result<(), String> {
        if !gain_db.is_finite() {
            return Err("Ganancia de audio inválida".into());
        }
        self.require_audio_clip(id)?;
        self.doc
            .get_map(format!("audio:{}", id.value()))
            .insert("gain_db", f64::from(gain_db))
            .map_err(err)?;
        Ok(())
    }

    fn require_audio_clip(&self, id: ClipId) -> Result<(), String> {
        if self.audio_clips()?.iter().any(|clip| clip.id == id) {
            Ok(())
        } else {
            Err("Clip de audio inexistente".into())
        }
    }
}

pub(super) fn write_clips(
    doc: &LoroDoc,
    tracks: &[ProjectTrack],
    clips: &[ProjectAudioClip],
) -> Result<(), String> {
    let mut ids = HashSet::new();
    for clip in clips {
        if !ids.insert(clip.id.value()) {
            return Err("ID de clip de audio duplicado".into());
        }
        validate_clip(clip, tracks)?;
    }
    let order = doc.get_movable_list(CLIP_ORDER);
    midi::reconcile_order(
        &order,
        &clips.iter().map(|clip| clip.id.value()).collect::<Vec<_>>(),
    )?;
    for clip in clips {
        let map = doc.get_map(format!("audio:{}", clip.id.value()));
        write_edit(&map, &clip.edit)?;
        map.insert("track", i64::from(clip.track.0)).map_err(err)?;
        map.insert("source", clip.source.as_str()).map_err(err)?;
        map.insert("name", clip.name.as_str()).map_err(err)?;
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
    }
    Ok(())
}

pub(super) fn read_clips(
    doc: &LoroDoc,
    tracks: &[ProjectTrack],
) -> Result<Vec<ProjectAudioClip>, String> {
    let order = doc.get_movable_list(CLIP_ORDER);
    let mut clips = Vec::with_capacity(order.len());
    let mut ids = HashSet::new();
    for index in 0..order.len() {
        let id = midi::order_id(&order, index).ok_or("ID de clip de audio inválido")?;
        if !ids.insert(id) {
            return Err("ID de clip de audio duplicado".into());
        }
        let map = doc.get_map(format!("audio:{id}"));
        let trim_len = midi::read_f64(&map, "trim_len")?;
        let color = read_i64(&map, "color")?;
        let clip = ProjectAudioClip {
            id: ClipId::restore(id)?,
            track: TrackId(
                u32::try_from(read_i64(&map, "track")?).map_err(|_| "TrackId de audio inválido")?,
            ),
            source: read_string(&map, "source")?,
            name: read_string(&map, "name")?,
            start_beat: midi::read_f64(&map, "start")?,
            trim_start: midi::read_f64(&map, "trim_start")?,
            trim_len: if trim_len == -1.0 {
                None
            } else {
                Some(trim_len)
            },
            edit: read_edit(&map)?,
            props: read_props(&map, color)?,
        };
        validate_clip(&clip, tracks)?;
        clips.push(clip);
    }
    Ok(clips)
}

fn read_props(map: &LoroMap, color: i64) -> Result<ClipProps, String> {
    Ok(ClipProps {
        muted: read_bool(map, "muted")?,
        locked: read_bool(map, "locked")?,
        looped: read_bool(map, "looped")?,
        gain_db: midi::read_f64(map, "gain_db")? as f32,
        pitch_semitones: i8::try_from(read_i64(map, "pitch")?)
            .map_err(|_| "Afinación de audio inválida")?,
        color_index: match color {
            -1 => None,
            value => Some(u8::try_from(value).map_err(|_| "Color de audio inválido")?),
        },
    })
}

fn validate_clip(clip: &ProjectAudioClip, tracks: &[ProjectTrack]) -> Result<(), String> {
    clip.edit.validate()?;
    if clip.id.value() == 0
        || clip.id.value() >= i64::MAX as u64
        || clip.source.is_empty()
        || clip.source.contains('\0')
        || !clip.start_beat.is_finite()
        || clip.start_beat < 0.0
        || !clip.trim_start.is_finite()
        || clip.trim_start < 0.0
        || clip
            .trim_len
            .is_some_and(|len| !len.is_finite() || len < 0.0)
        || !clip.props.gain_db.is_finite()
        || !tracks.iter().any(|track| {
            track.id == clip.track && (track.kind != TrackKind::Group || track.hilo.is_some())
        })
    {
        return Err("Propiedades de clip de audio inválidas".into());
    }
    Ok(())
}

fn err(error: loro::LoroError) -> String {
    error.to_string()
}

fn write_edit(map: &LoroMap, edit: &SampleEdit) -> Result<(), String> {
    for (key, fade) in ["fade_in", "fade_out"].into_iter().zip(edit.fades) {
        if fade.active() {
            let encoded = format!("{},{},{}", fade.start_us, fade.end_us, fade.bend);
            if read_string(map, key).ok().as_ref() != Some(&encoded) {
                map.insert(key, encoded.as_str()).map_err(err)?;
            }
        } else if map.get(key).is_some() {
            map.delete(key).map_err(err)?;
        }
    }
    if let Some(root) = edit.crossfade_root {
        map.insert("crossfade_root", root.value() as i64)
            .map_err(err)?;
    } else if map.get("crossfade_root").is_some() {
        map.delete("crossfade_root").map_err(err)?;
    }
    if let Some(fade) = edit.crossfade {
        let value = format!("{}:{}", fade.from.value(), fade.center);
        if read_string(map, "crossfade").ok().as_ref() != Some(&value) {
            map.insert("crossfade", value.as_str()).map_err(err)?;
        }
    } else if map.get("crossfade").is_some() {
        map.delete("crossfade").map_err(err)?;
    }
    if edit.fine_cents != 0 || map.get("fine_cents").is_some() {
        map.insert("fine_cents", i64::from(edit.fine_cents))
            .map_err(err)?;
    }
    let initial = Bell::default();
    for (key, value, default) in [
        ("eq_hz", edit.bell.hz, initial.hz),
        ("eq_db", edit.bell.db, initial.db),
        ("eq_q", edit.bell.q, initial.q),
    ] {
        if value != default || map.get(key).is_some() {
            map.insert(key, value).map_err(err)?;
        }
    }
    if !edit.bell.enabled || map.get("eq_on").is_some() {
        map.insert("eq_on", edit.bell.enabled).map_err(err)?;
    }
    if !edit.ranges.is_empty() || map.get("range_count").is_some() {
        map.insert("range_count", edit.ranges.len() as i64)
            .map_err(err)?;
    }
    for (i, r) in edit.ranges.iter().enumerate() {
        map.insert(&format!("range_{i}_start"), r.first as i64)
            .map_err(err)?;
        map.insert(&format!("range_{i}_end"), r.end as i64)
            .map_err(err)?;
        map.insert(
            &format!("range_{i}_op"),
            match r.op {
                Process::FadeOut => 0i64,
                Process::Reverse => 1,
                Process::Normalize => 2,
                Process::FadeIn => 3,
                Process::Silence => 4,
                Process::ChordStart => 5,
                Process::ChordNext => 6,
            },
        )
        .map_err(err)?;
    }
    Ok(())
}
fn read_edit(map: &LoroMap) -> Result<SampleEdit, String> {
    let count = if map.get("range_count").is_some() {
        read_i64(map, "range_count")?
    } else {
        0
    };
    let num = |key: &str, default: f64| {
        if map.get(key).is_some() {
            midi::read_f64(map, key)
        } else {
            Ok(default)
        }
    };
    let d = Bell::default();
    if !(0..=SampleEdit::LIMIT as i64).contains(&count) {
        return Err("Demasiadas ediciones de muestra".into());
    }
    let mut ranges = Vec::with_capacity(count as usize);
    for i in 0..count {
        let frame = |part: &str| {
            usize::try_from(read_i64(map, &format!("range_{i}_{part}"))?)
                .map_err(|_| "Frame de audio inválido".to_string())
        };
        ranges.push(RangeEdit {
            first: frame("start")?,
            end: frame("end")?,
            op: match read_i64(map, &format!("range_{i}_op"))? {
                0 => Process::FadeOut,
                1 => Process::Reverse,
                2 => Process::Normalize,
                3 => Process::FadeIn,
                4 => Process::Silence,
                5 => Process::ChordStart,
                6 => Process::ChordNext,
                _ => return Err("Operación de audio desconocida".into()),
            },
        });
    }
    let fine_cents = if map.get("fine_cents").is_some() {
        i8::try_from(read_i64(map, "fine_cents")?).map_err(|_| "Afinación fina inválida")?
    } else {
        0
    };
    let crossfade = if map.get("crossfade").is_some() {
        let value = read_string(map, "crossfade")?;
        let (id, center) = value.split_once(':').ok_or("Cruce de audio inválido")?;
        Some(Crossfade {
            from: ClipId::restore(id.parse().map_err(|_| "ID de cruce inválido")?)?,
            center: center.parse().map_err(|_| "Centro de cruce inválido")?,
        })
    } else {
        None
    };
    let crossfade_root = if map.get("crossfade_root").is_some() {
        Some(ClipId::restore(read_i64(map, "crossfade_root")? as u64)?)
    } else {
        None
    };
    let fade = |key| -> Result<ClipFade, String> {
        if map.get(key).is_none() {
            return Ok(ClipFade::default());
        }
        let value = read_string(map, key)?;
        let mut parts = value.split(',');
        let (Some(start), Some(end), Some(bend), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err("Fundido inválido".into());
        };
        Ok(ClipFade {
            start_us: start.parse().map_err(|_| "Inicio de fundido inválido")?,
            end_us: end.parse().map_err(|_| "Fin de fundido inválido")?,
            bend: bend.parse().map_err(|_| "Curva de fundido inválida")?,
        })
    };
    Ok(SampleEdit {
        fades: [fade("fade_in")?, fade("fade_out")?],
        crossfade_root,
        crossfade,
        fine_cents,
        bell: Bell {
            hz: num("eq_hz", d.hz)?,
            db: num("eq_db", d.db)?,
            q: num("eq_q", d.q)?,
            enabled: if map.get("eq_on").is_some() {
                read_bool(map, "eq_on")?
            } else {
                d.enabled
            },
        },
        ranges,
    })
}
