//! Controles de reproducción actuales; el transporte headless está en musical/time.rs.
use super::super::{
    CenterView, DawApp,
    components::button::{icon_button, pill_button},
};
use elevate::musical::time::DEFAULT_BPM;
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .h(px(ds.transport_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .bg(rgb(ds.panel))
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon_button(ds, "⏮", ds.muted).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.stop_all();
                            this.timeline.go_to_start();
                            cx.notify();
                        }),
                    ))
                    .child(icon_button(ds, "▶", ds.success).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.play_context();
                            cx.notify();
                        }),
                    ))
                    .child(icon_button(ds, "■", ds.text).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.stop_all();
                            this.status_message = "Detenido".into();
                            cx.notify();
                        }),
                    ))
                    .child(
                        icon_button(
                            ds,
                            "●",
                            if self.midi_capture.is_recording() {
                                ds.danger
                            } else {
                                ds.muted
                            },
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.toggle_midi_record();
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        pill_button(
                            ds,
                            format!("↶ Recuperar MIDI ({})", self.midi_capture.note_count()),
                            false,
                            if self.midi_capture.has_notes() {
                                ds.cyan
                            } else {
                                ds.muted
                            },
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.capture_midi_to_track();
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .ml_4()
                            .text_color(rgb(ds.accent))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(format!("{DEFAULT_BPM:.2}")),
                    )
                    .child(div().text_color(rgb(ds.muted)).child("BPM"))
                    .child(div().ml_2().text_color(rgb(ds.muted)).child("4/4"))
                    .child(
                        div()
                            .ml_4()
                            .text_xs()
                            .text_color(rgb(ds.accent))
                            .child(format!("Volume {:.2}x", self.gain)),
                    )
                    .child(icon_button(ds, "−", ds.text).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.change_gain(-0.1);
                            cx.notify();
                        }),
                    ))
                    .child(icon_button(ds, "+", ds.text).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.change_gain(0.1);
                            cx.notify();
                        }),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if self.bypass { ds.warning } else { ds.success }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.toggle_bypass();
                                    cx.notify();
                                }),
                            )
                            .child(if self.bypass { "BYPASS" } else { "FX ON" }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.accent))
                            .child(format!("Edit {}", self.timeline.position_label())),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(
                        match self.center_view {
                            CenterView::Instrument => "Space: liberar notas",
                            CenterView::Session => "Space: play/stop · patrón en loop",
                            _ => "Space: play/stop",
                        },
                    ))
                    .child(
                        div()
                            .ml_2()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("[/]: gain · b: bypass"),
                    ),
            )
    }
}
