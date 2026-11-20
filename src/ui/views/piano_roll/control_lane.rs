//! Curva de pasos MIDI, contorno conectado de los palitos y puntos reales sobre `CurveOverlay`.
use super::grid_background::rectangle;
use crate::ui::{
    DesignSystem,
    components::curve_overlay::{
        CurvePath,
        paint::paint_curve,
        shapes::{ContourShape, connect_anchors, span_preview},
    },
    piano_roll::{LaneSpanDrag, RollLane, geometry::Grid},
};
use elevate::musical::{clip::MidiClip, model::Pattern, time::PPQ};
use gpui::{Bounds, ContentMask, Window, point, px, size};

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

/// Dibuja la curva/rampa/onda que conecta la parte superior de los palitos usando `CurveOverlay`.
pub(super) fn paint_note_contour(
    grid: &Grid,
    ds: &DesignSystem,
    clip: &MidiClip,
    lane: RollLane,
    shape: ContourShape,
    span: Option<LaneSpanDrag>,
    window: &mut Window,
) {
    if lane.controller().is_some() || shape == ContourShape::StemsOnly {
        return;
    }
    let usable_h = (f32::from(grid.velocity.size.height) - 16.0).max(1.0);
    let usable_w = f32::from(grid.notes.size.width).max(1.0);
    let draw_bounds = Bounds::new(
        point(grid.notes.left(), grid.velocity.bottom() - px(usable_h)),
        size(grid.notes.size.width, px(usable_h)),
    );
    let mut heads: Vec<(f32, f32)> = clip
        .notes()
        .iter()
        .filter(|note| !clip.muted_notes.contains(&note.id))
        .filter_map(|note| {
            let b = grid.note_bounds(note.start, note.duration, note.pitch);
            let stem_center = b.left() + px(7.5);
            let nx = f32::from(stem_center - grid.notes.left()) / usable_w;
            if (-0.05..=1.05).contains(&nx) {
                Some((nx.clamp(0.0, 1.0), lane.note_fraction(note).clamp(0.0, 1.0)))
            } else {
                None
            }
        })
        .collect();
    heads.sort_by(|a, b| a.0.total_cmp(&b.0));
    heads.dedup_by(|a, b| (a.0 - b.0).abs() < 0.002);

    let baseline = if lane.is_bipolar() { 0.5 } else { 0.0 };
    let color = ds.clip_color(clip.track, clip.props);

    window.with_content_mask(
        Some(ContentMask {
            bounds: Bounds::new(
                point(grid.notes.left(), grid.velocity.top()),
                size(grid.notes.size.width, grid.velocity.size.height),
            ),
        }),
        |window| {
            if heads.len() >= 2 {
                let points = connect_anchors(&heads, shape, 10);
                paint_curve(
                    draw_bounds,
                    &CurvePath {
                        points,
                        color,
                        stroke_px: 1.75,
                        fill_alpha: 0x22,
                    },
                    baseline,
                    window,
                );
            }
            for &(nx, ny) in &heads {
                let px_x = draw_bounds.left() + px(nx * usable_w);
                let px_y = draw_bounds.top() + px((1.0 - ny) * usable_h);
                rectangle(
                    window,
                    px_x - px(3.0),
                    px_y - px(3.0),
                    6.0,
                    6.0,
                    (ds.cyan << 8) | 0xff,
                );
            }
            if let Some(span) = span {
                let norm = |p: gpui::Point<gpui::Pixels>| {
                    let nx = (f32::from(p.x - draw_bounds.left()) / usable_w).clamp(0.0, 1.0);
                    let ny = ((f32::from(draw_bounds.bottom() - p.y)) / usable_h).clamp(0.0, 1.0);
                    (nx, ny)
                };
                let preview = span_preview(
                    norm(span.origin),
                    norm(span.current),
                    shape,
                    span.tension,
                    32,
                );
                paint_curve(
                    draw_bounds,
                    &CurvePath {
                        points: preview,
                        color: ds.cyan,
                        stroke_px: 2.0,
                        fill_alpha: 0x1a,
                    },
                    baseline,
                    window,
                );
            }
        },
    );
}

pub(in crate::ui::views) fn paint_velocity_stem(
    grid: &Grid,
    x: gpui::Pixels,
    fraction: f32,
    color: u32,
    window: &mut Window,
) {
    let height = fraction * (f32::from(grid.velocity.size.height) - 16.0).max(0.0);
    let top = grid.velocity.bottom() - px(height);
    rectangle(window, x + px(5.0), top, 5.0, height, (color << 8) | 0xff);
    rectangle(window, x + px(5.0), top, 5.0, 1.0, 0xffffff60);
}
