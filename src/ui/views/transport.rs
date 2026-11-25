//! Barra principal (transporte, display LCD de tiempo/tempo/compás, iconos y salida Utility).
use super::super::{
    DawApp, DesignSystem,
    components::button::{flat_control, hardware_lcd, led_dot, panel_shadow},
    icons::ToolbarScope,
};
use gpui::{
    Context, CursorStyle, Div, FontWeight, IntoElement, MouseButton, MouseDownEvent, ScrollEvent,
    SharedString, component::tooltip::Tooltip, div, prelude::*, px, rgb, rgba,
};

impl DawApp {
    pub(in crate::ui) fn render_transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let tap_active = self.tap_tempo_active();
        let tap_pad = flat_control(ds, tap_active, ds.accent)
            .id("transport-tap-tempo")
            .w(px(54.0))
            .h(px(ds.transport_btn_size + 6.0))
            .gap(px(5.0))
            .rounded(px(10.0))
            .text_size(px(ds.font_size_xs - 0.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(if tap_active { ds.text } else { ds.muted }))
            .tooltip(|window, cx| {
                Tooltip::new(
                    "Tap Tempo · haz clic al ritmo para calcular el BPM (clic derecho: reiniciar)",
                )
                .build(window, cx)
            })
            .when(tap_active, |el| el.child(led_dot(ds.accent, 5.0, true)))
            .child("TAP")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.tap_tempo();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(2250))
                            .await;
                        let _ = this.update(cx, |_, cx| cx.notify());
                    })
                    .detach();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| {
                    this.reset_tap_tempo();
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        // Sin caja alrededor: los controles emergen directamente del chasis base continuo.
        div()
            .id("transport-main-bar")
            .h(px(ds.transport_height))
            .mb(px(4.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(6.0))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_transport_cluster(cx))
                    .child(self.render_timing_lcd(cx))
                    .child(tap_pad),
            )
            .child(self.render_toolbar_bound_buttons(ToolbarScope::Transport, false, cx))
            .child(self.render_utility_capsule(cx))
    }

    fn render_transport_cluster(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let playing = self.player.is_some()
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.vst_any_clip_playing();
        let rec = self.midi_capture.is_recording();
        let audio_rec = self.audio_recorder.as_ref();
        let audio_rec_on = audio_rec.is_some();
        let audio_rec_peak = audio_rec.map_or(0.0, |r| r.recorder.peak());
        let audio_rec_label = if let Some(active) = audio_rec {
            if active.waiting_count_in {
                "PRE".to_string()
            } else {
                format!("{:.1}s", active.recorder.captured_seconds())
            }
        } else if self.audio_record_stereo {
            "Mic ST".to_string()
        } else {
            "Mic".to_string()
        };
        let audio_rec_color = if audio_rec_on {
            if audio_rec_peak > 0.92 {
                ds.warning
            } else {
                ds.danger
            }
        } else {
            ds.muted
        };
        let mon_mode = self.active_monitor_mode();
        let mon_on = mon_mode != crate::audio::input_monitor::MonitorMode::Off;
        let mon_peak = self.input_monitor_peak();
        let mon_label = match mon_mode {
            crate::audio::input_monitor::MonitorMode::Off => "Mon".to_string(),
            crate::audio::input_monitor::MonitorMode::In => "Mon IN".to_string(),
            crate::audio::input_monitor::MonitorMode::Auto => "Mon AUTO".to_string(),
        };
        let mon_color = match mon_mode {
            crate::audio::input_monitor::MonitorMode::In => ds.success,
            crate::audio::input_monitor::MonitorMode::Auto => ds.warning,
            crate::audio::input_monitor::MonitorMode::Off => ds.muted,
        };
        let btn_h = (ds.transport_btn_size - 2.0).max(22.0);
        let midi_color = if self.midi_capture.has_notes() {
            ds.cyan
        } else {
            ds.muted
        };
        let midi_label = format!("↶ MIDI ({})", self.midi_capture.note_count());
        let metro_on = self.metronome.is_enabled();
        let metro_vol = (self.metronome.gain() * 100.0).round() as u32;
        let count_in_bars = self.metronome.count_in_bars();
        let count_in_rem = self.metronome.count_in_remaining();
        let (pulse_seq, pulse_accent) = self.metronome.pulse_state();
        let metro_active_visual = metro_on || count_in_rem > 0;
        let metro_color = if count_in_rem > 0 {
            ds.danger
        } else if metro_on {
            if playing && pulse_seq > 0 && pulse_accent {
                ds.warning
            } else {
                ds.cyan
            }
        } else {
            ds.muted
        };
        let count_in_label = match count_in_bars {
            0 => "Pre: Off",
            1 => "Pre: 1c",
            _ => "Pre: 2c",
        };

        transport_group(ds)
            .child(
                transport_action_btn(ds, "|◀", false, ds.muted, btn_h).on_mouse_down(
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
                        this.stop_transport();
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
            .child(
                capsule_toggle(ds, audio_rec_label, audio_rec_on, audio_rec_color, btn_h)
                    .id("transport-audio-record")
                    .w(px(74.0))
                    .font_family("monospace")
                    .relative()
                    .overflow_hidden()
                    .when(audio_rec_on, |btn| {
                        let meter_w = (audio_rec_peak.clamp(0.0, 1.0) * 74.0).round();
                        btn.child(
                            div()
                                .absolute()
                                .left_0()
                                .bottom_0()
                                .h(px(2.5))
                                .w(px(meter_w))
                                .bg(rgb(if audio_rec_peak > 0.92 {
                                    ds.warning
                                } else {
                                    ds.danger
                                })),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_audio_record(None);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_audio_record_stereo();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                capsule_toggle(ds, mon_label, mon_on, mon_color, btn_h)
                    .id("transport-input-monitor")
                    .w(px(74.0))
                    .font_family("monospace")
                    .relative()
                    .overflow_hidden()
                    .tooltip(|window, cx| {
                        Tooltip::new(
                            "Monitoreo en vivo (sin grabar) con FX de la pista · Clic: In/Off · Clic derecho: Off/In/Auto",
                        )
                        .build(window, cx)
                    })
                    .when(mon_on, |btn| {
                        let meter_w = (mon_peak.clamp(0.0, 1.0) * 74.0).round();
                        btn.child(
                            div()
                                .absolute()
                                .left_0()
                                .bottom_0()
                                .h(px(2.5))
                                .w(px(meter_w))
                                .bg(rgb(if mon_peak > 0.92 {
                                    ds.danger
                                } else {
                                    mon_color
                                })),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_input_monitor();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.cycle_track_monitor(None);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
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
                capsule_toggle(
                    ds,
                    format!("♩ {metro_vol}%"),
                    metro_active_visual,
                    metro_color,
                    btn_h,
                )
                .id("transport-metronome")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.toggle_metronome();
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.set_metronome_gain(0.75);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                    let dy = f32::from(event.delta.pixel_delta(px(20.0)).y);
                    if dy != 0.0 {
                        let step = if event.modifiers.shift { 0.02 } else { 0.08 };
                        this.step_metronome_gain(dy.signum() * step);
                    }
                    cx.stop_propagation();
                    cx.notify();
                })),
            )
            .child(
                capsule_toggle(
                    ds,
                    count_in_label,
                    count_in_bars > 0,
                    if count_in_rem > 0 {
                        ds.danger
                    } else if count_in_bars > 0 {
                        ds.warning
                    } else {
                        ds.muted
                    },
                    btn_h,
                )
                .id("transport-count-in")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.cycle_count_in();
                        cx.stop_propagation();
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
        let bpm_text = format!("{:.2}", self.timeline.tempo.bpm());
        let meter_label = self.timeline.meter.label();
        let count_in_rem = self.metronome.count_in_remaining();
        let (pos_unit, pos_label) = if count_in_rem > 0 {
            ("PRE", format!("-{count_in_rem}"))
        } else {
            ("POS", self.timeline.position_label())
        };

        let lcd_pos_color = if count_in_rem > 0 { 0xff4769 } else { 0x38e8ff };
        let lcd_bpm_color = if ds.is_dark() { ds.accent } else { 0xff8a3d };
        let lcd_meter_color = 0xf4f6fb;
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
            .child(lcd_unit(pos_unit))
            .child(lcd_value(ds, lcd_pos_color, 50.0, pos_label));

        let bpm_readout = div()
            .id("transport-bpm-lcd")
            .px(px(5.0))
            .py(px(2.0))
            .rounded(px(ds.radius_sm))
            .when(self.timeline.tempo_drag.is_some(), |display| {
                display.bg(rgb(0x282d3d))
            })
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
            .child(lcd_value(ds, lcd_bpm_color, 48.0, bpm_text))
            .child(lcd_unit("BPM"));

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
            .child(lcd_value(ds, lcd_meter_color, 28.0, meter_label));

        hardware_lcd(ds)
            .h(px(ds.transport_btn_size + 6.0))
            .px(px(10.0))
            .rounded(px(10.0))
            .shadow(panel_shadow(ds))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(pos_readout)
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

        let master_fx_count = self.master_chain.nodes().len();
        let master_fx_open = self.fx.master_focus && self.fx.width > 0.0;
        let master_fx_label = if master_fx_count > 0 {
            format!("M-FX ({master_fx_count})")
        } else {
            "M-FX".into()
        };
        let [momentary, _] = self.master_lufs();
        transport_group(ds)
            .px(px(5.0))
            .gap(px(4.0))
            .child(
                capsule_toggle(
                    ds,
                    master_fx_label,
                    master_fx_open || master_fx_count > 0,
                    if master_fx_open || master_fx_count > 0 {
                        ds.cyan
                    } else {
                        ds.muted
                    },
                    20.0,
                )
                .id("transport-master-fx")
                .px(px(5.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.show_master_fx();
                        window.focus(&this.fx.view.read(cx).focus.clone(), cx);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .child(
                div()
                    .id("transport-utility-readout")
                    .px(px(3.0))
                    .text_size(px(ds.font_size_xs))
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
            .child(
                div()
                    .min_w(px(77.0))
                    .text_right()
                    .px(px(3.0))
                    .font_family("monospace")
                    .text_size(px(ds.font_size_xs))
                    .text_color(rgb(ds.cyan))
                    .child(format!("{momentary} LUFS M")),
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

fn transport_group(ds: &DesignSystem) -> Div {
    div()
        .h(px(ds.transport_btn_size + 6.0))
        .px(px(5.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(4.0))
        .rounded(px(10.0))
        .bg(rgb(ds.panel))
        .border_1()
        .border_color(rgba(if ds.is_dark() { 0xffffff0e } else { 0x0f172a14 }))
        .shadow(panel_shadow(ds))
}

fn transport_action_btn(ds: &DesignSystem, icon: &'static str, on: bool, c: u32, sz: f32) -> Div {
    flat_control(ds, on, c)
        .w(px(sz + 2.0))
        .h(px(sz))
        .rounded(px(ds.radius_md))
        .text_size(px(12.0))
        .text_color(rgb(if on { c } else { ds.text }))
        .child(icon)
}

fn capsule_toggle(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    active: bool,
    color: u32,
    height: f32,
) -> Div {
    flat_control(ds, active, color)
        .h(px(height))
        .py_0()
        .px(px(9.0))
        .gap(px(5.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .rounded(px(ds.radius_md))
        .text_size(px(ds.font_size_xs - 0.5))
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::MEDIUM
        })
        .text_color(rgb(if active { ds.text } else { ds.muted }))
        .when(active, |el| el.child(led_dot(color, 5.0, true)))
        .child(label.into())
}

fn lcd_step_button(ds: &DesignSystem, symbol: &'static str) -> Div {
    let hover = ds.panel_hover;
    div()
        .w(px(18.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(ds.radius_sm))
        .bg(rgb(0x1e2230))
        .border_1()
        .border_color(rgba(0xffffff12))
        .hover(move |el| el.bg(rgb(hover)))
        .cursor_pointer()
        .text_color(rgb(0xd8deeb))
        .text_size(px(ds.font_size_xs))
        .child(symbol.to_owned())
}

fn lcd_unit(text: &'static str) -> Div {
    div()
        .text_size(px(9.5))
        .text_color(rgb(0x7c859e))
        .child(text)
}

fn lcd_value(ds: &DesignSystem, color: u32, min_w: f32, text: impl Into<SharedString>) -> Div {
    div()
        .font_family("monospace")
        .min_w(px(min_w))
        .text_size(px(ds.font_size_xs + 1.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(color))
        .child(text.into())
}
