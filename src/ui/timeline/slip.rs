//! Slip compartido de audio/MIDI: desplaza la ventana de fuente, no el clip.
use super::{DragKind, Timeline, clip_frame};
use gpui::{Pixels, Point, px};

#[derive(Clone, Copy)]
pub(super) struct SlipDrag {
    kind: DragKind,
    pointer: Point<Pixels>,
    source_offset: f64,
    visible_length: f64,
    source_length: f64,
    moved: bool,
}
impl Timeline {
    pub(crate) fn begin_slip(&mut self, position: Point<Pixels>, height: f32) -> Option<usize> {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return None;
        }
        let row = self.row_at_y(f32::from(position.y - bounds.top()), height)?;
        // Mantener disponibles los controles de la cabecera.
        if f32::from(position.y - bounds.top()) - row.top
            <= clip_frame::CONTROL_TOP + clip_frame::CONTROL_HEIGHT
        {
            return None;
        }
        let track = row.track;
        let midi = self.activate_midi_clip_at(position, height);
        if !midi && !self.activate_audio_clip_at(position, height) {
            return None;
        }
        let bps = self.tempo.beats_per_second();
        let (kind, offset, length, source) = if midi {
            let clip = self.midi_clip.as_ref().filter(|c| !c.props.locked)?;
            (
                DragKind::Midi,
                clip.trim_start,
                clip.duration_beats(),
                clip.pattern_beats(),
            )
        } else {
            let clip = self.clip.as_ref().filter(|c| !c.props.locked)?;
            (
                DragKind::Audio,
                clip.trim_start * bps,
                clip.length_seconds() * bps,
                clip.file.duration() * bps,
            )
        };
        self.clip_drag = None;
        self.trim_drag = None;
        self.slip_drag = Some(SlipDrag {
            kind,
            pointer: position,
            source_offset: offset,
            visible_length: length,
            source_length: source,
            moved: false,
        });
        if midi {
            self.select_active_midi(false);
        }
        Some(track)
    }
    pub(super) fn apply_slip(&mut self, position: Point<Pixels>) -> Option<usize> {
        let drag = self.slip_drag?;
        let dx = f64::from(f32::from(position.x - drag.pointer.x));
        if !drag.moved && (position.x - drag.pointer.x).abs() < px(3.) {
            return None;
        }
        if let Some(state) = &mut self.slip_drag {
            state.moved = true;
        }
        // El contenido sigue el ratón; hacia la derecha revela una parte anterior.
        // Movimiento continuo, sin cuantizar y siempre relativo al gesto original.
        let offset = (drag.source_offset - dx / self.viewport.pixels_per_beat)
            .clamp(0.0, (drag.source_length - drag.visible_length).max(0.0));
        match drag.kind {
            DragKind::Midi => {
                let clip = self.midi_clip.as_mut().filter(|c| !c.props.locked)?;
                if (clip.trim_start - offset).abs() > 1e-9 {
                    clip.trim_start = offset;
                    clip.trim_len = Some(drag.visible_length);
                }
                Some(clip.track)
            }
            DragKind::Audio => {
                let bps = self.tempo.beats_per_second();
                let clip = self.clip.as_mut().filter(|c| !c.props.locked)?;
                if (clip.trim_start - offset / bps).abs() > 1e-9 {
                    clip.trim_start = offset / bps;
                    clip.trim_len = Some(drag.visible_length / bps);
                }
                Some(clip.track)
            }
        }
    }
    pub(crate) fn slipping(&self) -> bool {
        self.slip_drag.is_some()
    }
    pub(crate) fn slip_has_room(&self) -> bool {
        self.slip_drag
            .is_some_and(|drag| drag.source_length - drag.visible_length > 1e-9)
    }
}
