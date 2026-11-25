//! Marco de nota: trazo fino, sombra GPU discreta y tirador existente.
use super::grid_background::rectangle;
use crate::ui::DesignSystem;
use gpui::{Bounds, BoxShadow, Pixels, Window, fill, px, rgb, rgba};

#[cfg(feature = "p2p")]
pub(super) fn paint_peer(
    grid: &crate::ui::piano_roll::geometry::Grid,
    note: crate::collaboration::live::NotePresence,
    alias: &str,
    ds: &DesignSystem,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    use elevate::musical::time::Tick;
    use gpui::{ContentMask, TextAlign, TextRun, point, size};
    let bounds = grid.note_bounds(Tick(note.start), Tick(note.duration), note.pitch);
    if bounds.right() <= grid.notes.left()
        || bounds.left() >= grid.notes.right()
        || bounds.bottom() <= grid.notes.top()
        || bounds.top() >= grid.notes.bottom()
    {
        return;
    }
    window.with_content_mask(Some(ContentMask { bounds: grid.notes }), |window| {
        window.paint_quad(
            fill(bounds, rgba((ds.cyan << 8) | 0x68))
                .corner_radii(px(1.5))
                .border_widths(px(1.5))
                .border_color(rgb(ds.cyan)),
        );
        let run = TextRun {
            len: alias.len(),
            font: window.text_style().font(),
            color: rgb(ds.panel_dark).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window.text_system().shape_line(
            alias.to_owned().into(),
            px(ds.font_size_xs),
            &[run],
            None,
        );
        let height = px(ds.font_size_xs + 5.0);
        let width = (line.width + px(8.0)).min(grid.notes.size.width);
        let x = bounds
            .left()
            .clamp(grid.notes.left(), grid.notes.right() - width);
        let y = if bounds.top() - height >= grid.notes.top() {
            bounds.top() - height
        } else {
            bounds.bottom()
        };
        window.paint_quad(
            fill(Bounds::new(point(x, y), size(width, height)), rgb(ds.cyan)).corner_radii(px(2.0)),
        );
        let _ = line.paint(
            point(x + px(4.0), y),
            height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    });
}

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
