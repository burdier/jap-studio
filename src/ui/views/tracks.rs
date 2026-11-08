//! Headers del Arrangement. Datos de demostración explícitos hasta introducir pistas reales.
use super::super::DawApp;
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

pub(super) const TRACK_NAMES: [&str; 8] = [
    "Kick", "Snare", "Hi-Hat", "Tom", "Bass", "Keys", "FX", "Vox",
];
pub(super) const DEMO_LEVELS: [i32; 8] = [78, 65, 52, 48, 70, 58, 42, 55];
const TRACK_TYPES: [&str; 8] = [
    "Drum", "Drum", "Drum", "Drum", "Synth", "Synth", "Effect", "Audio",
];
const PANS: [i32; 8] = [50, 50, 30, 50, 50, 70, 50, 50];

impl DawApp {
    pub(super) fn render_track_header(
        &self,
        i: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let color = [
            ds.accent, ds.success, ds.warning, ds.danger, ds.cyan, ds.orange, ds.pink, ds.accent,
        ][i];
        let has_audio = self
            .timeline
            .clip
            .as_ref()
            .is_some_and(|clip| clip.track == i);
        let volume_percent = if has_audio {
            (self.gain * 100.0).round() as i32
        } else {
            DEMO_LEVELS[i]
        };
        let volume_width = if has_audio {
            self.gain / 2.0 * ds.vol_bar_width
        } else {
            DEMO_LEVELS[i] as f32 / 100.0 * ds.vol_bar_width
        };
        div()
            .h(px(ds.track_header_height))
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .px_3()
            .py(px(ds.padding_xs))
            .gap_1()
            .bg(rgb(if i == self.cursor_row {
                ds.panel_hover
            } else {
                ds.panel
            }))
            .border_b_1()
            .border_color(rgb(ds.border))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.cursor_row = i;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(rgb(color)).child(TRACK_NAMES[i]))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(if has_audio { "Audio" } else { TRACK_TYPES[i] }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("P"))
                    .child(
                        div()
                            .w(px(ds.pan_bar_width))
                            .h(px(ds.control_bar_height))
                            .bg(rgb(ds.panel_dark))
                            .rounded(px(ds.radius_sm))
                            .child(
                                div()
                                    .w(px(PANS[i] as f32 / 100.0 * ds.pan_bar_width))
                                    .h_full()
                                    .bg(rgb(color))
                                    .rounded(px(ds.radius_sm)),
                            ),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("V"))
                    .child(
                        div()
                            .w(px(ds.vol_bar_width))
                            .h(px(ds.control_bar_height))
                            .on_scroll(cx.listener(
                                move |this, event: &gpui::ScrollEvent, _, cx| {
                                    if this
                                        .timeline
                                        .clip
                                        .as_ref()
                                        .is_some_and(|clip| clip.track == i)
                                    {
                                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                                        this.change_gain(delta.signum() * 0.05);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                },
                            ))
                            .bg(rgb(ds.panel_dark))
                            .rounded(px(ds.radius_sm))
                            .child(
                                div()
                                    .w(px(volume_width))
                                    .h_full()
                                    .bg(rgb(ds.success))
                                    .rounded(px(ds.radius_sm)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(format!("{volume_percent}%")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().text_color(rgb(ds.danger)).child("M"))
                    .child(div().text_xs().text_color(rgb(ds.warning)).child("S"))
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("R"))
                    .child(
                        div()
                            .ml_auto()
                            .text_xs()
                            .text_color(rgb(ds.accent))
                            .child("|+|"),
                    ),
            )
    }
}
