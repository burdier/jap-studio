//! Botones, píldoras (pills) y etiquetas (badges) reutilizables para toda la UI.
use crate::ui::DesignSystem;
use gpui::{Div, SharedString, Svg, div, prelude::*, px, rgb, svg};

pub(in crate::ui) const MAMEY_LOGO_COLOR: u32 = 0xff6b35;
const HAY_LOGO_SVG: &[u8] = include_bytes!("../../../assets/icons/hay-logo.svg");

/// Logo vectorial oficial de Hay en color Mamey (`#ff6b35`).
pub(in crate::ui) fn brand_logo(size_px: f32) -> Svg {
    svg()
        .data(HAY_LOGO_SVG)
        .size(px(size_px))
        .text_color(rgb(MAMEY_LOGO_COLOR))
}

/// Botón cuadrado de icono (usado en transporte y controles de incremento).
pub(in crate::ui) fn icon_button(
    ds: &DesignSystem,
    icon: impl Into<SharedString>,
    color: u32,
) -> Div {
    div()
        .w(px(ds.transport_btn_size))
        .h(px(ds.transport_btn_size))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(ds.border))
        .rounded(px(ds.radius_sm))
        .text_color(rgb(color))
        .cursor_pointer()
        .child(icon.into())
}

/// Botón tipo píldora con estado activo/inactivo (para toggles de barra, bancos, acciones).
pub(in crate::ui) fn pill_button(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    active: bool,
    color: u32,
) -> Div {
    div()
        .px_2()
        .py_1()
        .text_xs()
        .rounded(px(ds.radius_lg))
        .border_1()
        .border_color(rgb(if active { color } else { ds.border }))
        .bg(rgb(if active { color } else { ds.panel }))
        .text_color(rgb(if active { ds.bg } else { color }))
        .cursor_pointer()
        .child(label.into())
}

/// Chip selector para paneles de configuración, categorías del buscador y bancos.
pub(in crate::ui) fn chip_option(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    selected: bool,
    accent_color: u32,
) -> Div {
    div()
        .px_3()
        .py_1()
        .rounded(px(ds.radius_sm))
        .bg(rgb(if selected { ds.panel_hover } else { ds.panel }))
        .border_1()
        .border_color(rgb(if selected { accent_color } else { ds.border }))
        .text_xs()
        .text_color(rgb(if selected { ds.text } else { ds.muted }))
        .cursor_pointer()
        .child(label.into())
}

/// Etiqueta compacta (para indicadores `M`, `S`, `R`, `[+]` o categorías).
pub(in crate::ui) fn badge_tag(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    color: u32,
) -> Div {
    div()
        .px_1()
        .rounded(px(ds.radius_sm))
        .bg(rgb(ds.panel_dark))
        .text_xs()
        .text_color(rgb(color))
        .child(label.into())
}
