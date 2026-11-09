//! Muestras de color reutilizables. El propietario aporta la paleta y el callback.
use crate::ui::DesignSystem;
use gpui::{Div, div, prelude::*, px, rgb};

pub(in crate::ui) fn color_swatch(
    ds: &DesignSystem,
    color: u32,
    selected: bool,
    focused: bool,
    automatic: bool,
) -> Div {
    let brightness =
        ((color >> 16) & 0xff) * 299 + ((color >> 8) & 0xff) * 587 + (color & 0xff) * 114;
    div()
        .size(px(24.0))
        .flex_shrink_0()
        .p(px(2.0))
        .border_1()
        .border_color(rgb(if focused || selected {
            ds.text
        } else {
            ds.panel_dark
        }))
        .cursor_pointer()
        .child(
            div()
                .size_full()
                .bg(rgb(color))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .text_color(rgb(if brightness > 128_000 {
                    0x151515
                } else {
                    0xffffff
                }))
                .child(if automatic {
                    "A"
                } else if selected {
                    "✓"
                } else {
                    ""
                }),
        )
}
