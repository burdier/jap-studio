//! Controles compactos de mezcla, conectados al fader y paneo de la pista.
use super::{DawApp, TRACK_UNITY_LEVEL, TcpControlStyle};
use crate::ui::{
    DesignSystem,
    components::knob::{KnobSpec, rotary_knob},
    design::blend_color,
};
use elevate::musical::midi_map::MidiBindTarget;
use gpui::{
    AnyElement, Context, CursorStyle, Div, IntoElement, MouseButton, MouseDownEvent, Pixels, Point,
    ScrollEvent, div, prelude::*, px, rgb,
};

#[derive(Clone, Copy)]
pub(in crate::ui) struct TrackControlDrag {
    pub(in crate::ui) track: usize,
    pan: bool,
    origin: Point<Pixels>,
    original: i32,
    span: Option<f32>,
    moved: bool,
}

impl DawApp {
    fn begin_track_control_drag(
        &mut self,
        track: usize,
        pan: bool,
        origin: Point<Pixels>,
        span: Option<f32>,
    ) {
        self.cursor_row = track;
        let original = if pan {
            self.track_pans[track]
        } else {
            self.track_levels[track]
        };
        self.track_control_drag = Some(TrackControlDrag {
            track,
            pan,
            origin,
            original,
            span,
            moved: false,
        });
    }

    pub(in crate::ui) fn update_track_control_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
    ) {
        let Some(drag) = &mut self.track_control_drag else {
            return;
        };
        if !dragging {
            self.end_track_control_drag();
            return;
        }
        let delta = if let Some(span) = drag.span {
            f32::from(position.x - drag.origin.x) * 100.0 / span
        } else {
            f32::from(drag.origin.y - position.y) * 0.8
        };
        let (track, pan, next) = (drag.track, drag.pan, drag.original + delta.round() as i32);
        drag.moved |= delta.abs() >= 0.5;
        self.set_track_mix_value(track, pan, next);
    }

    pub(in crate::ui) fn end_track_control_drag(&mut self) -> bool {
        let Some(drag) = self.track_control_drag.take() else {
            return false;
        };
        // El toque sin arrastre conserva el incremento de las perillas del perfil.
        if drag.span.is_none() && !drag.moved {
            let step = if drag.pan { 5 } else { 4 };
            self.set_track_mix_value(drag.track, drag.pan, drag.original + step);
        }
        true
    }

    fn set_track_mix_value(&mut self, track: usize, pan: bool, value: i32) {
        if pan {
            self.set_track_pan(track, value);
        } else {
            self.set_track_level(track, value);
        }
    }

    pub(super) fn render_track_mix_control(
        &self,
        i: usize,
        pan: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ds = &self.ds;
        let track = &self.tracks[i];
        let track_id = track.id;
        let value = if pan {
            self.track_pans[i]
        } else {
            self.track_levels[i]
        };
        let fraction = value as f32 / 100.0;
        let bar = ds.track_header.control_style() == TcpControlStyle::Bars;
        let color = if pan { ds.cyan } else { ds.warning };
        let width = if pan {
            ds.pan_bar_width.clamp(18., 36.)
        } else {
            ds.vol_bar_width.clamp(48., 70.)
        };
        let dragging = self
            .track_control_drag
            .is_some_and(|d| d.track == i && d.pan == pan);
        let hover_group = gpui::SharedString::from(format!("track-volume-{}", track.id.0));
        let control = if bar {
            if pan {
                pan_groove(ds, fraction, width, color)
            } else {
                volume_groove(ds, fraction, width, color)
            }
            .into_any_element()
        } else {
            rotary_knob(
                ds,
                KnobSpec {
                    fraction,
                    bipolar: pan,
                    color,
                    diameter: ds.knob_size.min(15.),
                },
            )
            .into_any_element()
        };
        let centered = if value == 50 {
            "C"
        } else if value < 50 {
            "L"
        } else {
            "R"
        };
        div()
            .group(hover_group.clone())
            .relative()
            .flex()
            .items_center()
            .gap(px(3.0))
            .h(px(16.0))
            .cursor(if bar {
                CursorStyle::ResizeLeftRight
            } else {
                CursorStyle::ResizeUpDown
            })
            .on_scroll(cx.listener(move |this, event: &ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                if delta != 0.0 {
                    let prior = if pan {
                        this.track_pans[i]
                    } else {
                        this.track_levels[i]
                    };
                    let step = if event.modifiers.shift {
                        1
                    } else if pan {
                        5
                    } else {
                        4
                    };
                    this.set_track_mix_value(i, pan, prior + step * delta.signum() as i32);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if this.midi_mapper.learn_mode() {
                        this.arm_midi_learn(if pan {
                            MidiBindTarget::TrackPan(track_id)
                        } else {
                            MidiBindTarget::TrackVolume(track_id)
                        });
                    } else {
                        window.focus(&this.focus_handle, cx);
                        this.begin_track_control_drag(i, pan, event.position, bar.then_some(width));
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    this.set_track_mix_value(i, pan, if pan { 50 } else { TRACK_UNITY_LEVEL });
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .when(!pan, |el| {
                el.child(
                    div()
                        .text_size(px(ds.font_size_xs - 1.0))
                        .text_color(rgb(ds.muted))
                        .child("V"),
                )
            })
            .child(control)
            .when(pan, |el| {
                el.child(
                    div()
                        .w(px(8.))
                        .text_size(px(ds.font_size_xs - 1.0))
                        .font_family("monospace")
                        .text_color(rgb(ds.muted))
                        .child(centered),
                )
            })
            .when(!pan, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(1.))
                        .left(px(10.))
                        .px(px(2.0))
                        .whitespace_nowrap()
                        .bg(rgb(ds.panel_dark))
                        .rounded(px(ds.radius_sm))
                        .text_color(rgb(ds.text))
                        .text_size(px(ds.font_size_xs - 1.0))
                        .font_family("monospace")
                        .opacity(if dragging { 1.0 } else { 0.0 })
                        .group_hover(hover_group, |style| style.opacity(1.0))
                        .child(level_db(value)),
                )
            })
            .into_any_element()
    }
}

fn level_db(level: i32) -> String {
    if level <= 0 {
        "−∞ dB".into()
    } else {
        format!(
            "{:+.1} dB",
            20.0 * (level as f32 / TRACK_UNITY_LEVEL as f32).log10()
        )
    }
}

fn volume_groove(ds: &DesignSystem, fraction: f32, width: f32, color: u32) -> Div {
    let height = ds.control_bar_height.clamp(4.0, 7.0);
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .rounded(px(2.))
        .bg(rgb(ds.panel_dark))
        .child(
            div()
                .h_full()
                .w(px(width * fraction))
                .rounded(px(2.))
                .bg(rgb(blend_color(ds.panel_dark, color, 52))),
        )
        .child(
            div()
                .absolute()
                .left(px(width * (TRACK_UNITY_LEVEL as f32 / 100.)))
                .top_0()
                .bottom_0()
                .w(px(1.))
                .bg(rgb(blend_color(ds.panel_dark, ds.text, 36))),
        )
        .child(
            div()
                .absolute()
                .left(px((width - 3.0) * fraction))
                .top(px(-1.0))
                .w(px(3.0))
                .h(px(height + 2.0))
                .rounded(px(1.0))
                .bg(rgb(color)),
        )
}

fn pan_groove(ds: &DesignSystem, fraction: f32, width: f32, color: u32) -> Div {
    let center = width * 0.5;
    let x = fraction * width;
    div()
        .relative()
        .w(px(width))
        .h(px(4.0))
        .rounded(px(2.0))
        .bg(rgb(ds.panel_dark))
        .child(
            div()
                .absolute()
                .left(px(x.min(center)))
                .top_0()
                .bottom_0()
                .w(px((x - center).abs()))
                .bg(rgb(blend_color(ds.panel_dark, color, 60))),
        )
        .child(
            div()
                .absolute()
                .left(px(center - 0.5))
                .top(px(-1.0))
                .h(px(6.0))
                .w(px(1.0))
                .bg(rgb(ds.muted)),
        )
        .child(
            div()
                .absolute()
                .left(px((width - 2.0) * fraction))
                .top(px(-1.0))
                .h(px(6.0))
                .w(px(2.0))
                .rounded(px(1.0))
                .bg(rgb(color)),
        )
}
