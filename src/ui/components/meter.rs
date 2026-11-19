//! Medidores verticales para el Mixer y el Arrangement.
use crate::ui::{DesignSystem, design::blend_color};
use gpui::{Div, div, prelude::*, px, relative, rgb};

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

/// Medidor angosto del bus de pista. Sin una fuente preparada queda en reposo.
pub(in crate::ui) fn track_peak_meter(ds: &DesignSystem, peak: f32) -> Div {
    let floor = -48.0;
    let normalized = |db: f32| (db - floor) / -floor;
    let level = normalized(20.0 * peak.max(0.000_001).log10()).clamp(0.0, 1.0);
    let amber = normalized(-12.0);
    let red = normalized(-1.0);
    let ranges = [
        (0.0, amber, blend_color(ds.success, 0x75c98c, 65)),
        (amber, red, ds.warning),
        (red, 1.0, ds.danger),
    ];
    div()
        .absolute()
        .right(px(4.0))
        .top(px(4.0))
        .bottom(px(4.0))
        .w(px(5.0))
        .bg(rgb(ds.panel_dark))
        .rounded(px(1.5))
        .overflow_hidden()
        .children(
            ranges
                .into_iter()
                .filter(|(start, _, _)| level > *start)
                .map(|(start, end, color)| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(start))
                        .h(relative(level.min(end) - start))
                        .bg(rgb(color))
                }),
        )
}
