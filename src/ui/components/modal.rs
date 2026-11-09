//! Overlay modal reutilizable con fondo translúcido y cierre al hacer clic fuera.
use gpui::{App, Div, IntoElement, MouseButton, Window, div, prelude::*, rgba};
use std::rc::Rc;

pub(in crate::ui) type ModalClose = Rc<dyn Fn(&gpui::MouseDownEvent, &mut Window, &mut App)>;

/// Envuelve un cuadro de diálogo centrado sobre un fondo translúcido que captura clics exteriores.
pub(in crate::ui) fn modal_backdrop(
    backdrop_rgba: u32,
    on_close: ModalClose,
    card: impl IntoElement,
) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .bg(rgba(backdrop_rgba))
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .on_pinch(|_, _, cx| cx.stop_propagation())
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            on_close(event, window, cx)
        })
        .child(
            div()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card),
        )
}
