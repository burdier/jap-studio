//! Barra principal (transporte, display LCD de tiempo/tempo/compás, iconos y salida Utility).
use super::super::{
    DawApp, DesignSystem,
    components::button::{icon_button, pill_button},
    icons::ToolbarScope,
};
use gpui::{
    Context, CursorStyle, Div, FontWeight, IntoElement, MouseButton, MouseDownEvent, ScrollEvent,
    SharedString, div, prelude::*, px, rgb,
};

impl DawApp {
    pub(in crate::ui) fn render_transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .id("transport-main-bar")
            .h(px(ds.transport_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .bg(rgb(ds.panel))
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.frame_border()))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.render_transport_cluster(cx))
                    .child(bar_separator(ds.border))
                    .child(self.render_timing_lcd(cx))
                    .child(bar_separator(ds.border)),
            )
            .child(self.render_toolbar_bound_buttons(ToolbarScope::Transport, false, cx))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(bar_separator(ds.border))
                    .child(self.render_utility_capsule(cx)),
            )
    }

    fn render_transport_cluster(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let playing = self.player.is_some()
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.vst_any_clip_playing();
        let rec = self.midi_capture.is_recording();
        let btn_h = (ds.transport_btn_size - 2.0).max(22.0);
        let midi_color = if self.midi_capture.has_notes() {
            ds.cyan
        } else {
            ds.muted
        };
        let midi_label = format!("↶ MIDI ({})", self.midi_capture.note_count());

        capsule_frame(ds)
            .child(
                transport_action_btn(ds, "⏮", false, ds.muted, btn_h).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Err(err) = this.seek_audio(0.) {
                            this.status_message = err;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                transport_action_btn(ds, "▶", playing, ds.success, btn_h).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.play_context();
                        cx.notify();
                    }),
                ),
            )
            .child(
                transport_action_btn(ds, "■", false, ds.text, btn_h).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.stop_all();
                        this.status_message = "Detenido".into();
                        cx.notify();
                    }),
                ),
            )
            .child(
                transport_action_btn(ds, "●", rec, if rec { ds.danger } else { ds.muted }, btn_h)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_midi_record();
                            cx.notify();
                        }),
                    ),
            )
            .child(inner_divider(ds.border))
            .child(
                capsule_toggle(ds, "↻ Loop", self.audio_loop.is_some(), ds.accent, btn_h)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_audio_loop();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                capsule_toggle(ds, midi_label, false, midi_color, btn_h).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.capture_midi_to_track();
                        cx.notify();
                    }),
                ),
            )
    }

    fn render_timing_lcd(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let bpm_bg = if self.timeline.tempo_drag.is_some() {
            ds.selection_background()
        } else {
            ds.panel_dark
        };
        let bpm_text = format!("{:.2}", self.timeline.tempo.bpm());
        let meter_label = self.timeline.meter.label();
        let pos_label = self.timeline.position_label();

        let pos_readout = div()
            .id("transport-pos-lcd")
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(3.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if let Err(err) = this.seek_audio(0.) {
                        this.status_message = err;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(lcd_unit(ds, "POS"))
            .child(lcd_value(ds.cyan, 44.0, pos_label));

        let bpm_readout = div()
            .id("transport-bpm-lcd")
            .px(px(5.0))
            .py(px(2.0))
            .rounded(px(ds.radius_sm))
            .bg(rgb(bpm_bg))
            .flex()
            .items_center()
            .gap(px(4.0))
            .cursor(CursorStyle::ResizeUpDown)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.click_count >= 2 {
                        this.timeline.tempo_drag = None;
                        this.open_tempo_form(window, cx);
                    } else {
                        this.begin_tempo_drag(event.position.y);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| {
                    this.reset_tempo();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                let dy = f32::from(event.delta.pixel_delta(px(20.0)).y);
                if dy != 0.0 {
                    let step = if event.modifiers.shift { 0.1 } else { 1.0 };
                    this.step_tempo(f64::from(dy.signum()) * step);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(lcd_value(ds.accent, 46.0, bpm_text))
            .child(lcd_unit(ds, "BPM"));

        let meter_readout = div()
            .id("transport-meter-lcd")
            .px(px(5.0))
            .py(px(2.0))
            .rounded(px(ds.radius_sm))
            .cursor_pointer()
            .flex()
            .items_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.open_meter_form(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| {
                    this.cycle_meter(true);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                let dy = f32::from(event.delta.pixel_delta(px(20.0)).y);
                if dy != 0.0 {
                    this.cycle_meter(dy > 0.0);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(lcd_value(ds.text, 24.0, meter_label));

        capsule_frame(ds)
            .px(px(6.0))
            .gap(px(5.0))
            .child(pos_readout)
            .child(inner_divider(ds.border))
            .child(lcd_step_button(ds, "−").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    this.step_tempo(if ev.modifiers.shift { -0.1 } else { -1.0 });
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(bpm_readout)
            .child(lcd_step_button(ds, "+").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| {
                    this.step_tempo(if ev.modifiers.shift { 0.1 } else { 1.0 });
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(inner_divider(ds.border))
            .child(meter_readout)
    }

    fn render_utility_capsule(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let state = self.utility.state();
        let (fx_label, fx_color) = if !state.inserted {
            ("SIN FX", ds.muted)
        } else if state.bypassed {
            ("BYPASS", ds.warning)
        } else {
            ("FX ON", ds.success)
        };
        let label_color = if self.utility_open {
            ds.cyan
        } else {
            ds.accent
        };

        capsule_frame(ds)
            .px(px(5.0))
            .gap(px(4.0))
            .child(
                div()
                    .id("transport-utility-readout")
                    .px(px(3.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(label_color))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.open_utility();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            let reset = elevate::devices::utility_slot::UtilityAction::Reset;
                            this.apply_utility_action(reset);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                        let dy = f32::from(event.delta.pixel_delta(px(20.0)).y);
                        if dy != 0.0 {
                            let step = if event.modifiers.shift { 0.01 } else { 0.05 };
                            this.change_gain(dy.signum() * step);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(format!("Utility {:.2}x", state.gain)),
            )
            .child(lcd_step_button(ds, "−").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.change_gain(-0.1);
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(lcd_step_button(ds, "+").on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.change_gain(0.1);
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(inner_divider(ds.border))
            .child(
                capsule_toggle(ds, fx_label, false, fx_color, 20.0)
                    .px(px(5.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_bypass();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }
}

fn capsule_frame(ds: &DesignSystem) -> Div {
    div()
        .h(px(ds.transport_btn_size + 4.0))
        .px(px(3.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(3.0))
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(ds.border))
        .rounded(px(ds.radius_md))
}

fn transport_action_btn(ds: &DesignSystem, icon: &'static str, on: bool, c: u32, sz: f32) -> Div {
    icon_button(ds, icon, if on { ds.bg } else { c })
        .w(px(sz))
        .h(px(sz))
        .flex_shrink_0()
        .bg(rgb(if on { c } else { ds.panel }))
        .border_color(rgb(if on { c } else { ds.border }))
        .text_size(px(12.0))
}

fn capsule_toggle(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    active: bool,
    color: u32,
    height: f32,
) -> Div {
    pill_button(ds, label, active, color)
        .h(px(height))
        .py_0()
        .px(px(7.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .rounded(px(ds.radius_sm))
        .text_size(px(10.5))
        .font_weight(FontWeight::BOLD)
}

fn lcd_step_button(ds: &DesignSystem, symbol: &'static str) -> Div {
    icon_button(ds, symbol, ds.text)
        .w(px(18.0))
        .h(px(20.0))
        .flex_shrink_0()
        .bg(rgb(ds.panel))
        .text_size(px(11.0))
}

fn lcd_unit(ds: &DesignSystem, text: &'static str) -> Div {
    div()
        .text_size(px(9.5))
        .text_color(rgb(ds.muted))
        .child(text)
}

fn lcd_value(color: u32, min_w: f32, text: impl Into<SharedString>) -> Div {
    div()
        .min_w(px(min_w))
        .text_size(px(11.5))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(color))
        .child(text.into())
}

fn inner_divider(color: u32) -> Div {
    div()
        .w(px(1.0))
        .h(px(16.0))
        .mx(px(2.0))
        .flex_shrink_0()
        .bg(rgb(color))
}

fn bar_separator(color: u32) -> Div {
    div().w(px(1.0)).h(px(22.0)).flex_shrink_0().bg(rgb(color))
}
