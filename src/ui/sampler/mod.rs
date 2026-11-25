//! Controlador/vista del dispositivo; el instrumento y sus voces permanecen fuera de GPUI.
mod audio;
mod controls;
mod hilo;
mod input;
mod view;
mod waveform;

use crate::ui::timeline::DragKind;
use gpui::{Bounds, Pixels, Task};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum WaveHandle {
    Start,
    End,
    LoopStart,
    LoopEnd,
    SlideRange,
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct WaveDrag {
    pub handle: WaveHandle,
    pub bounds: Bounds<Pixels>,
    pub anchor_pct: f32,
    pub initial_start: f32,
    pub initial_end: f32,
}

#[derive(Default)]
pub(super) struct SamplerUi {
    pub busy: bool,
    pub pending: bool,
    pub task: Option<Task<()>>,
    pub picker: Option<Task<()>>,
    pub drag: Option<(usize, Pixels, f32)>,
    pub wave_drag: Option<WaveDrag>,
    /// Rango visible en porcentaje `(v0, v1)` dentro de `0..=100`; `None` = vista completa.
    pub wave_zoom: Option<(f32, f32)>,
    pub wave_hover: Option<f32>,
    /// Banda activa en la curva combinada: 0 = Filtro, 1 = Low, 2 = Mid, 3 = High.
    pub eq_band: u8,
    /// Área en pantalla del panel de Hay Sampler para soltar clips del Arrangement.
    pub panel_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Tipo de clip del Arrangement que sobrevuela el panel de Hay Sampler.
    pub drop_hover: Option<DragKind>,
}

impl SamplerUi {
    pub fn visible_range(&self) -> (f32, f32) {
        self.wave_zoom
            .filter(|(a, b)| b - a >= 0.5)
            .unwrap_or((0.0, 100.0))
    }
}
