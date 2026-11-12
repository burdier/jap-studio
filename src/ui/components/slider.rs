//! Slider genérico para parámetros. Trabaja con fracciones 0..1; quien lo usa
//! traduce a su rango. El slider son celdas: click o arrastre fijan el valor sin medir bounds.
use crate::ui::DesignSystem;
use gpui::{App, IntoElement, MouseButton, SharedString, Window, div, prelude::*, px, rgb};
use std::rc::Rc;

/// Recibe la fracción 0..1 pedida.
pub(in crate::ui) type FractionSet = Rc<dyn Fn(&f32, &mut Window, &mut App)>;
/// Se invoca al hacer click en la etiqueta (restaurar el valor por defecto).
pub(in crate::ui) type Reset = Rc<dyn Fn(&(), &mut Window, &mut App)>;

const CELLS: usize = 32;
const WIDTH: f32 = 220.0;

pub(in crate::ui) fn param_slider(
    ds: &DesignSystem,
    label: SharedString,
    value_text: String,
    fraction: f32,
    on_set: FractionSet,
    on_reset: Reset,
) -> impl IntoElement + use<> {
    let filled = (fraction * (CELLS - 1) as f32).round() as usize;
    let cells = (0..CELLS).map(|index| {
        let target = index as f32 / (CELLS - 1) as f32;
        let (press, drag) = (on_set.clone(), on_set.clone());
        div()
            .flex_1()
            .h_full()
            .bg(rgb(if index <= filled {
                ds.accent
            } else {
                ds.panel_dark
            }))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                press(&target, window, cx)
            })
            .on_mouse_move(move |event, window, cx| {
                if event.dragging() {
                    drag(&target, window, cx)
                }
            })
    });
    frame(ds, label, value_text, on_reset).child(
        div()
            .h(px(14.0))
            .flex()
            .gap(px(1.0))
            .border_1()
            .border_color(rgb(ds.border))
            .children(cells),
    )
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
