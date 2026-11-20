//! Enlaces de clips de percusión, proyección por pieza y plan absoluto del Arrangement.
use super::{DrumKitId, DrumPattern, DrumPatternId, DrumSession};
use crate::musical::{
    clip::MidiClip,
    model::{InstrumentId, Pattern},
    time::{PPQ, Tick},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrumClipTarget {
    pub kit: DrumKitId,
    pub piece: InstrumentId,
    pub pattern: DrumPatternId,
    pub linked: bool,
}

impl DrumClipTarget {
    pub fn resolve(self, session: &DrumSession) -> Result<Option<&DrumPattern>, String> {
        if self.kit != session.kit.id
            || !session
                .kit
                .pieces
                .iter()
                .any(|piece| piece.id == self.piece)
        {
            return Err("Clip de batería sin kit o pieza".into());
        }
        self.linked
            .then(|| {
                session
                    .pattern(self.pattern)
                    .ok_or_else(|| "Clip de batería sin patrón fuente".into())
            })
            .transpose()
    }
}

/// Las notas proyectadas conservan identidad; su duración es la del patrón completo.
pub fn piece_pattern(pattern: &Pattern, piece: InstrumentId) -> Result<Pattern, String> {
    let notes = pattern
        .notes()
        .iter()
        .filter(|note| note.instrument == piece)
        .copied()
        .collect();
    pattern
        .with_notes(notes, pattern.next_id())
        .map_err(|error| error.to_string())
}

pub fn arrangement_pattern<'a>(
    clips: impl IntoIterator<Item = &'a MidiClip>,
    mut audible: impl FnMut(&MidiClip) -> bool,
) -> Result<Option<Pattern>, String> {
    let mut events = Vec::new();
    let mut end = 0_i64;
    for clip in clips {
        let Some(target) = clip.drum_target else {
            continue;
        };
        if clip.duration_beats() <= 0.0 {
            continue;
        }
        let origin = ticks(clip.start_beat)?;
        let duration = ticks(clip.duration_beats())?;
        if duration == 0 {
            continue;
        }
        let clip_end = origin
            .checked_add(duration)
            .ok_or("Posición MIDI fuera de rango")?;
        end = end.max(clip_end);
        if clip.props.muted || !audible(clip) {
            continue;
        }
        let window = clip.playback_pattern();
        for note in window.notes() {
            let mut event = *note;
            event.id.0 = events.len() as u64 + 1;
            event.start.0 = origin
                .checked_add(note.start.0)
                .ok_or("Nota fuera de rango")?;
            event.duration.0 = event.duration.0.min(clip_end - event.start.0);
            if event.duration.0 <= 0 {
                continue;
            }
            if events.len() >= 32_768 {
                return Err("Plan de batería excede 65536 eventos".into());
            }
            event.instrument = target.piece;
            event.pitch = (i16::from(event.pitch) + i16::from(clip.props.pitch_semitones))
                .clamp(0, 127) as u8;
            events.push(event);
        }
    }
    if end == 0 {
        return Ok(None);
    }
    Pattern::restore(Tick(end), events)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn ticks(beat: f64) -> Result<i64, String> {
    let value = (beat * PPQ as f64).round();
    if !value.is_finite() || value < 0.0 || value >= i64::MAX as f64 {
        return Err("Posición MIDI fuera de rango".into());
    }
    Ok(value as i64)
}
