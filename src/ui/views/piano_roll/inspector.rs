//! Inspector visual del clip: usa las mismas etiquetas y barras que las cabeceras.
use crate::ui::{
    DawApp, DesignSystem,
    components::{button::badge_tag, meter::horizontal_bar},
    piano_roll::RollAction,
};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_roll_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let clip = self.timeline.midi_clip.as_ref().expect("clip visible");
        div()
            .w(px(176.0))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(ds.panel))
            .border_r_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .h(px(36.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .children(["Clip", "Notes", "Scale", "Map"].map(|label| {
                        self.roll_button(label, label == "Clip", RollAction::Preview(label), cx)
                            .px(px(5.0))
                            .h(px(21.0))
                    })),
            )
            .child(
                div()
                    .id("roll-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(5.0))
                    .child(
                        section(ds, "Clip Properties")
                            .child(property(ds, "Name", clip.name.clone()))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(16.0))
                                    .child(
                                        div()
                                            .w(px(64.0))
                                            .text_color(rgb(ds.muted))
                                            .text_size(px(10.5))
                                            .child("Color"),
                                    )
                                    .child(horizontal_bar(ds, 1.0, 90.0, ds.accent)),
                            )
                            .child(property(ds, "Channel", "1"))
                            .child(property(
                                ds,
                                "Length",
                                format!("{} bars", (clip.duration_beats() / 4.0).ceil() as u64),
                            ))
                            .child(property(ds, "Start", "1.1.1"))
                            .child(property(
                                ds,
                                "End",
                                format!("{}.1.1", (clip.duration_beats() / 4.0).ceil() as u64 + 1),
                            ))
                            .child(property(ds, "Loop", "ON"))
                            .child(property(ds, "Mute", "OFF"))
                            .child(property(ds, "Solo", "OFF")),
                    )
                    .child(
                        section(ds, "Quantize")
                            .child(property(ds, "Q", "1/16"))
                            .child(property(ds, "Strength", "100%"))
                            .child(property(ds, "Swing", "0%"))
                            .child(property(ds, "Random", "0%")),
                    )
                    .child(
                        section(ds, "Scale")
                            .child(property(ds, "Scale", "C minor (Aeolian) ▾"))
                            .child(property(ds, "Root", "C ▾"))
                            .child(self.roll_checkbox(
                                "Highlight scale notes",
                                self.piano_roll.highlight_scale,
                                RollAction::HighlightScale,
                                cx,
                            ))
                            .child(self.roll_checkbox(
                                "Clamp to scale",
                                false,
                                RollAction::Preview("ajustar a escala"),
                                cx,
                            ))
                            .child(self.roll_checkbox(
                                "Show scale in keyboard",
                                false,
                                RollAction::Preview("escala en teclado"),
                                cx,
                            )),
                    )
                    .child(
                        section(ds, "View")
                            .child(self.roll_checkbox(
                                "Ghost notes (other clips)",
                                self.piano_roll.ghost_notes,
                                RollAction::GhostNotes,
                                cx,
                            ))
                            .child(self.roll_checkbox(
                                "Show velocities",
                                self.piano_roll.velocities,
                                RollAction::Velocities,
                                cx,
                            ))
                            .child(self.roll_checkbox(
                                "Note names",
                                false,
                                RollAction::Preview("nombres de nota"),
                                cx,
                            ))
                            .child(property(ds, "Grid", "Bars + Beats ▾"))
                            .child(property(ds, "Zoom", "1 bar ▾")),
                    ),
            )
    }

    fn roll_checkbox(
        &self,
        label: &'static str,
        checked: bool,
        action: RollAction,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        div()
            .flex()
            .items_center()
            .gap_1()
            .h(px(17.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.apply_roll_action(action, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                badge_tag(ds, if checked { "✓" } else { " " }, ds.accent)
                    .w(px(10.0))
                    .h(px(10.0))
                    .px_0()
                    .text_size(px(9.0))
                    .line_height(px(10.0))
                    .border_1()
                    .border_color(rgb(if checked { ds.accent } else { ds.muted }))
                    .bg(rgb(if checked { ds.accent } else { ds.bg }))
                    .text_color(rgb(ds.bg)),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(ds.text))
                    .child(label),
            )
    }
}

fn section(ds: &DesignSystem, label: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .pb(px(5.0))
        .mb(px(3.0))
        .border_b_1()
        .border_color(rgb(ds.grid))
        .child(
            div()
                .h(px(23.0))
                .flex()
                .items_center()
                .text_size(px(12.0))
                .text_color(rgb(ds.accent))
                .child(format!("▾ {label}")),
        )
}

fn property(ds: &DesignSystem, label: &'static str, value: impl Into<gpui::SharedString>) -> Div {
    div()
        .h(px(16.0))
        .flex()
        .items_center()
        .text_size(px(10.5))
        .child(
            div()
                .w(px(64.0))
                .flex_shrink_0()
                .text_color(rgb(ds.muted))
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(rgb(ds.text))
                .child(value.into()),
        )
}
