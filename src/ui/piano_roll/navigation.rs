//! Vista musical del piano roll: escala horizontal y desplazamiento de tiempo/registro.
use super::geometry::{FIRST_PITCH, Grid, LAST_PITCH};
use crate::ui::{
    DawApp,
    navigation::{Navigation, pinch_factor, scroll_navigation},
};
use gpui::{PinchEvent, Pixels, Point, ScrollEvent, px};

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
    /// Super controla la altura; Alt controla el ancho. El resto conserva la
    /// navegación común, incluidos Ctrl+rueda y el desplazamiento con dos dedos.
    pub fn scroll(&mut self, grid: Grid, beats: f64, event: &ScrollEvent) {
        if event.modifiers.platform {
            self.zoom_vertical_at(grid, event.position, axis_zoom_factor(event, -1.0));
        } else if event.modifiers.alt {
            self.zoom_at(grid, event.position, beats, axis_zoom_factor(event, 1.0));
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
    pub(in crate::ui) fn scroll_piano_roll(&mut self, event: &ScrollEvent) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        let grid = self.piano_roll.grid.get();
        self.piano_roll
            .viewport
            .scroll(grid, clip.pattern_beats(), event);
    }

    pub(in crate::ui) fn pinch_piano_roll(&mut self, event: &PinchEvent) {
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
