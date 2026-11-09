//! Mock visual explícito. Estos medidores no representan audio del dispositivo.
use super::super::{DawApp, components::meter::vertical_meter};
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
                            .text_color(rgb(colors[i % colors.len()]))
                            .child(track.name.to_string()),
                    )
                    .child(vertical_meter(
                        ds,
                        self.track_levels[i] / 10,
                        colors[i % colors.len()],
                        ds.border,
                    ))
                    .child(
                        div()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(format!("{}%", self.track_levels[i])),
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
                    .child(vertical_meter(ds, 9, ds.success, ds.success))
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
