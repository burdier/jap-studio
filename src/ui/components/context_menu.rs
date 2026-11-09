//! Menú contextual anclado al puntero. El propietario decide sus opciones y acciones.
use super::modal::ModalClose;
use crate::ui::DesignSystem;
use gpui::{Div, IntoElement, MouseButton, Pixels, Point, SharedString, div, prelude::*, px, rgb};

/// Superficie compartida por menús de inserción, clips y futuros contextos.
pub(in crate::ui) fn menu_panel(ds: &DesignSystem, width: f32) -> Div {
    div()
        .w(px(width))
        .p_1()
        .flex()
        .flex_col()
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(ds.border))
        .rounded(px(2.0))
}

/// Fila plana: el fondo solo aparece al navegar o pasar el puntero.
pub(in crate::ui) fn menu_row(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    selected: bool,
    checked: bool,
) -> Div {
    div()
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(rgb(ds.text))
        .when(selected, |row| row.bg(rgb(ds.selection_background())))
        .cursor_default()
        .child(
            div()
                .w(px(16.0))
                .flex_shrink_0()
                .child(if checked { "✓" } else { "" }),
        )
        .child(label.into())
}

pub(in crate::ui) fn menu_separator(ds: &DesignSystem) -> Div {
    div().h(px(1.0)).my_1().bg(rgb(ds.border)).flex_shrink_0()
}

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
        .on_pinch(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .absolute()
                .left(position.x)
                .top(position.y)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(content),
        )
}
