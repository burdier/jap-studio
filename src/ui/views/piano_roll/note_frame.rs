//! Marco de nota: trazo fino, sombra GPU discreta y tirador existente.
use super::grid_background::rectangle;
use crate::ui::DesignSystem;
use gpui::{Bounds, BoxShadow, Pixels, Window, fill, px, rgb, rgba};

pub(in crate::ui::views) fn paint(
    bounds: Bounds<Pixels>,
    color: u32,
    hovered: bool,
    resize_edge: bool,
    ds: &DesignSystem,
    window: &mut Window,
) {
    let radius = px(1.7);
    window.paint_drop_shadows(
        bounds,
        radius.into(),
        &[BoxShadow::new(px(0.0), px(1.0), rgba(0x00000030).into()).blur_radius(px(2.0))],
    );
    window.paint_quad(
        fill(bounds, rgb(color))
            .corner_radii(radius)
            .border_widths(px(0.7))
            .border_color(rgba(if hovered {
                (ds.text << 8) | 0x90
            } else {
                (ds.panel_dark << 8) | 0xa8
            })),
    );
    if !hovered && !resize_edge {
        return;
    }
    let width = f32::from(bounds.size.width);
    let handle = width.min(6.0);
    rectangle(
        window,
        bounds.right() - px(handle),
        bounds.top() + px(0.7),
        (handle - 0.7).max(0.0),
        (f32::from(bounds.size.height) - 1.4).max(0.0),
        (ds.text << 8) | if resize_edge { 0x38 } else { 0x18 },
    );
    if resize_edge {
        rectangle(
            window,
            bounds.right() - px(width.min(2.0)),
            bounds.top() + px(1.0),
            width.min(1.0),
            (f32::from(bounds.size.height) - 2.0).max(0.0),
            (ds.text << 8) | 0xc0,
        );
    }
}
