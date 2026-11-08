//! Mock visual explícito. Estos medidores no representan audio del dispositivo.
use super::super::{DawApp, DesignSystem};
use super::tracks::DEMO_LEVELS;
use gpui::{IntoElement, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_mixer(&self) -> impl IntoElement {
        let ds = &self.ds;
        let colors = [
            ds.accent, ds.danger, ds.cyan, ds.orange, ds.warning, ds.pink, ds.muted, ds.success,
        ];
        div()
            .flex()
            .flex_1()
            .p_4()
            .gap_3()
            .children(self.tracks.iter().enumerate().map(|(i, track)| {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(colors[i]))
                            .child(track.name),
                    )
                    .child(meter(DEMO_LEVELS[i] / 10, colors[i], ds.border, ds))
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(format!("{}%", DEMO_LEVELS[i])),
                    )
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("M S R"),
                    )
            }))
            .child(
                div()
                    .w(px(80.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(ds.success))
                            .child("MASTER"),
                    )
                    .child(meter(9, ds.success, ds.success, ds))
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("92%"),
                    ),
            )
    }
}

fn meter(filled: i32, color: u32, border: u32, ds: &DesignSystem) -> impl IntoElement {
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
        .children((0..10).rev().map(|bar| {
            div()
                .w_full()
                .h(px(12.0))
                .rounded(px(1.0))
                .bg(rgb(if bar < filled { color } else { ds.border }))
        }))
}
