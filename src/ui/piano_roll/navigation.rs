//! Vista musical del piano roll: escala horizontal y desplazamiento de tiempo/registro.
use super::geometry::{FIRST_PITCH, Grid, LAST_PITCH};
use crate::ui::{
    DawApp,
    navigation::{Navigation, pinch_factor, scroll_navigation},
};
use gpui::{Context, PinchEvent, Pixels, Point, ScrollEvent, px};

#[derive(Clone, Copy)]
pub(in crate::ui) struct RollViewport {
    pub zoom: f64,
    pub vertical_zoom: f32,
    pub first_beat: f64,
    pub top_pitch: f32,
}

impl Default for RollViewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            vertical_zoom: 1.0,
            first_beat: 0.0,
            top_pitch: f32::from(LAST_PITCH),
        }
    }
}

impl RollViewport {
    /// Encuadre cómodo por defecto: muestra ~4 compases (4 acordes) a lo ancho y ~1.8 octavas
    /// centradas en el registro del clip para evitar tener que hacer zoom al abrir.
    pub fn for_clip(
        clip_beats: f64,
        bar_beats: f64,
        notes: &[elevate::musical::model::NoteEvent],
    ) -> Self {
        let target_beats = (bar_beats.max(1.0) * 4.0).max(4.0);
        let zoom = if clip_beats.is_finite() && clip_beats > target_beats {
            (clip_beats / target_beats).clamp(1.0, 64.0)
        } else {
            1.0
        };
        let total_rows = f32::from(LAST_PITCH - FIRST_PITCH + 1);
        let (vertical_zoom, top_pitch) = if let (Some(min_p), Some(max_p)) = (
            notes.iter().map(|n| n.pitch).min(),
            notes.iter().map(|n| n.pitch).max(),
        ) {
            let span = f32::from(max_p.saturating_sub(min_p) + 1);
            let desired_rows = (span + 8.0).clamp(18.0, 28.0);
            let vzoom = (total_rows / desired_rows).clamp(1.0, 4.0);
            let center = (f32::from(min_p) + f32::from(max_p)) * 0.5;
            let top = center + desired_rows * 0.5;
            (vzoom, top)
        } else {
            // Registro cómodo para acordes (C3..E5 aprox., ~22 filas legibles)
            let desired_rows = 22.0_f32;
            ((total_rows / desired_rows).clamp(1.0, 4.0), 76.0)
        };
        let mut viewport = Self {
            zoom,
            vertical_zoom,
            first_beat: 0.0,
            top_pitch,
        };
        viewport.top_pitch = viewport.bounded_top_pitch();
        viewport
    }

    /// Super controla la altura; Ctrl controla el ancho. El resto conserva la
    /// navegación común, incluidos Ctrl+rueda y el desplazamiento con dos dedos.
    pub fn scroll(&mut self, grid: Grid, beats: f64, event: &ScrollEvent) {
        if event.modifiers.platform {
            self.zoom_vertical_at(grid, event.position, axis_zoom_factor(event, -1.0));
        } else {
            match scroll_navigation(event) {
                Navigation::Zoom(factor) => self.zoom_at(grid, event.position, beats, factor),
                Navigation::Pan { x, y } => self.pan(grid, beats, x, y),
            }
        }
    }

    pub fn visible_rows(&self) -> f32 {
        f32::from(LAST_PITCH - FIRST_PITCH + 1) / self.vertical_zoom
    }

    pub fn row_height(&self, height: f32) -> f32 {
        height / self.visible_rows()
    }

    pub fn bounded_top_pitch(&self) -> f32 {
        self.top_pitch
            .clamp((self.visible_rows() - 1.0).clamp(0.0, 127.0), 127.0)
    }

    /// Escala solo el registro visible, conservando la altura musical bajo el puntero.
    pub fn zoom_vertical_at(&mut self, grid: Grid, position: Point<Pixels>, factor: f64) {
        let height = f32::from(grid.notes.size.height);
        if !factor.is_finite() || factor <= 0.0 || height <= 0.0 {
            return;
        }
        let y = f32::from(position.y - grid.notes.top()).clamp(0.0, height);
        if !y.is_finite() {
            return;
        }
        let anchor = self.bounded_top_pitch() - y / self.row_height(height);
        let min_zoom = f32::from(LAST_PITCH - FIRST_PITCH + 1) / 128.0;
        self.vertical_zoom =
            (f64::from(self.vertical_zoom) * factor).clamp(f64::from(min_zoom), 8.0) as f32;
        self.top_pitch = anchor + y / self.row_height(height);
        self.top_pitch = self.bounded_top_pitch();
    }

    pub fn zoom_at(&mut self, grid: Grid, position: Point<Pixels>, beats: f64, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 || grid.pixels_per_beat <= 0.0 {
            return;
        }
        let x = f64::from(f32::from(position.x - grid.notes.left()))
            .clamp(0.0, f64::from(f32::from(grid.notes.size.width)));
        // Varios eventos pueden llegar antes del siguiente frame: usar el estado
        // actual, no el desplazamiento/escala del último canvas pintado.
        let current_scale = f64::from(f32::from(grid.notes.size.width)) / beats * self.zoom;
        let anchor = self.first_beat + x / current_scale;
        let zoom = (self.zoom * factor).clamp(1.0, 64.0);
        let scale = current_scale * zoom / self.zoom;
        self.zoom = zoom;
        self.first_beat = (anchor - x / scale).clamp(0.0, (beats - beats / zoom).max(0.0));
    }

    pub fn pan(&mut self, grid: Grid, beats: f64, x: f64, y: f64) {
        if grid.pixels_per_beat > 0.0 {
            let scale = f64::from(f32::from(grid.notes.size.width)) / beats * self.zoom;
            self.first_beat =
                (self.first_beat - x / scale).clamp(0.0, (beats - beats / self.zoom).max(0.0));
        }
        let row_height = self.row_height(f32::from(grid.notes.size.height));
        if row_height > 0.0 {
            self.top_pitch = self.bounded_top_pitch() + y as f32 / row_height;
            self.top_pitch = self.bounded_top_pitch();
        }
    }
}

impl DawApp {
    /// Sigue la posición leída del motor; nunca programa notas ni avanza el reloj.
    pub(in crate::ui) fn follow_roll_playhead(&mut self) {
        if !self.piano_roll.open || !self.piano_roll.follow || self.piano_roll.drag.is_some() {
            return;
        }
        let Some(clip) = self.timeline.midi_clip.as_ref() else {
            return;
        };
        let Some(beat) = self.timeline.clip_visual_playhead(clip) else {
            return;
        };
        let beats = clip.pattern_beats();
        let viewport = &mut self.piano_roll.viewport;
        let visible = beats / viewport.zoom;
        if beat < viewport.first_beat || beat >= viewport.first_beat + visible * 0.85 {
            viewport.first_beat = (beat - visible * 0.15).clamp(0.0, (beats - visible).max(0.0));
        }
    }

    pub(in crate::ui) fn scroll_piano_roll(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        if self.action_form.is_some() || self.command_open {
            return;
        }
        if event.modifiers.alt && !event.modifiers.platform && !event.modifiers.control {
            let delta = event.delta.pixel_delta(px(24.0));
            let amount = if delta.y.abs() >= delta.x.abs() {
                delta.y
            } else {
                delta.x
            };
            if self
                .piano_roll
                .split_gesture
                .wheel(f32::from(amount), event.touch_phase)
                && self.script_task.is_none()
            {
                self.execute_script_command("midi.subdivide", cx);
            }
            return;
        }
        self.piano_roll.split_gesture = Default::default();
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        self.piano_roll
            .viewport
            .scroll(grid, clip.pattern_beats(), event);
    }

    pub(in crate::ui) fn pinch_piano_roll(&mut self, event: &PinchEvent, cx: &mut Context<Self>) {
        if self.action_form.is_some() || self.command_open {
            return;
        }
        if event.modifiers.shift
            && !event.modifiers.control
            && !event.modifiers.platform
            && !event.modifiers.alt
        {
            if self
                .piano_roll
                .split_gesture
                .pinch(event.delta, event.phase)
                && self.script_task.is_none()
            {
                self.execute_script_command("midi.subdivide", cx);
            }
            return;
        }
        self.piano_roll.split_gesture = Default::default();
        if let Some(clip) = &self.timeline.midi_clip
            && let Some(factor) = pinch_factor(event)
        {
            self.piano_roll.viewport.zoom_at(
                self.piano_roll.grid.get(),
                event.position,
                clip.pattern_beats(),
                factor,
            );
        }
    }
}

fn axis_zoom_factor(event: &ScrollEvent, direction: f64) -> f64 {
    let delta = f64::from(f32::from(event.delta.pixel_delta(px(24.0)).y));
    if !delta.is_finite() {
        return 1.0;
    }
    (direction * delta * 0.005).clamp(-2.0, 2.0).exp()
}
