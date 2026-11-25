//! Tira común. El mismo gesto llega al controlador de pista, pieza o salida por ID.
use crate::{
    audio::ChannelMeter,
    ui::{
        DawApp,
        components::{
            button::{panel_shadow, toggle_badge},
            meter::{format_peak_db, stereo_mixer_meter},
            spectrum::{SpectrumMode, spectrum_graph_mode},
        },
        design::blend_color,
        mixer::{MixParam, MixSwitch, MixerTarget},
    },
};
use elevate::musical::{fx::FxId, track::TrackId};
use gpui::{
    Context, CursorStyle, Div, MouseButton, MouseDownEvent, ScrollEvent, Stateful,
    component::tooltip::Tooltip, div, prelude::*, px, rgb, rgba,
};

pub(super) enum ChannelSwitches {
    Mix { muted: bool, solo: bool },
    Utility { bypassed: bool },
}

pub(super) struct MasterLufs {
    pub momentary: Option<f64>,
    pub short_term: Option<f64>,
    pub integrated: Option<f64>,
}

pub(super) struct ChannelSend {
    pub target: TrackId,
    pub label: String,
    pub full_name: String,
    pub amount: u8,
}

pub(super) struct Channel {
    pub target: MixerTarget,
    pub name: String,
    pub color: u32,
    pub selected: bool,
    pub is_return: bool,
    pub sends: Vec<ChannelSend>,
    pub level: f32,
    pub unity: f32,
    pub pan: Option<i32>,
    pub switches: ChannelSwitches,
    pub meter: Option<ChannelMeter>,
    pub spectrum_fx: Option<FxId>,
    pub master_lufs: Option<MasterLufs>,
}

impl DawApp {
    pub(super) fn render_mixer_channel(
        &self,
        channel: Channel,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let target = channel.target;
        let color = channel.color;
        let key = match target {
            MixerTarget::Track(id) => format!("mixer-track-{}", id.0),
            MixerTarget::Piece(id) => format!("mixer-piece-{}", id.0),
            MixerTarget::Drums => "mixer-drums-bus".into(),
            MixerTarget::Master => "mixer-master".into(),
        };
        let state_buttons = div().flex().items_center().gap(px(2.0));
        let state_buttons = match channel.switches {
            ChannelSwitches::Mix { muted, solo } => state_buttons
                .child(self.mixer_button(target, MixSwitch::Mute, muted, cx))
                .child(self.mixer_button(target, MixSwitch::Solo, solo, cx)),
            ChannelSwitches::Utility { bypassed } => {
                state_buttons.child(self.mixer_button(target, MixSwitch::Bypass, bypassed, cx))
            }
        };
        let name = channel.name.clone();
        let unity = channel.unity;
        let level = channel.level;
        let meter = channel.meter;
        let is_master = matches!(target, MixerTarget::Master);
        let strip_width = if is_master { 136.0 } else { 108.0 };
        let section_border = rgba((ds.text << 8) | 0x1c);
        let section_bg = rgba((ds.panel_dark << 8) | 0x66);
        let clipped = meter.is_some_and(|m| m.hold >= 1.0);
        let peak_text = meter.map(|m| {
            let label = format_peak_db(m.hold);
            if is_master {
                format!("{label} TP")
            } else {
                format!("{label} dB")
            }
        });
        let head_spectrum = if let Some(fx_id) = channel.spectrum_fx {
            let tap_id = crate::audio::analyzer::tap::TapPoint::Output(fx_id);
            let view = self.fx.analyzers.views.get(&tap_id);
            let mode = view.map_or(SpectrumMode::Spectrum, |view| view.mode);
            let mut data = crate::ui::spectrum::data(view.map(|view| &view.analyzer), mode, ds);
            data.height = 38.0;
            let short_mode = match mode {
                SpectrumMode::Spectrum => "SPEC",
                SpectrumMode::Visualizer => "ONDA",
                SpectrumMode::SpectrogramLog => "LOG",
                SpectrumMode::SpectrogramLinear => "LIN",
                SpectrumMode::Level => "LVL",
            };
            Some(
                div()
                    .relative()
                    .w_full()
                    .h(px(38.0))
                    .flex_shrink_0()
                    .rounded(px(ds.radius_sm))
                    .overflow_hidden()
                    .border_1()
                    .border_color(section_border)
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if let Some(view) = this.fx.analyzers.views.get_mut(&tap_id) {
                                view.mode = view.mode.next();
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(spectrum_graph_mode(data, mode))
                    .child(
                        div()
                            .absolute()
                            .top(px(1.0))
                            .right(px(3.0))
                            .text_size(px(8.0))
                            .text_color(rgba((ds.text << 8) | 0x88))
                            .child(short_mode),
                    ),
            )
        } else if is_master {
            let mut data = crate::ui::spectrum::data(
                self.player.as_ref().map(|player| player.master_spectrum()),
                SpectrumMode::Spectrum,
                ds,
            );
            data.height = 38.0;
            Some(
                div()
                    .relative()
                    .w_full()
                    .h(px(38.0))
                    .flex_shrink_0()
                    .rounded(px(ds.radius_sm))
                    .overflow_hidden()
                    .border_1()
                    .border_color(section_border)
                    .child(spectrum_graph_mode(data, SpectrumMode::Spectrum))
                    .child(
                        div()
                            .absolute()
                            .top(px(1.0))
                            .right(px(3.0))
                            .text_size(px(8.0))
                            .text_color(rgba((ds.text << 8) | 0x88))
                            .child("MASTER"),
                    ),
            )
        } else {
            None
        };
        let controls_row = div()
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px(px(3.0))
            .py(px(2.0))
            .rounded(px(ds.radius_sm))
            .bg(section_bg)
            .border_1()
            .border_color(section_border)
            .child(state_buttons)
            .when_some(channel.pan, |row, pan| {
                row.child(self.render_mixer_pan(target, pan, cx))
            });
        let return_badge = channel.is_return.then(|| {
            div()
                .w_full()
                .flex_shrink_0()
                .py(px(1.5))
                .rounded(px(ds.radius_sm))
                .bg(rgba((ds.cyan << 8) | 0x24))
                .border_1()
                .border_color(rgba((ds.cyan << 8) | 0x66))
                .text_size(px(8.5))
                .text_center()
                .text_color(rgb(ds.cyan))
                .child("RETURN AUX")
        });
        let sends_block = (!channel.sends.is_empty()).then(|| {
            div()
                .w_full()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .px(px(3.0))
                .py(px(2.0))
                .rounded(px(ds.radius_sm))
                .bg(section_bg)
                .border_1()
                .border_color(section_border)
                .child(
                    div()
                        .text_size(px(8.0))
                        .text_color(rgb(ds.muted))
                        .child("ENVÍOS"),
                )
                .children(
                    channel
                        .sends
                        .into_iter()
                        .map(|send| self.render_mixer_send(target, send, cx)),
                )
        });
        let lufs_block = channel.master_lufs.map(|lufs| {
            let fmt = |v: Option<f64>| v.map_or_else(|| "—".to_string(), |db| format!("{db:.1}"));
            div()
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .px(px(3.0))
                .py(px(2.0))
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(section_border)
                .font_family("monospace")
                .text_size(px(8.5))
                .child(
                    div()
                        .text_color(rgb(ds.cyan))
                        .child(format!("M {}", fmt(lufs.momentary))),
                )
                .child(
                    div()
                        .text_color(rgb(ds.text))
                        .child(format!("S {}", fmt(lufs.short_term))),
                )
                .child(
                    div()
                        .text_color(rgb(ds.accent))
                        .child(format!("I {}", fmt(lufs.integrated))),
                )
        });
        let strip_bg =
            if channel.selected || matches!(target, MixerTarget::Drums | MixerTarget::Master) {
                blend_color(ds.panel, color, if ds.is_dark() { 10 } else { 7 })
            } else {
                ds.panel
            };
        div()
            .id(key)
            .w(px(strip_width))
            .h_full()
            .min_h(px(142.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(5.0))
            .p(px(6.0))
            .my(px(4.0))
            .ml(px(4.0))
            .bg(rgb(strip_bg))
            .rounded(px(ds.radius_lg))
            .when(channel.selected, |el| {
                el.border_1().border_color(rgba((color << 8) | 0x88))
            })
            .shadow(panel_shadow(ds))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if let MixerTarget::Track(id) = target
                        && let Some(index) = this.tracks.iter().position(|t| t.id == id)
                    {
                        this.select_track(index, event.modifiers.control, event.modifiers.shift);
                        window.focus(&this.focus_handle, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .w_full()
                    .h(px(3.0))
                    .flex_shrink_0()
                    .bg(rgb(color))
                    .rounded(px(1.5)),
            )
            .when_some(head_spectrum, |strip, spec| strip.child(spec))
            .when_some(return_badge, |strip, badge| strip.child(badge))
            .child(controls_row)
            .when_some(sends_block, |strip, block| strip.child(block))
            .when_some(lufs_block, |strip, block| strip.child(block))
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(3.0))
                    .py(px(2.0))
                    .rounded(px(ds.radius_sm))
                    .bg(section_bg)
                    .border_1()
                    .border_color(section_border)
                    .font_family("monospace")
                    .text_size(px(9.5))
                    .child(div().text_color(rgb(ds.text)).child(gain_db(level / unity)))
                    .when_some(peak_text, |row, text| {
                        row.child(
                            div()
                                .px(px(3.0))
                                .rounded(px(2.0))
                                .cursor_pointer()
                                .bg(if clipped {
                                    rgba((ds.danger << 8) | 0x44)
                                } else {
                                    rgba((ds.panel_dark << 8) | 0xc0)
                                })
                                .text_color(rgb(if clipped { ds.danger } else { ds.muted }))
                                .child(text)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        if let Some(player) = this.player.as_mut() {
                                            match target {
                                                MixerTarget::Master => player.reset_master_meter(),
                                                MixerTarget::Track(id) => {
                                                    player.reset_track_hold(id)
                                                }
                                                _ => {}
                                            }
                                        }
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h(px(36.0))
                    .w_full()
                    .gap(px(4.0))
                    .px(px(3.0))
                    .rounded(px(ds.radius_sm))
                    .bg(section_bg)
                    .border_1()
                    .border_color(section_border)
                    .child(self.render_mixer_fader(target, level, unity, color, cx))
                    .when_some(meter, |row, m| {
                        row.child(stereo_mixer_meter(ds, m.peak, m.rms, m.hold, true))
                    }),
            )
            .child(
                div()
                    .w_full()
                    .flex_shrink_0()
                    .px(px(3.0))
                    .py(px(2.0))
                    .rounded(px(ds.radius_sm))
                    .bg(section_bg)
                    .border_1()
                    .border_color(section_border)
                    .text_xs()
                    .text_color(rgb(ds.text))
                    .text_center()
                    .truncate()
                    .id("channel-name")
                    .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
                    .child(channel.name),
            )
    }

    fn mixer_button(
        &self,
        target: MixerTarget,
        switch: MixSwitch,
        active: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let (label, hint, color) = match switch {
            MixSwitch::Mute => ("M", "Silenciar", ds.danger),
            MixSwitch::Solo => ("S", "Solo", ds.cyan),
            MixSwitch::Bypass => ("BYP", "Bypass de Utility", ds.warning),
        };
        toggle_badge(ds, label, active, color)
            .id(label)
            .h(px(18.0))
            .px_1()
            .tooltip(move |window, cx| Tooltip::new(hint).build(window, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.toggle_mixer_control(target, switch);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }

    fn render_mixer_pan(
        &self,
        target: MixerTarget,
        pan: i32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let signed = (pan - 50) * 2;
        let label = match signed.cmp(&0) {
            std::cmp::Ordering::Less => format!("L{}", -signed),
            std::cmp::Ordering::Equal => "C".into(),
            std::cmp::Ordering::Greater => format!("R{signed}"),
        };
        div()
            .id("channel-pan")
            .h(px(18.0))
            .px(px(5.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(ds.radius_sm))
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgba((ds.cyan << 8) | if signed == 0 { 0x33 } else { 0x88 }))
            .cursor(CursorStyle::ResizeUpDown)
            .tooltip(|window, cx| {
                Tooltip::new("Paneo · arrastrar o rueda · doble clic para centrar")
                    .build(window, cx)
            })
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(if signed == 0 { ds.muted } else { ds.cyan }))
                    .child(label),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if event.click_count == 2 {
                        this.reset_mixer_fraction(target, MixParam::Pan, 0.5);
                    } else {
                        this.begin_mixer_drag(target, MixParam::Pan, event.position.y, 105.0);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &ScrollEvent, _, cx| {
                let scroll = event.delta.pixel_delta(px(24.0));
                let delta = f32::from(scroll.y);
                if scroll.x.abs() > scroll.y.abs() {
                    return;
                }
                if delta != 0.0 {
                    let step = if event.modifiers.shift { 0.01 } else { 0.05 };
                    this.step_mixer_control(target, MixParam::Pan, step * delta.signum());
                }
                cx.stop_propagation();
                cx.notify();
            }))
    }

    fn render_mixer_send(
        &self,
        target: MixerTarget,
        send: ChannelSend,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let dest = send.target;
        let active = send.amount > 0;
        let fill_pct = (f32::from(send.amount) / 100.0).clamp(0.0, 1.0);
        let tip = format!(
            "Envío → {} ({}%) · arrastrar o rueda · clic derecho/doble clic: 0%",
            send.full_name, send.amount
        );
        div()
            .id(("channel-send", dest.0))
            .relative()
            .w_full()
            .h(px(16.0))
            .rounded(px(2.5))
            .overflow_hidden()
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgba((ds.cyan << 8) | if active { 0x88 } else { 0x26 }))
            .cursor(CursorStyle::ResizeUpDown)
            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            .when(active, |row| {
                row.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(gpui::relative(fill_pct))
                        .bg(rgba((ds.cyan << 8) | 0x44)),
                )
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .px(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(9.0))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(if active { ds.text } else { ds.muted }))
                            .child(format!("↗ {}", send.label)),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(rgb(if active { ds.cyan } else { ds.muted }))
                            .child(format!("{}%", send.amount)),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if this.midi_mapper.learn_mode()
                        && let MixerTarget::Track(track_id) = target
                    {
                        this.arm_midi_learn(
                            elevate::musical::midi_map::MidiBindTarget::TrackSend {
                                track: track_id,
                                target: dest,
                            },
                        );
                    } else if event.click_count == 2 {
                        this.reset_mixer_fraction(target, MixParam::Send(dest), 0.0);
                    } else {
                        this.begin_mixer_drag(
                            target,
                            MixParam::Send(dest),
                            event.position.y,
                            110.0,
                        );
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    this.reset_mixer_fraction(target, MixParam::Send(dest), 0.0);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &ScrollEvent, _, cx| {
                let scroll = event.delta.pixel_delta(px(24.0));
                let delta = f32::from(scroll.y);
                if scroll.x.abs() > scroll.y.abs() {
                    return;
                }
                if delta != 0.0 {
                    let step = if event.modifiers.shift { 0.01 } else { 0.05 };
                    this.step_mixer_control(target, MixParam::Send(dest), step * delta.signum());
                }
                cx.stop_propagation();
                cx.notify();
            }))
    }
}

fn gain_db(gain: f32) -> String {
    if gain <= 0.0 {
        "−∞ dB".into()
    } else {
        format!("{:+.1} dB", 20.0 * gain.log10())
    }
}
