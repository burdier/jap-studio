//! Controles de reproducción actuales; el transporte headless está en musical/time.rs.
use super::super::{
    CenterView, DawApp,
    components::button::{icon_button, pill_button},
    icons::ToolbarScope,
};
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
            .border_color(rgb(ds.frame_border()))
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
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    if let Some(entry) = elevate::catalog::find("action.tempo.set")
                                        && let Some(spec) = entry.action.parameters()
                                    {
                                        this.open_action_form(entry, spec, window, cx);
                                    }
                                    cx.stop_propagation();
                                }),
                            )
                            .child(format!("{:.2}", self.timeline.tempo.bpm())),
                    )
                    .child(div().text_color(rgb(ds.muted)).child("BPM"))
                    .child(
                        div()
                            .ml_2()
                            .text_color(rgb(ds.muted))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    if let Some(entry) = elevate::catalog::find("action.meter.set")
                                        && let Some(spec) = entry.action.parameters()
                                    {
                                        this.open_action_form(entry, spec, window, cx);
                                    }
                                    cx.stop_propagation();
                                }),
                            )
                            .child(self.timeline.meter.label()),
                    )
                    .child(
                        div()
                            .ml_4()
                            .text_xs()
                            .text_color(rgb(ds.accent))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.open_utility();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(format!("Utility {:.2}x", self.utility.state().gain)),
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
                            .text_color(rgb(if self.utility.state().bypassed {
                                ds.warning
                            } else {
                                ds.success
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.toggle_bypass();
                                    cx.notify();
                                }),
                            )
                            .child(if !self.utility.state().inserted {
                                "SIN FX"
                            } else if self.utility.state().bypassed {
                                "BYPASS"
                            } else {
                                "FX ON"
                            }),
                    ),
            )
            .child(self.render_toolbar_bound_buttons(ToolbarScope::Transport, false, cx))
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
