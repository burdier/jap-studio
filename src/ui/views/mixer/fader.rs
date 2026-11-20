//! Fader vertical de la tira. Render y picking comparten la geometría del riel.
use crate::ui::{
    DawApp,
    design::blend_color,
    mixer::{MixParam, MixerTarget},
};
use gpui::{
    Bounds, Context, CursorStyle, Div, MouseButton, MouseDownEvent, Pixels, Point, ScrollEvent,
    Stateful, canvas, component::tooltip::Tooltip, div, fill, point, prelude::*, px, rgb, size,
};
use std::{cell::Cell, rc::Rc};

const THUMB_HEIGHT: f32 = 10.0;
const THUMB_WIDTH: f32 = 21.0;
const INSET: f32 = 7.0;

impl DawApp {
    pub(super) fn render_mixer_fader(
        &self,
        target: MixerTarget,
        level: f32,
        unity: f32,
        color: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let rail_color = ds.panel_dark;
        let fill_color = blend_color(ds.panel_dark, color, 58);
        let tick_color = ds.border;
        let unity_color = ds.text;
        let thumb_color = blend_color(ds.panel_hover, ds.text, 28);
        let geometry = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let painted = geometry.clone();
        div()
            .id("channel-fader")
            .flex_1()
            .min_w_0()
            .h_full()
            .cursor(CursorStyle::ResizeUpDown)
            .tooltip(|window, cx| Tooltip::new("Fader · doble clic para 0 dB").build(window, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    let bounds = geometry.get();
                    if event.click_count == 2 {
                        this.set_mixer_fraction(target, MixParam::Level, unity);
                    } else {
                        if !event.modifiers.shift {
                            let fraction =
                                f32::from(bounds.bottom() - px(INSET) - event.position.y)
                                    / travel(bounds);
                            this.set_mixer_fraction(target, MixParam::Level, fraction);
                        }
                        this.begin_mixer_drag(
                            target,
                            MixParam::Level,
                            event.position.y,
                            travel(bounds),
                        );
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
                    let step = if event.modifiers.shift { 0.01 } else { 0.04 };
                    this.step_mixer_control(target, MixParam::Level, step * delta.signum());
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds, _, _| painted.set(bounds),
                    move |bounds, (), window, _| {
                        let x = bounds.left() + bounds.size.width * 0.5;
                        let top = bounds.top() + px(INSET);
                        let height = travel(bounds);
                        let at =
                            |fraction: f32| top + px(height * (1.0 - fraction.clamp(0.0, 1.0)));
                        let y = at(level);
                        let rail_bottom = top + px(height);
                        window.paint_quad(
                            fill(rect(point(x - px(2.5), top), 5.0, height), rgb(rail_color))
                                .corner_radii(px(2.0)),
                        );
                        window.paint_quad(fill(
                            rect(point(x - px(1.0), y), 2.0, f32::from(rail_bottom - y)),
                            rgb(fill_color),
                        ));
                        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                            let marker = fraction == unity;
                            window.paint_quad(fill(
                                rect(
                                    point(x - px(8.0), at(fraction)),
                                    if marker { 5.0 } else { 3.0 },
                                    1.0,
                                ),
                                rgb(if marker { unity_color } else { tick_color }),
                            ));
                        }
                        let thumb = rect(
                            point(x - px(THUMB_WIDTH * 0.5), y - px(THUMB_HEIGHT * 0.5)),
                            THUMB_WIDTH,
                            THUMB_HEIGHT,
                        );
                        window.paint_quad(fill(thumb, rgb(thumb_color)).corner_radii(px(2.0)));
                        window.paint_quad(fill(
                            rect(point(thumb.left() + px(3.0), y), THUMB_WIDTH - 6.0, 1.0),
                            rgb(unity_color),
                        ));
                    },
                )
                .size_full(),
            )
    }
}

fn travel(bounds: Bounds<Pixels>) -> f32 {
    (f32::from(bounds.size.height) - 2.0 * INSET).max(1.0)
}

fn rect(origin: Point<Pixels>, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(origin, size(px(width), px(height)))
}
