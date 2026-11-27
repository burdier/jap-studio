//! Controles táctiles estilo estudio mate (misma línea visual limpia del navegador de dispositivos):
//! superficies planas de perfil bajo, bordes capilares sutiles y lavado tintado suave + LED en estado activo.
use crate::ui::{DesignSystem, design::blend_color};
use gpui::{BoxShadow, Div, FontWeight, SharedString, Svg, div, prelude::*, px, rgb, rgba, svg};

pub(in crate::ui) const MAMEY_LOGO_COLOR: u32 = 0xff6b35;
const HAY_LOGO_SVG: &[u8] = include_bytes!("../../../assets/icons/hay-logo.svg");

/// Logo vectorial oficial de Hay en color Mamey (`#ff6b35`).
pub(in crate::ui) fn brand_logo(size_px: f32) -> Svg {
    svg()
        .data(HAY_LOGO_SVG)
        .size(px(size_px))
        .text_color(rgb(MAMEY_LOGO_COLOR))
}

/// Sombra discreta y limpia para cápsulas y paneles asentados sobre el chasis.
pub(in crate::ui) fn panel_shadow(ds: &DesignSystem) -> Vec<BoxShadow> {
    if ds.is_dark() {
        vec![BoxShadow::new(px(0.0), px(1.5), rgba(0x04030a88).into()).blur_radius(px(4.0))]
    } else {
        vec![BoxShadow::new(px(0.0), px(1.5), rgba(0x0f172a18).into()).blur_radius(px(4.0))]
    }
}

/// Micro-elevación muy contenida para el estado activo/seleccionado (estilo fila activa del navegador).
pub(in crate::ui) fn extruded_shadow(ds: &DesignSystem) -> Vec<BoxShadow> {
    if ds.is_dark() {
        vec![BoxShadow::new(px(0.0), px(1.0), rgba(0x04030a90).into()).blur_radius(px(3.0))]
    } else {
        vec![BoxShadow::new(px(0.0), px(1.0), rgba(0x0f172a1a).into()).blur_radius(px(3.0))]
    }
}

/// Punto LED quirúrgico (estilo hardware OP-1 Field / Braun) para estados activos.
pub(in crate::ui) fn led_dot(color: u32, size_px: f32, glow: bool) -> Div {
    div()
        .w(px(size_px))
        .h(px(size_px))
        .rounded_full()
        .flex_shrink_0()
        .bg(rgb(color))
        .when(glow, |dot| {
            dot.shadow(vec![
                BoxShadow::new(px(0.0), px(0.0), rgba((color << 8) | 0x88).into())
                    .blur_radius(px(4.0)),
            ])
        })
}

/// Superficie hundida suave (para barra de búsqueda, barra de estado y ranuras).
pub(in crate::ui) fn recessed_well(ds: &DesignSystem) -> Div {
    div()
        .bg(rgb(ds.panel_dark))
        .rounded(px(ds.radius_md))
        .border_1()
        .border_color(rgba(if ds.is_dark() { 0xffffff0c } else { 0x0f172a16 }))
}

/// Pantalla OLED/LCD oscura de cristal mate (estilo OP-1 Field) con contraste máximo en ambos temas.
pub(in crate::ui) fn hardware_lcd(ds: &DesignSystem) -> Div {
    let screen_bg = if ds.is_dark() { 0x0b0a12 } else { 0x151821 };
    div()
        .bg(rgb(screen_bg))
        .rounded(px(ds.radius_md))
        .border_1()
        .border_color(rgba(if ds.is_dark() { 0xffffff12 } else { 0x0f172a28 }))
}

/// Botón tipo píldora mate de perfil bajo (estilo navegador de dispositivos).
pub(in crate::ui) fn pill_button(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    active: bool,
    color: u32,
) -> Div {
    let dark = ds.is_dark();
    let (bg, border, text_col) = if active {
        (
            blend_color(ds.panel_hover, color, if dark { 24 } else { 16 }),
            rgba((color << 8) | if dark { 0x70 } else { 0x5c }),
            if dark { ds.text } else { color },
        )
    } else {
        (
            if dark {
                ds.panel_dark
            } else {
                blend_color(ds.panel_hover, 0xffffff, 35)
            },
            rgba(if dark { 0xffffff10 } else { 0x0f172a16 }),
            ds.text,
        )
    };
    let hover = blend_color(ds.panel_hover, color, if dark { 14 } else { 10 });
    div()
        .px(px(9.0))
        .py(px(3.0))
        .text_xs()
        .rounded(px(ds.radius_lg))
        .bg(rgb(bg))
        .border_1()
        .border_color(border)
        .text_color(rgb(text_col))
        .when(active, |el| el.shadow(extruded_shadow(ds)))
        .hover(move |style| style.bg(rgb(hover)))
        .cursor_pointer()
        .child(label.into())
}

/// Chip selector mate para paneles de configuración y categorías.
pub(in crate::ui) fn chip_option(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    selected: bool,
    accent_color: u32,
) -> Div {
    flat_control(ds, selected, accent_color)
        .px_3()
        .py(px(5.0))
        .gap(px(6.0))
        .rounded(px(ds.radius_md))
        .when(selected, |el| el.child(led_dot(accent_color, 5.5, true)))
        .text_xs()
        .text_color(rgb(if selected { ds.text } else { ds.muted }))
        .child(label.into())
}

/// Etiqueta compacta mate (misma línea que las etiquetas `VST3 FX` del navegador).
pub(in crate::ui) fn badge_tag(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    color: u32,
) -> Div {
    div()
        .px(px(5.0))
        .py(px(1.5))
        .rounded(px(ds.radius_sm))
        .bg(rgb(ds.panel_dark))
        .text_xs()
        .text_color(rgb(color))
        .child(label.into())
}

/// Control táctil de perfil bajo (mismo diseño mate que el navegador de dispositivos):
/// - En reposo: pastilla mate pegada a la base con borde capilar muy fino y hover suave.
/// - Activo: lavado tintado limpio del color de acento + borde capilar tintado + micro-elevación discreta.
pub(in crate::ui) fn flat_control(ds: &DesignSystem, active: bool, color: u32) -> Div {
    let dark = ds.is_dark();
    if active {
        let bg = blend_color(ds.panel_hover, color, if dark { 20 } else { 14 });
        let hover = blend_color(ds.panel_hover, color, if dark { 26 } else { 19 });
        let border = rgba((color << 8) | if dark { 0x66 } else { 0x55 });
        div()
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .rounded(px(ds.radius_sm))
            .bg(rgb(bg))
            .border_1()
            .border_color(border)
            .shadow(extruded_shadow(ds))
            .hover(move |style| style.bg(rgb(hover)))
            .cursor_pointer()
    } else {
        let bg = if dark {
            ds.panel_dark
        } else {
            blend_color(ds.panel_hover, 0xffffff, 35)
        };
        let hover = blend_color(ds.panel_hover, color, if dark { 12 } else { 8 });
        let border = rgba(if dark { 0xffffff10 } else { 0x0f172a16 });
        div()
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .rounded(px(ds.radius_sm))
            .bg(rgb(bg))
            .border_1()
            .border_color(border)
            .hover(move |style| style.bg(rgb(hover)))
            .cursor_pointer()
    }
}

pub(in crate::ui) fn toggle_badge(
    ds: &DesignSystem,
    label: impl Into<SharedString>,
    active: bool,
    color: u32,
) -> Div {
    flat_control(ds, active, color)
        .min_w(px(19.0))
        .h(px(17.0))
        .px(px(5.0))
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::MEDIUM
        })
        .text_color(rgb(if active { color } else { ds.muted }))
        .text_size(px(ds.font_size_xs - 1.0))
        .line_height(px(13.0))
        .child(label.into())
}
