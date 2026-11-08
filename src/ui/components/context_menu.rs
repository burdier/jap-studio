//! Menú contextual anclado al puntero. El propietario decide sus opciones y acciones.
use super::modal::ModalClose;
use gpui::{Div, IntoElement, MouseButton, Pixels, Point, div, prelude::*};

pub(in crate::ui) fn context_menu(
    position: Point<Pixels>,
    on_close: ModalClose,
    content: impl IntoElement,
) -> Div {
    let right_close = on_close.clone();
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .occlude()
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            on_close(event, window, cx);
            cx.stop_propagation();
        })
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            right_close(event, window, cx);
            cx.stop_propagation();
        })
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .absolute()
                .left(position.x)
                .top(position.y)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(content),
        )
}
