//! Tira común. El mismo gesto llega al controlador de pista, pieza o salida por ID.
use crate::ui::{
    DawApp,
    components::{
        button::toggle_badge,
        knob::{KnobSpec, rotary_knob},
        meter::track_peak_meter,
    },
    design::blend_color,
    mixer::{MixParam, MixSwitch, MixerTarget},
};
use gpui::{
    Context, CursorStyle, Div, IntoElement, MouseButton, MouseDownEvent, ScrollEvent, Stateful,
    component::tooltip::Tooltip, div, prelude::*, px, rgb,
};

pub(super) enum ChannelSwitches {
    Mix { muted: bool, solo: bool },
    Utility { bypassed: bool },
}

pub(super) struct Channel {
    pub target: MixerTarget,
    pub name: String,
    pub color: u32,
    pub selected: bool,
    pub level: f32,
    pub unity: f32,
    pub pan: Option<i32>,
    pub switches: ChannelSwitches,
    pub peak: Option<f32>,
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
        let bg = if channel.selected || matches!(target, MixerTarget::Drums | MixerTarget::Master) {
            blend_color(ds.panel, color, 12)
        } else {
            ds.panel
        };
        let state_buttons = div()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .h(px(22.0));
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
        let peak = channel.peak;
        div()
            .id(key)
            .w(px(73.0))
            .h_full()
            .min_h(px(142.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .p_1()
            .bg(rgb(bg))
            .rounded(px(ds.radius_sm))
            .font_family("monospace")
            .child(
                div()
                    .w_full()
                    .h(px(3.0))
                    .flex_shrink_0()
                    .bg(rgb(color))
                    .rounded(px(1.5)),
            )
            .child(state_buttons.flex_shrink_0())
            .child(if let Some(pan) = channel.pan {
                self.render_mixer_pan(target, pan, cx).into_any_element()
            } else {
                div()
                    .h(px(43.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .text_color(rgb(ds.muted))
                    .text_xs()
                    .child("Utility")
                    .into_any_element()
            })
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_center()
                    .text_color(rgb(ds.text))
                    .child(gain_db(level / unity)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_h(px(36.0))
                    .w_full()
                    .pr(px(if peak.is_some() { 13.0 } else { 0.0 }))
                    .child(self.render_mixer_fader(target, level, unity, color, cx))
                    .when_some(peak, |fader, peak| fader.child(track_peak_meter(ds, peak))),
            )
            .child(
                div()
                    .w_full()
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
            .w_full()
            .h(px(43.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .cursor(CursorStyle::ResizeUpDown)
            .tooltip(|window, cx| Tooltip::new("Paneo · doble clic para centrar").build(window, cx))
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction: pan as f32 / 100.0,
                    bipolar: true,
                    color: ds.cyan,
                    diameter: 23.0,
                },
            ))
            .child(div().text_xs().text_color(rgb(ds.muted)).child(label))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    if event.click_count == 2 {
                        this.set_mixer_fraction(target, MixParam::Pan, 0.5);
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
}

fn gain_db(gain: f32) -> String {
    if gain <= 0.0 {
        "−∞ dB".into()
    } else {
        format!("{:+.1} dB", 20.0 * gain.log10())
    }
}
