//! Controlador/vista del dispositivo; el instrumento y sus voces permanecen fuera de GPUI.
mod audio;
mod controls;
mod input;
mod view;
mod waveform;

use gpui::{Pixels, Task};
#[derive(Default)]
pub(super) struct SamplerUi {
    pub busy: bool,
    pub pending: bool,
    pub task: Option<Task<()>>,
    pub picker: Option<Task<()>>,
    pub drag: Option<(usize, Pixels, f32)>,
    pub wave_drag: Option<(usize, gpui::Bounds<Pixels>)>,
}
