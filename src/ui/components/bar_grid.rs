//! Franjas, estilos y etiquetas compartidos por Arrangement y piano roll.
use crate::ui::{
    DesignSystem,
    time_grid::{
        TimeGrid,
        musical::{MarkKind, MusicalMark},
    },
};
use gpui::{Bounds, Pixels, Point, ShapedLine, TextRun, Window, fill, point, px, rgb, rgba, size};

pub(in crate::ui) fn mark_style(kind: MarkKind, design: &DesignSystem) -> (f32, f32, u32) {
    match kind {
        MarkKind::Bar => (1.5, 11.0, (design.muted << 8) | 0xa0),
        MarkKind::Beat => (1.0, 7.0, (design.text << 8) | 0x40),
        MarkKind::Subdivision => (1.0, 3.0, (design.text << 8) | 0x1c),
    }
}

pub(in crate::ui) fn ruler_labels(
    bounds: Bounds<Pixels>,
    axis: TimeGrid,
    marks: &[MusicalMark],
    design: &DesignSystem,
    window: &mut Window,
) -> Vec<(Point<Pixels>, ShapedLine)> {
    let mut labels = Vec::new();
    let mut previous_end = bounds.left();
    for mark in marks {
        let Some(text) = &mark.label else { continue };
        let x = bounds.left() + px(axis.x_at(mark.beat).round() as f32);
        let origin = point(x + px(design.padding_xs), bounds.top());
        if origin.x < previous_end {
            continue;
        }
        let run = TextRun {
            len: text.len(),
            font: window.text_style().font(),
            color: rgb(if mark.kind == MarkKind::Bar {
                design.text
            } else {
                design.muted
            })
            .into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window.text_system().shape_line(
            text.clone().into(),
            px(design.font_size_xs),
            &[run],
            None,
        );
        if origin.x + line.width > bounds.right() {
            continue;
        }
        previous_end = origin.x + line.width + px(design.padding_sm);
        labels.push((origin, line));
    }
    labels
}

/// Solo el fondo; cada vista dibuja sus marcas sin duplicar fronteras de compás.
pub(in crate::ui) fn paint_bands(
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
    }
}
