//! Curva de pasos MIDI y puntos reales; reutiliza geometría y primitivas del canvas.
use super::grid_background::rectangle;
use crate::ui::{
    DesignSystem,
    piano_roll::{RollLane, geometry::Grid},
};
use elevate::musical::{model::Pattern, time::PPQ};
use gpui::{ContentMask, Window, px};

pub(super) fn paint_controls(
    grid: &Grid,
    ds: &DesignSystem,
    pattern: &Pattern,
    lane: RollLane,
    selected: Option<u64>,
    window: &mut Window,
) {
    let Some(controller) = lane.controller() else {
        return;
    };
    let points: Vec<_> = pattern
        .controls()
        .points()
        .iter()
        .filter(|p| p.lane == controller)
        .collect();
    let height = (f32::from(grid.velocity.size.height) - 16.0).max(0.0);
    window.with_content_mask(
        Some(ContentMask {
            bounds: grid.velocity,
        }),
        |window| {
            for (index, point) in points.iter().enumerate() {
                let x = grid.x(point.at.0 as f64 / PPQ as f64);
                let end = points.get(index + 1).map_or(grid.notes.right(), |next| {
                    grid.x(next.at.0 as f64 / PPQ as f64)
                });
                if end < grid.notes.left() || x > grid.notes.right() {
                    continue;
                }
                let y = grid.velocity.bottom() - px(lane.control_fraction(point.value) * height);
                let color = if selected == Some(point.id) {
                    ds.text
                } else {
                    ds.accent
                };
                rectangle(
                    window,
                    x.max(grid.notes.left()),
                    y,
                    f32::from(end.min(grid.notes.right()) - x.max(grid.notes.left())),
                    1.0,
                    (color << 8) | 0xa0,
                );
                if let Some(next) = points.get(index + 1) {
                    let next_y =
                        grid.velocity.bottom() - px(lane.control_fraction(next.value) * height);
                    rectangle(
                        window,
                        end,
                        y.min(next_y),
                        1.0,
                        f32::from(y - next_y).abs(),
                        (color << 8) | 0x80,
                    );
                }
                rectangle(
                    window,
                    x - px(3.0),
                    y - px(3.0),
                    6.0,
                    6.0,
                    (color << 8) | 0xff,
                );
            }
        },
    );
}
