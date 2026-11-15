//! Pintado 2D de curvas, relleno bajo la curva, stems y handles sobre un `Bounds<Pixels>`.
use super::{CurveAnchor, CurveModel, CurvePath};
use crate::ui::DesignSystem;
use gpui::{Bounds, PathBuilder, Pixels, Window, fill, point, px, rgb, rgba, size};

pub fn paint_model(
    bounds: Bounds<Pixels>,
    ds: &DesignSystem,
    model: &CurveModel,
    window: &mut Window,
) {
    let w = f32::from(bounds.size.width).max(1.0);
    let h = f32::from(bounds.size.height).max(1.0);
    let at = |nx: f32, ny: f32| {
        point(
            bounds.left() + px(nx.clamp(0.0, 1.0) * w),
            bounds.top() + px((1.0 - ny.clamp(0.0, 1.0)) * h),
        )
    };
    let border = ds.frame_border();

    if !model.grid_x.is_empty() || !model.grid_y.is_empty() {
        let mut grid = PathBuilder::stroke(px(1.0));
        for &gx in &model.grid_x {
            grid.move_to(at(gx, 0.0));
            grid.line_to(at(gx, 1.0));
        }
        for &gy in &model.grid_y {
            grid.move_to(at(0.0, gy));
            grid.line_to(at(1.0, gy));
        }
        if let Ok(path) = grid.build() {
            window.paint_path(path, rgba((border << 8) | 0x38));
        }
    }

    if let Some(base_y) = model.baseline {
        let py = bounds.top() + px((1.0 - base_y.clamp(0.0, 1.0)) * h);
        window.paint_quad(fill(
            Bounds::new(point(bounds.left(), py), size(bounds.size.width, px(1.0))),
            rgba((ds.muted << 8) | 0x4c),
        ));
    }

    for curve in &model.curves {
        paint_curve(bounds, curve, model.baseline.unwrap_or(0.0), window);
    }

    for anchor in &model.anchors {
        paint_anchor(bounds, ds, anchor, model.baseline.unwrap_or(0.0), window);
    }
}

pub fn paint_curve(bounds: Bounds<Pixels>, curve: &CurvePath, baseline: f32, window: &mut Window) {
    if curve.points.len() < 2 {
        return;
    }
    let w = f32::from(bounds.size.width).max(1.0);
    let h = f32::from(bounds.size.height).max(1.0);
    let at = |nx: f32, ny: f32| {
        point(
            bounds.left() + px(nx.clamp(0.0, 1.0) * w),
            bounds.top() + px((1.0 - ny.clamp(0.0, 1.0)) * h),
        )
    };

    if curve.fill_alpha > 0 {
        let mut area = PathBuilder::fill();
        let first = curve.points[0];
        let last = curve.points[curve.points.len() - 1];
        area.move_to(at(first.0, baseline));
        for &(nx, ny) in &curve.points {
            area.line_to(at(nx, ny));
        }
        area.line_to(at(last.0, baseline));
        area.close();
        if let Ok(path) = area.build() {
            window.paint_path(path, rgba((curve.color << 8) | u32::from(curve.fill_alpha)));
        }
    }

    let mut stroke = PathBuilder::stroke(px(curve.stroke_px.max(1.0)));
    for (i, &(nx, ny)) in curve.points.iter().enumerate() {
        let p = at(nx, ny);
        if i == 0 {
            stroke.move_to(p);
        } else {
            stroke.line_to(p);
        }
    }
    if let Ok(path) = stroke.build() {
        window.paint_path(path, rgb(curve.color));
    }
}

fn paint_anchor(
    bounds: Bounds<Pixels>,
    ds: &DesignSystem,
    anchor: &CurveAnchor,
    baseline: f32,
    window: &mut Window,
) {
    let w = f32::from(bounds.size.width).max(1.0);
    let h = f32::from(bounds.size.height).max(1.0);
    let px_x = bounds.left() + px(anchor.x.clamp(0.0, 1.0) * w);
    let px_y = bounds.top() + px((1.0 - anchor.y.clamp(0.0, 1.0)) * h);
    let base_y = bounds.top() + px((1.0 - baseline.clamp(0.0, 1.0)) * h);

    if anchor.crosshair {
        window.paint_quad(fill(
            Bounds::new(point(px_x, bounds.top()), size(px(1.0), bounds.size.height)),
            rgba((anchor.color << 8) | 0x55),
        ));
        window.paint_quad(fill(
            Bounds::new(point(bounds.left(), px_y), size(bounds.size.width, px(1.0))),
            rgba((anchor.color << 8) | 0x40),
        ));
    }

    if anchor.stem {
        let top = px_y.min(base_y);
        let stem_h = f32::from(px_y - base_y).abs().max(1.0);
        window.paint_quad(fill(
            Bounds::new(point(px_x - px(2.0), top), size(px(4.0), px(stem_h))),
            rgba((anchor.color << 8) | 0xd8),
        ));
    }

    window.paint_quad(fill(
        Bounds::new(
            point(px_x - px(5.0), px_y - px(5.0)),
            size(px(10.0), px(10.0)),
        ),
        rgb(ds.panel_dark),
    ));
    window.paint_quad(fill(
        Bounds::new(
            point(px_x - px(3.5), px_y - px(3.5)),
            size(px(7.0), px(7.0)),
        ),
        rgb(anchor.color),
    ));
}
