//! Recorte no destructivo de clips arrastrando sus bordes. El archivo y el patrón no se tocan:
//! solo cambian `trim_start`/`trim_len`, así el clip se puede volver a extender.
use super::{DragKind, Timeline};
use crate::ui::time_grid::TimeSpan;
use gpui::{Pixels, Point};

/// Zona sensible junto a cada borde, en píxeles.
pub(super) const EDGE_PX: f64 = 7.0;
/// Longitud mínima visible de un clip.
const MIN_BEATS: f64 = 0.25;

#[derive(Clone, Copy)]
pub(super) struct TrimDrag {
    pub(super) kind: DragKind,
    left: bool,
    /// Posición en el timeline del borde izquierdo al empezar.
    start0: f64,
    /// Recorte inicial al inicio y longitud visible, en beats.
    trim0: f64,
    len0: f64,
    /// Longitud total de la fuente (archivo o patrón) en beats.
    source: f64,
    /// Si está en bucle interno, el borde derecho se puede estirar más allá de 1 ciclo.
    looped: bool,
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
        let beats_per_second = self.tempo.beats_per_second();
        let beat = self.pointer_beat(position);
        let bounds = self.lanes_bounds.get();
        let y = f32::from(position.y - bounds.top());
        let Some(row) = self.row_at_y(y, track_height) else {
            return false;
        };
        let track = row.track;
        let candidate = if let Some(c) = &self.midi_clip
            && track == c.track
            && self.hits_layer(row, y, c.id)
            && beat >= c.start_beat
            && beat < c.end_beat()
        {
            (!c.props.locked).then_some((
                DragKind::Midi,
                c.start_beat,
                c.trim_start,
                c.duration_beats(),
                c.pattern_beats(),
                c.props.looped,
            ))
        } else if let Some(c) = &self.clip
            && track == c.track
            && self.hits_layer(row, y, c.id)
            && beat >= c.start_beat
            && beat < c.end_beat(self.tempo.bpm())
        {
            (!c.props.locked).then_some((
                DragKind::Audio,
                c.start_beat,
                c.trim_start * beats_per_second,
                c.length_beats(self.tempo.bpm()),
                c.file.duration() * beats_per_second,
                c.props.looped,
            ))
        } else {
            None
        };
        let Some((kind, start0, trim0, len0, source, looped)) = candidate else {
            return false;
        };
        let edge = self.viewport.grid().edge_beats(EDGE_PX, len0);
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
            looped,
            grab_beat: beat,
        });
        true
    }

    /// Aplica el recorte según el puntero. Devuelve la pista del clip.
    pub(super) fn apply_trim(&mut self, position: Point<Pixels>) -> Option<usize> {
        let beats_per_second = self.tempo.beats_per_second();
        let drag = self.trim_drag?;
        let step = self.viewport.grid_step();
        let delta = self.pointer_beat(position) - drag.grab_beat;
        let grid = self.viewport.grid();
        let span = TimeSpan {
            start: drag.start0,
            end: drag.start0 + drag.len0,
        };
        let (start, trim, len) = if drag.left {
            let lowest = (drag.start0 - drag.trim0).max(0.0);
            let resized = grid.resize_start(span, delta, lowest, MIN_BEATS, step);
            let shift = resized.start - drag.start0;
            (resized.start, drag.trim0 + shift, resized.length())
        } else {
            let longest = if drag.looped {
                2048.0_f64.max(drag.len0 + 256.0)
            } else {
                (drag.source - drag.trim0).max(MIN_BEATS)
            };
            let resized = grid.resize_end(span, delta, drag.start0 + longest, MIN_BEATS, step);
            (drag.start0, drag.trim0, resized.length())
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
                clip.trim_start = trim / beats_per_second;
                clip.trim_len = Some(len / beats_per_second);
                Some(clip.track)
            }
        }
    }
}
