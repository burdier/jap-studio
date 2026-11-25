//! Visual compartido sin dependencia del motor: espectro, onda, espectrogramas y nivel.
mod plot;
use gpui::{App, Div, MouseButton, MouseDownEvent, Window, div, prelude::*, px, rgb};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ui) enum SpectrumMode {
    #[default]
    Spectrum,
    Visualizer,
    SpectrogramLog,
    SpectrogramLinear,
    Level,
}
impl SpectrumMode {
    pub fn next(self) -> Self {
        match self {
            Self::Spectrum => Self::Visualizer,
            Self::Visualizer => Self::SpectrogramLog,
            Self::SpectrogramLog => Self::SpectrogramLinear,
            Self::SpectrogramLinear => Self::Level,
            Self::Level => Self::Spectrum,
        }
    }
    pub fn previous(self) -> Self {
        let mut mode = self.next();
        while mode.next() != self {
            mode = mode.next();
        }
        mode
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Spectrum => "ESPECTRO",
            Self::Visualizer => "ONDA",
            Self::SpectrogramLog => "ESPECTROGRAMA · LOG",
            Self::SpectrogramLinear => "ESPECTROGRAMA · LINEAL",
            Self::Level => "NIVEL · PEAK / RMS",
        }
    }
}

/// Datos preparados por el consumidor; puede recibir audio, archivos o señales sintéticas.
#[derive(Default)]
pub(in crate::ui) struct SpectrumData {
    pub spectrum: Vec<f32>,
    pub wave_min: Vec<f32>,
    pub wave_max: Vec<f32>,
    pub history: Vec<f32>,
    pub history_bins: usize,
    pub history_capacity: usize,
    pub seconds: f32,
    pub upper_hz: f32,
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub guide_hz: Option<f32>,
    pub color: u32,
    pub hot: u32,
    pub background: u32,
    pub border: u32,
    pub muted: u32,
    pub width: Option<f32>,
    pub height: f32,
}

pub(in crate::ui) fn spectrum_graph(data: SpectrumData) -> impl IntoElement {
    plot::graph(data, SpectrumMode::Spectrum)
}

pub(in crate::ui) fn spectrum_graph_mode(
    data: SpectrumData,
    mode: SpectrumMode,
) -> impl IntoElement {
    plot::graph(data, mode)
}

pub(in crate::ui) fn spectrum_panel(
    data: SpectrumData,
    mode: SpectrumMode,
    previous: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    next: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let header = div()
        .h(px(23.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .px_2()
        .gap_1()
        .border_b_1()
        .border_color(rgb(data.border))
        .text_size(px(10.))
        .text_color(rgb(data.muted))
        .child(div().overflow_hidden().child(mode.label()))
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .gap_1()
                .child(
                    div()
                        .w(px(20.))
                        .text_center()
                        .cursor_pointer()
                        .text_color(rgb(data.color))
                        .child("<")
                        .on_mouse_down(MouseButton::Left, previous),
                )
                .child(
                    div()
                        .w(px(20.))
                        .text_center()
                        .cursor_pointer()
                        .text_color(rgb(data.color))
                        .child(">")
                        .on_mouse_down(MouseButton::Left, next),
                ),
        );
    let footer = match mode {
        SpectrumMode::Spectrum => format!(
            "20 Hz                {:.0}k Hz · −80…0 dBFS",
            data.upper_hz / 1000.
        ),
        SpectrumMode::Visualizer => "−1                        0                        +1".into(),
        SpectrumMode::SpectrogramLog | SpectrumMode::SpectrogramLinear => format!(
            "−{:.1} s → ahora · {}–{:.0}k Hz ↑",
            data.seconds,
            if mode == SpectrumMode::SpectrogramLog {
                20
            } else {
                0
            },
            data.upper_hz / 1000.
        ),
        SpectrumMode::Level => {
            format!("L {:.1} / R {:.1} dBFS", db(data.peak[0]), db(data.peak[1]))
        }
    };
    let mut panel = div()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .border_1()
        .border_color(rgb(data.border))
        .bg(rgb(data.background));
    panel = if let Some(width) = data.width {
        panel.w(px(width))
    } else {
        panel.w_full()
    };
    let footer = div()
        .px_1()
        .h(px(16.))
        .text_size(px(9.))
        .text_color(rgb(data.muted))
        .overflow_hidden()
        .child(footer);
    panel
        .child(header)
        .child(plot::graph(data, mode))
        .child(footer)
}
fn db(value: f32) -> f32 {
    20. * value.max(1e-4).log10()
}
