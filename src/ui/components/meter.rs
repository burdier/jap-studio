//! Medidores verticales segmentados (LED) y barras horizontales reutilizables.
use crate::ui::DesignSystem;
use gpui::{Div, div, prelude::*, px, rgb};

const DEFAULT_SEGMENTS: i32 = 10;

/// Medidor vertical de segmentos LED (usado en canales del Mixer y Master).
pub(in crate::ui) fn vertical_meter(
    ds: &DesignSystem,
    filled_segments: i32,
    color: u32,
    border: u32,
) -> Div {
    let filled = filled_segments.clamp(0, DEFAULT_SEGMENTS);
    div()
        .flex_1()
        .flex()
        .flex_col()
        .justify_end()
        .gap_1()
        .p_2()
        .bg(rgb(ds.panel))
        .rounded(px(ds.radius_sm))
        .border_1()
        .border_color(rgb(border))
        .children((0..DEFAULT_SEGMENTS).rev().map(|bar| {
            div()
                .w_full()
                .h(px(12.0))
                .rounded(px(1.0))
                .bg(rgb(if bar < filled { color } else { ds.border }))
        }))
}

/// Barra horizontal compacta de progreso/nivel (modo `Bars` de Pan/Vol en cabeceras TCP).
pub(in crate::ui) fn horizontal_bar(
    ds: &DesignSystem,
    fraction: f32,
    width: f32,
    color: u32,
) -> Div {
    let frac = fraction.clamp(0.0, 1.0);
    div()
        .w(px(width))
        .h(px(ds.control_bar_height))
        .bg(rgb(ds.panel_dark))
        .rounded(px(ds.radius_sm))
        .child(
            div()
                .w(px(frac * width))
                .h_full()
                .bg(rgb(color))
                .rounded(px(ds.radius_sm)),
        )
}
