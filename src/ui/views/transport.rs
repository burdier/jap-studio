//! Controles de reproducción actuales; el transporte headless está en musical/time.rs.
use super::super::{CenterView, DawApp, DesignSystem};
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
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.stop_all();
                                    this.timeline.go_to_start();
                                    cx.notify();
                                }),
                            )
                            .child(transport_btn("⏮", ds.muted, ds)),
                    )
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.play_context();
                                    cx.notify();
                                }),
                            )
                            .child(transport_btn("▶", ds.success, ds)),
                    )
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.stop_all();
                                    this.status_message = "Detenido".into();
                                    cx.notify();
                                }),
                            )
                            .child(transport_btn("■", ds.text, ds)),
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
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.change_gain(-0.1);
                                    cx.notify();
                                }),
                            )
                            .child(transport_btn("−", ds.text, ds)),
                    )
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.change_gain(0.1);
                                    cx.notify();
                                }),
                            )
                            .child(transport_btn("+", ds.text, ds)),
                    )
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

fn transport_btn(icon: &'static str, color: u32, ds: &DesignSystem) -> impl IntoElement {
    div()
        .w(px(ds.transport_btn_size))
        .h(px(ds.transport_btn_size))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(ds.border))
        .rounded(px(ds.radius_sm))
        .text_color(rgb(color))
        .child(icon)
}
