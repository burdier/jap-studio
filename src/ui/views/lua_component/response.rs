//! Respuesta de una primitiva indicada por Lua; no es un analizador de señal.
use crate::ui::DesignSystem;
use elevate::{primitives::filter::Filter, scripting::audio_components::AudioComponent};
use gpui::{IntoElement, PathBuilder, canvas, div, point, prelude::*, px, rgb};
use std::collections::BTreeMap;
pub(super) fn plot(
    ds: &DesignSystem,
    descriptor: &AudioComponent,
    parameters: &BTreeMap<u32, f32>,
    node: usize,
    rate: f64,
) -> impl IntoElement + use<> {
    let mut values = [
        descriptor.nodes[node].values[0],
        descriptor.nodes[node].values[1],
        descriptor.nodes[node].values[2],
    ];
    for control in &descriptor.controls {
        for &(target, parameter) in &control.targets {
            if target == node {
                values[parameter.0 as usize] = parameters
                    .get(&control.id)
                    .copied()
                    .unwrap_or(control.default);
            }
        }
    }
    let points = Filter::response(rate, values);
    let color = ds.accent;
    let border = ds.frame_border();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .max_w(px(640.))
        .p_2()
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(border))
        .child(
            div()
                .text_xs()
                .text_color(rgb(ds.muted))
                .child("Respuesta del filtro · +24 a −60 dB"),
        )
        .child(
            canvas(
                move |_, _, _| (),
                move |bounds, (), window, _| {
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    let at =
                        |x: f32, y: f32| point(bounds.left() + px(x * w), bounds.top() + px(y * h));
                    let mut grid = PathBuilder::stroke(px(1.));
                    for hz in [
                        20_f32, 50., 100., 200., 500., 1000., 2000., 5000., 10000., 20000.,
                    ] {
                        let x = (hz / 20.).ln() / 1000_f32.ln();
                        grid.move_to(at(x, 0.));
                        grid.line_to(at(x, 1.));
                    }
                    for db in [24., 12., 0., -12., -24., -36., -48., -60.] {
                        let y = (24. - db) / 84.;
                        grid.move_to(at(0., y));
                        grid.line_to(at(1., y));
                    }
                    if let Ok(path) = grid.build() {
                        window.paint_path(path, rgb(border));
                    }
                    let mut curve = PathBuilder::stroke(px(2.));
                    for (i, &(x, db)) in points.iter().enumerate() {
                        let p = at(x, (24. - db) / 84.);
                        if i == 0 {
                            curve.move_to(p);
                        } else {
                            curve.line_to(p);
                        }
                    }
                    if let Ok(path) = curve.build() {
                        window.paint_path(path, rgb(color));
                    }
                },
            )
            .w_full()
            .h(px(130.)),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(rgb(ds.muted))
                .children(["20 Hz", "200", "2k", "20k"].map(|s| div().child(s))),
        )
}
