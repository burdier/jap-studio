//! Slider continuo a 60 fps para parámetros. Trabaja con fracciones 0..1; quien lo usa
//! traduce a su rango. Click, arrastre continuo (Shift: precisión) o doble click para reset.
use crate::ui::DesignSystem;
use gpui::{
    App, Bounds, DispatchPhase, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Window, canvas, div,
    fill, point, prelude::*, px, rgb, size,
};
use std::{cell::Cell, rc::Rc};

/// Recibe la fracción 0..1 pedida.
pub(in crate::ui) type FractionSet = Rc<dyn Fn(&f32, &mut Window, &mut App)>;
/// Se invoca al hacer click en la etiqueta o doble click en la barra (restaurar valor por defecto).
pub(in crate::ui) type Reset = Rc<dyn Fn(&(), &mut Window, &mut App)>;

const WIDTH: f32 = 220.0;

#[derive(Clone, Copy)]
struct SliderDrag {
    last_x: Pixels,
    fraction: f32,
}

pub(in crate::ui) fn param_slider(
    ds: &DesignSystem,
    label: SharedString,
    value_text: String,
    fraction: f32,
    on_set: FractionSet,
    on_reset: Reset,
) -> impl IntoElement + use<> {
    let clamped = fraction.clamp(0.0, 1.0);
    let state_id = ElementId::from((ElementId::from("param-slider"), label.clone()));
    let ds_paint = ds.clone();
    let on_down = on_set.clone();
    let on_move = on_set;
    let reset_dbl = on_reset.clone();

    frame(ds, label, value_text, on_reset).child(
        div()
            .h(px(14.0))
            .w_full()
            .cursor_pointer()
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.panel_dark))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let state = window.use_keyed_state(state_id.clone(), cx, |_, _| {
                            Rc::new(Cell::new(None::<SliderDrag>))
                        });
                        (bounds, state.read(cx).clone())
                    },
                    move |bounds, (_, drag_cell), window, _| {
                        let w = f32::from(bounds.size.width).max(1.0);
                        let fill_w = (w * clamped).clamp(0.0, w);
                        if fill_w > 0.0 {
                            window.paint_quad(fill(
                                Bounds::new(bounds.origin, size(px(fill_w), bounds.size.height)),
                                rgb(ds_paint.accent),
                            ));
                        }
                        let thumb_x = (bounds.left() + px(fill_w - 1.5))
                            .clamp(bounds.left(), bounds.right() - px(3.0));
                        window.paint_quad(fill(
                            Bounds::new(
                                point(thumb_x, bounds.top()),
                                size(px(3.0), bounds.size.height),
                            ),
                            rgb(ds_paint.cyan),
                        ));

                        let down_cell = drag_cell.clone();
                        let on_down = on_down.clone();
                        let reset_dbl = reset_dbl.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || event.button != MouseButton::Left
                                || !bounds.contains(&event.position)
                            {
                                return;
                            }
                            if event.click_count >= 2 {
                                down_cell.set(None);
                                reset_dbl(&(), window, cx);
                                return;
                            }
                            let next = if event.modifiers.shift {
                                clamped
                            } else {
                                normalized_x(bounds, event.position)
                            };
                            down_cell.set(Some(SliderDrag {
                                last_x: event.position.x,
                                fraction: next,
                            }));
                            on_down(&next, window, cx);
                        });

                        let move_cell = drag_cell.clone();
                        let on_move = on_move.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let Some(mut session) = move_cell.get() else {
                                return;
                            };
                            if !event.dragging() {
                                move_cell.set(None);
                                return;
                            }
                            let w = f32::from(bounds.size.width).max(1.0);
                            if event.modifiers.shift {
                                let dx = f32::from(event.position.x - session.last_x) / w * 0.15;
                                session.fraction = (session.fraction + dx).clamp(0.0, 1.0);
                            } else {
                                session.fraction = normalized_x(bounds, event.position);
                            }
                            session.last_x = event.position.x;
                            move_cell.set(Some(session));
                            on_move(&session.fraction, window, cx);
                        });

                        let up_cell = drag_cell;
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, _| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                up_cell.set(None);
                            }
                        });
                    },
                )
                .size_full(),
            ),
    )
}

fn normalized_x(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let w = f32::from(bounds.size.width).max(1.0);
    (f32::from(position.x - bounds.left()) / w).clamp(0.0, 1.0)
}

fn frame(ds: &DesignSystem, label: SharedString, value_text: String, on_reset: Reset) -> gpui::Div {
    div().w(px(WIDTH)).flex().flex_col().gap(px(2.0)).child(
        div()
            .flex()
            .justify_between()
            .text_xs()
            .child(
                div()
                    .text_color(rgb(ds.muted))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        on_reset(&(), window, cx)
                    })
                    .child(label),
            )
            .child(div().text_color(rgb(ds.text)).child(value_text)),
    )
}
