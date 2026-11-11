//! Franjas de compás compartidas por Arrangement y piano roll; independientes del snap.
use crate::ui::{DesignSystem, time_grid::TimeGrid};
use gpui::{Bounds, Pixels, Window, fill, point, px, rgba, size};

pub(in crate::ui) fn paint(
    bounds: Bounds<Pixels>,
    axis: TimeGrid,
    beats_per_bar: f64,
    design: &DesignSystem,
    window: &mut Window,
) {
    let width = f64::from(f32::from(bounds.size.width));
    for bar in axis.visible_bars(width, beats_per_bar) {
        let start = axis.x_at(bar as f64 * beats_per_bar);
        let end = axis.x_at((bar as f64 + 1.0) * beats_per_bar);
        let left = start.max(0.0);
        let right = end.min(width);
        // Texto del tema: aclara en temas oscuros y oscurece en temas claros.
        // Transparencia para mantener filas de notas, escala y selección visibles.
        if bar % 2 == 1 && right > left {
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.left() + px(left as f32), bounds.top()),
                    size(px((right - left) as f32), bounds.size.height),
                ),
                rgba((design.text << 8) | 0x18),
            ));
        }
        if start >= 0.0 && start < width {
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.left() + px(start as f32), bounds.top()),
                    size(px(1.5_f32.min((width - start) as f32)), bounds.size.height),
                ),
                rgba((design.muted << 8) | 0xa0),
            ));
        }
    }
}
