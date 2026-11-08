//! Barras de título y estado. No abrir dispositivos ni ejecutar DSP aquí.
use super::super::{
    CenterView, DawApp, VimMode,
    components::button::{MAMEY_LOGO_COLOR, brand_logo},
    cursor_fx::CursorFxMode,
};
use gpui::{Context, IntoElement, MouseButton, SharedString, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_title_bar(
        &self,
        mode_label: SharedString,
        mode_color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let view_label = match self.center_view {
            CenterView::Arrangement => "ARRANGEMENT",
            CenterView::Instrument => "INSTRUMENT",
            CenterView::Session => "PATTERN",
            CenterView::Mixer => "MIXER",
        };
        let audio_label = if let Some(player) = &self.pattern_player {
            format!("Patrón {} Hz", player.sample_rate)
        } else if let Some(player) = &self.player {
            format!("{} Hz", player.sample_rate)
        } else if let Some(rate) = self.instrument.sample_rate() {
            format!("{} · {rate} Hz", self.instrument.active_name())
        } else if let Some(clip) = &self.timeline.clip {
            format!("{} Hz", clip.file.sample_rate)
        } else {
            "Sin audio".into()
        };
        let engine_label = if self.pattern_player.is_some() {
            "● DRUMS"
        } else if self.player.is_some() {
            "● AUDIO"
        } else if self.instrument.is_running() {
            "● INSTRUMENT"
        } else {
            "○ STOP"
        };
        div()
            .h(px(ds.title_bar_height))
            .mb(px(5.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .bg(rgb(ds.panel_dark))
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .child(brand_logo(18.0))
                            .child(
                                div()
                                    .text_color(rgb(MAMEY_LOGO_COLOR))
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("Hay"),
                            ),
                    )
                    .child(div().text_color(rgb(ds.muted)).child("/"))
                    .child(div().text_color(rgb(ds.text)).child("Untitled Session"))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .py_1()
                            .bg(rgb(mode_color))
                            .rounded(px(ds.radius_sm))
                            .text_xs()
                            .text_color(rgb(ds.bg))
                            .child(mode_label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .px_2()
                            .py(px(2.0))
                            .rounded(px(ds.radius_sm))
                            .bg(rgb(ds.panel))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .text_xs()
                            .text_color(rgb(ds.pink))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.cycle_theme();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(format!("🎨 {}", ds.theme.label())),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(2.0))
                            .rounded(px(ds.radius_sm))
                            .bg(rgb(ds.panel))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .text_xs()
                            .text_color(rgb(if self.cursor_fx.mode() == CursorFxMode::Off {
                                ds.muted
                            } else {
                                ds.orange
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                                    let all = CursorFxMode::ALL;
                                    let idx = all
                                        .iter()
                                        .position(|m| *m == this.cursor_fx.mode())
                                        .unwrap_or(0);
                                    let next = all[(idx + 1) % all.len()];
                                    this.set_cursor_fx(next);
                                    this.cursor_fx.on_mouse_down(event.position);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(format!(
                                "{} {}",
                                self.cursor_fx.mode().badge(),
                                self.cursor_fx.mode().label()
                            )),
                    )
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(view_label))
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(audio_label))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.success))
                            .child(engine_label),
                    ),
            )
    }

    pub(in crate::ui) fn render_status_bar(
        &self,
        mode_label: SharedString,
        mode_color: u32,
    ) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .h(px(ds.status_bar_height))
            .mt(px(5.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(rgb(ds.panel_dark))
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(mode_color))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(mode_label),
                    )
                    .child(
                        div()
                            .text_color(rgb(if self.controller.enabled() {
                                ds.success
                            } else {
                                ds.muted
                            }))
                            .child(if self.controller.enabled() {
                                format!(
                                    "MIDI ON → {} · octava {} ({}) · vel {} · z/x:octava · m:off",
                                    self.midi_target_label(),
                                    self.controller.octave(),
                                    self.controller.base_label(),
                                    self.controller.velocity()
                                )
                            } else {
                                "MIDI off · m:on".to_string()
                            }),
                    )
                    .children(
                        [
                            "h j k l",
                            "i:insert",
                            "::cmd",
                            "a:buscar",
                            "Tab:view",
                            "+/-:zoom",
                            "R:reload",
                            "1/2/3/4:jump",
                        ]
                        .map(|label| div().text_color(rgb(ds.muted)).child(label)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(if self.mode == VimMode::Command {
                        div()
                            .text_color(rgb(ds.pink))
                            .child(format!(":{}", self.command_buffer))
                    } else {
                        div()
                            .text_color(rgb(ds.success))
                            .child(self.status_message.clone())
                    })
                    .child(div().text_color(rgb(ds.grid)).child(self.last_key.clone())),
            )
    }
}
