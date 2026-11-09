//! Recorte no destructivo de clips arrastrando sus bordes. El archivo y el patrón no se tocan:
//! solo cambian `trim_start`/`trim_len`, así el clip se puede volver a extender.
use super::{DragKind, Timeline};
use crate::audio::BEATS_PER_SECOND;
use gpui::{Pixels, Point};

/// Zona sensible junto a cada borde, en píxeles.
const EDGE_PX: f64 = 7.0;
/// Longitud mínima visible de un clip.
const MIN_BEATS: f64 = 0.25;

#[derive(Clone, Copy)]
pub(super) struct TrimDrag {
    kind: DragKind,
    left: bool,
    /// Posición en el timeline del borde izquierdo al empezar.
    start0: f64,
    /// Recorte inicial al inicio y longitud visible, en beats.
    trim0: f64,
    len0: f64,
    /// Longitud total de la fuente (archivo o patrón) en beats.
    source: f64,
    grab_beat: f64,
}

impl Timeline {
    fn pointer_beat(&self, position: Point<Pixels>) -> f64 {
        let bounds = self.lanes_bounds.get();
        self.viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())))
    }

    /// Inicia un recorte si el clic cae junto al borde del clip activo. Devuelve `true` si lo inició.
    pub(super) fn begin_trim(&mut self, position: Point<Pixels>, track_height: f32) -> bool {
        let beat = self.pointer_beat(position);
        let bounds = self.lanes_bounds.get();
        let Some(row) = self.row_at_y(f32::from(position.y - bounds.top()), track_height) else {
            return false;
        };
        let track = row.track;
        let candidate = if let Some(c) = &self.midi_clip
            && track == c.track
            && beat >= c.start_beat
            && beat < c.end_beat()
        {
            (!c.props.locked).then_some((
                DragKind::Midi,
                c.start_beat,
                c.trim_start,
                c.duration_beats(),
                c.pattern_beats(),
            ))
        } else if let Some(c) = &self.clip
            && track == c.track
            && beat >= c.start_beat
            && beat < c.end_beat()
        {
            (!c.props.locked).then_some((
                DragKind::Audio,
                c.start_beat,
                c.trim_start * BEATS_PER_SECOND,
                c.length_beats(),
                c.file.duration() * BEATS_PER_SECOND,
            ))
        } else {
            None
        };
        let Some((kind, start0, trim0, len0, source)) = candidate else {
            return false;
        };
        let edge = (EDGE_PX / self.viewport.pixels_per_beat).min(len0 / 3.0);
        let left = beat - start0 <= edge;
        let right = start0 + len0 - beat <= edge;
        if !left && !right {
            return false;
        }
        self.trim_drag = Some(TrimDrag {
            kind,
            left,
            start0,
            trim0,
            len0,
            source,
            grab_beat: beat,
        });
        true
    }

    /// Aplica el recorte según el puntero. Devuelve la pista del clip.
    pub(super) fn apply_trim(&mut self, position: Point<Pixels>) -> Option<usize> {
        let drag = self.trim_drag?;
        let step = self.viewport.grid_step();
        let snap = |beat: f64| (beat / step).round() * step;
        let delta = self.pointer_beat(position) - drag.grab_beat;
        let (start, trim, len) = if drag.left {
            let lowest = (drag.start0 - drag.trim0).max(0.0);
            let highest = drag.start0 + drag.len0 - MIN_BEATS;
            let start = snap(drag.start0 + delta).clamp(lowest, highest.max(lowest));
            let shift = start - drag.start0;
            (start, drag.trim0 + shift, drag.len0 - shift)
        } else {
            let longest = (drag.source - drag.trim0).max(MIN_BEATS);
            let end = snap(drag.start0 + drag.len0 + delta);
            let len = (end - drag.start0).clamp(MIN_BEATS, longest);
            (drag.start0, drag.trim0, len)
        };
        match drag.kind {
            DragKind::Midi => {
                let clip = self.midi_clip.as_mut()?;
                clip.start_beat = start;
                clip.trim_start = trim;
                clip.trim_len = Some(len);
                Some(clip.track)
            }
            DragKind::Audio => {
                let clip = self.clip.as_mut()?;
                clip.start_beat = start;
                clip.trim_start = trim / BEATS_PER_SECOND;
                clip.trim_len = Some(len / BEATS_PER_SECOND);
                Some(clip.track)
            }
        }
    }
}
