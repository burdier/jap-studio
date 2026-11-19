//! Botones, píldoras (pills) y etiquetas (badges) reutilizables para toda la UI.
use crate::ui::{DesignSystem, design::blend_color};
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
        .bg(rgb(if selected {
            ds.selection_background()
        } else {
            ds.panel
        }))
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

/// Superficie mate para controles densos, con indicación de estado por relleno.
pub(in crate::ui) fn flat_control(ds: &DesignSystem, active: bool, color: u32) -> Div {
    let rest = if active {
        blend_color(ds.panel_dark, color, 24)
    } else {
        ds.panel_dark
    };
    let hover = if active {
        blend_color(ds.panel_hover, color, 30)
    } else {
        ds.panel_hover
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .rounded(px(ds.radius_sm))
        .bg(rgb(rest))
        .hover(move |style| style.bg(rgb(hover)))
        .cursor_pointer()
}

pub(in crate::ui) fn toggle_badge(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    active: bool,
    color: u32,
) -> Div {
    flat_control(ds, active, color)
        .min_w(px(16.0))
        .h(px(14.0))
        .px(px(3.0))
        .text_color(rgb(if active { color } else { ds.muted }))
        .text_size(px(10.0))
        .child(label.into())
}
