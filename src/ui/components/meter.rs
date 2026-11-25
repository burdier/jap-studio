//! Medidores verticales para el Mixer y el Arrangement.
use crate::ui::{DesignSystem, design::blend_color};
use gpui::{Div, div, prelude::*, px, relative, rgb, rgba};

/// Medidor angosto del bus de pista (TCP del Arrangement).
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

const METER_FLOOR_DB: f32 = -60.0;
const METER_CEIL_DB: f32 = 3.0;

fn db_fraction(linear: f32) -> f32 {
    if linear <= 1e-6 {
        return 0.0;
    }
    let db = 20.0 * linear.log10();
    ((db - METER_FLOOR_DB) / (METER_CEIL_DB - METER_FLOOR_DB)).clamp(0.0, 1.0)
}

fn db_mark_fraction(db: f32) -> f32 {
    ((db - METER_FLOOR_DB) / (METER_CEIL_DB - METER_FLOOR_DB)).clamp(0.0, 1.0)
}

/// Medidor estéreo L/R de estudio para el Mixer: Peak + RMS + Peak Hold + escala dBFS/dBTP.
pub(in crate::ui) fn stereo_mixer_meter(
    ds: &DesignSystem,
    peak: [f32; 2],
    rms: [f32; 2],
    hold: f32,
    show_scale: bool,
) -> Div {
    let green_hi = db_mark_fraction(-18.0);
    let yellow_hi = db_mark_fraction(-6.0);
    let zero_db = db_mark_fraction(0.0);
    let hold_frac = db_fraction(hold);
    let clipped = hold >= 1.0;

    let ranges = [
        (0.0, green_hi, blend_color(ds.success, 0x52d68a, 50)),
        (green_hi, yellow_hi, blend_color(ds.success, ds.warning, 45)),
        (yellow_hi, zero_db, ds.warning),
        (zero_db, 1.0, ds.danger),
    ];

    let bar = |p: f32, r: f32| {
        let p_frac = db_fraction(p);
        let r_frac = db_fraction(r).min(p_frac);
        div()
            .relative()
            .w(px(7.0))
            .h_full()
            .bg(rgb(ds.panel_dark))
            .rounded(px(1.5))
            .overflow_hidden()
            .children(
                ranges
                    .into_iter()
                    .filter(|(start, _, _)| p_frac > *start)
                    .map(|(start, end, color)| {
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(relative(start))
                            .h(relative(p_frac.min(end) - start))
                            .bg(rgba((color << 8) | 0xb8))
                    }),
            )
            .when(r_frac > 0.005, |el| {
                el.child(
                    div()
                        .absolute()
                        .left(px(1.5))
                        .right(px(1.5))
                        .bottom_0()
                        .h(relative(r_frac))
                        .bg(rgba((ds.text << 8) | 0x45)),
                )
            })
            .when(hold_frac > 0.005, |el| {
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(hold_frac.min(0.985)))
                        .h(px(2.0))
                        .bg(rgb(if clipped { ds.danger } else { ds.text })),
                )
            })
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(relative(zero_db))
                    .h(px(1.0))
                    .bg(rgba((ds.danger << 8) | 0x66)),
            )
    };

    let mut container = div()
        .h_full()
        .py(px(4.0))
        .flex()
        .items_stretch()
        .gap(px(2.0))
        .flex_shrink_0()
        .child(bar(peak[0], rms[0]))
        .child(bar(peak[1], rms[1]));

    if show_scale {
        let ticks = [0.0_f32, -6.0, -12.0, -24.0, -48.0];
        container = container.child(
            div()
                .relative()
                .w(px(22.0))
                .h_full()
                .flex_shrink_0()
                .children(ticks.into_iter().map(|db| {
                    let frac = db_mark_fraction(db);
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(frac))
                        .mb(px(-4.5))
                        .flex()
                        .items_center()
                        .gap(px(2.0))
                        .child(div().w(px(3.0)).h(px(1.0)).bg(rgb(if db == 0.0 {
                            ds.danger
                        } else {
                            ds.border
                        })))
                        .child(
                            div()
                                .text_size(px(8.5))
                                .text_color(rgb(if db == 0.0 { ds.danger } else { ds.muted }))
                                .child(format!("{db:.0}")),
                        )
                })),
        );
    }

    container
}

pub(in crate::ui) fn format_peak_db(linear: f32) -> String {
    if linear <= 1e-4 {
        "−∞".into()
    } else {
        let db = 20.0 * linear.log10();
        if db >= 0.0 {
            format!("+{db:.1}")
        } else {
            format!("{db:.1}")
        }
    }
}
