//! Consolida ventanas MIDI en un patrón independiente; nunca procesa audio.
use super::{
    clip::MidiClip,
    model::Pattern,
    time::{PPQ, Tick},
};

fn ticks(beat: f64) -> Result<i64, String> {
    let value = (beat * PPQ as f64).round();
    if !value.is_finite() || value < 0.0 || value >= i64::MAX as f64 {
        return Err("Posición MIDI fuera de rango".into());
    }
    Ok(value as i64)
}

/// Un clip fija su recorte; varios conservan offsets, silencios y notas simultáneas.
/// El resultado se prepara entero antes de retirar los originales del Arrangement.
pub fn materialize(clips: &[&MidiClip]) -> Result<MidiClip, String> {
    let mut clips = clips.to_vec();
    clips.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    let first = *clips.first().ok_or("Selecciona al menos un clip MIDI")?;
    let origin = ticks(first.start_beat)?;
    let mut end = origin;
    for clip in &clips {
        if clip.track != first.track {
            return Err("Materialize requiere clips MIDI de la misma pista".into());
        }
        if clip.drum_target.map(|target| (target.kit, target.piece))
            != first.drum_target.map(|target| (target.kit, target.piece))
        {
            return Err("Materialize requiere clips con el mismo destino de batería".into());
        }
        if clip.props.locked {
            return Err("Desbloquea los clips antes de materializar".into());
        }
        let a = clip.props;
        let b = first.props;
        if clip.channel != first.channel
            || a.gain_db != b.gain_db
            || a.muted != b.muted
            || a.pitch_semitones != b.pitch_semitones
            || a.looped != b.looped
        {
            return Err(
                "Para conservar el sonido, iguala canal, ganancia, mute, pitch y loop antes de unir"
                    .into(),
            );
        }
        end = end.max(ticks(clip.end_beat())?);
    }
    let mut pattern = Pattern::new(Tick(end - origin)).map_err(|e| e.to_string())?;
    let mut muted_notes = std::collections::HashSet::new();
    for clip in clips {
        let offset = ticks(clip.start_beat)? - origin;
        let source_start = ticks(clip.trim_start)?;
        let length = ticks(clip.end_beat())? - ticks(clip.start_beat)?;
        let source_end = source_start
            .checked_add(length)
            .ok_or("Duración fuera de rango")?;
        let controls = clip
            .pattern
            .controls()
            .window(Tick(source_start), Tick(length));
        for point in controls.points() {
            let at = Tick(offset + point.at.0);
            if pattern.controls().points().iter().any(|existing| {
                existing.at == at && existing.lane == point.lane && existing.value != point.value
            }) {
                return Err(
                    "Los clips tienen controles MIDI incompatibles en el mismo instante".into(),
                );
            }
            pattern.set_control(point.lane, at, point.value)?;
        }
        for note in clip.notes() {
            let start = note.start.0.max(source_start);
            let end = (note.start.0 + note.duration.0).min(source_end);
            if end > start {
                let id = pattern
                    .add_note_copy(
                        *note,
                        Tick(offset + start - source_start),
                        Tick(end - start),
                        note.instrument,
                    )
                    .map_err(|e| e.to_string())?;
                if clip.muted_notes.contains(&note.id) {
                    muted_notes.insert(id);
                }
            }
        }
    }
    let mut result = MidiClip::new(
        format!("{} · materialized", first.name),
        first.track,
        origin as f64 / PPQ as f64,
        pattern,
    );
    result.drum_target = first.drum_target;
    result.detach_drum_link();
    result.props = first.props;
    result.channel = first.channel;
    result.muted_notes = muted_notes;
    Ok(result)
}
