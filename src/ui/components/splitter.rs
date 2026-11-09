//! Separadores (splitters) reutilizables con indicador visual de redimensionado entre módulos flotantes.
use crate::ui::DesignSystem;
use gpui::{CursorStyle, Div, div, prelude::*, px, rgb};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::ui) enum SplitDrag {
    Fx { start_x: f32, start_width: f32 },
    Explorer { start_x: f32, start_width: f32 },
    TrackHeaders { start_x: f32, start_width: f32 },
    WorkspaceVertical { start_y: f32, start_ratio: f32 },
    TransportHeight { start_y: f32, start_height: f32 },
}

/// Divisor vertical (izquierda ↔ derecha) entre módulos flotantes con píldora indicadora.
pub(in crate::ui) fn vertical_splitter(ds: &DesignSystem, active: bool) -> Div {
    let pill_color = if active { ds.accent } else { ds.muted };
    div()
        .w(px(7.0))
        .h_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::ResizeLeftRight)
        .child(
            div()
                .w(px(2.5))
                .h(px(28.0))
                .rounded(px(2.0))
                .bg(rgb(pill_color)),
        )
}

/// Divisor horizontal compacto (arriba ↕ abajo, altura reducida) con píldora indicadora.
pub(in crate::ui) fn horizontal_splitter(ds: &DesignSystem, active: bool) -> Div {
    let pill_color = if active { ds.accent } else { ds.muted };
    div()
        .h(px(6.0))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::ResizeUpDown)
        .child(
            div()
                .w(px(36.0))
                .h(px(2.5))
                .rounded(px(2.0))
                .bg(rgb(pill_color)),
        )
}
