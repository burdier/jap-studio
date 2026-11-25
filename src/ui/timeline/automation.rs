//! Edición y pintado de curvas de automatización (Volumen / Pan) en el Arrangement.
use super::{DesignSystem, Timeline, viewport::Viewport};
use crate::ui::track_layout::{TrackLayout, TrackRow};
use elevate::musical::{
    track::TrackId,
    track_automation::{ArrangementAutomation, AutomationParam},
};
use gpui::{
    App, Bounds, PathBuilder, Pixels, Point, TextAlign, TextRun, Window, fill, point, px, rgb,
    rgba, size,
};

const PAD_Y: f32 = 6.0;
const POINT_HIT_RADIUS: f32 = 8.5;

#[derive(Clone, Copy, Debug)]
pub(super) struct AutomationDrag {
    pub track_index: usize,
    pub track_id: TrackId,
    pub param: AutomationParam,
    pub point_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AutomationHover {
    pub track_id: TrackId,
    pub point_index: usize,
}

fn value_y(row: &TrackRow, value: f32) -> f32 {
    let usable = (row.height - 2.0 * PAD_Y).max(4.0);
    row.top + PAD_Y + usable * (1.0 - value.clamp(0.0, 1.0))
}

fn y_value(row: &TrackRow, y_rel: f32) -> f32 {
    let usable = (row.height - 2.0 * PAD_Y).max(4.0);
    (1.0 - (y_rel - (row.top + PAD_Y)) / usable).clamp(0.0, 1.0)
}

fn param_color(param: AutomationParam, ds: &DesignSystem) -> u32 {
    match param {
        AutomationParam::Volume => ds.warning,
        AutomationParam::Pan => ds.cyan,
    }
}

impl Timeline {
    pub(in crate::ui) fn automation_dragging(&self) -> bool {
        self.automation_drag.is_some()
    }

    pub(in crate::ui) fn hovering_automation_point(&self) -> bool {
        self.automation_mode && self.automation_hover.is_some()
    }

    fn hit_automation_point(
        &self,
        position: Point<Pixels>,
        track_height: f32,
        tracks: &[elevate::musical::track::Track],
    ) -> Option<(usize, TrackId, usize)> {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return None;
        }
        let x_rel = f32::from(position.x - bounds.left());
        let y_rel = f32::from(position.y - bounds.top());
        let row = self.row_at_y(y_rel, track_height)?;
        let track = tracks.get(row.track)?;
        let lane = self.automation.lane(track.id, self.automation_param)?;
        let mut best: Option<(usize, f32)> = None;
        for (idx, pt) in lane.points().iter().enumerate() {
            let px_x = self.viewport.x_at(pt.beat) as f32;
            let px_y = value_y(&row, pt.value);
            let dist = ((px_x - x_rel).powi(2) + (px_y - y_rel).powi(2)).sqrt();
            if dist <= POINT_HIT_RADIUS && best.is_none_or(|(_, d)| dist < d) {
                best = Some((idx, dist));
            }
        }
        best.map(|(idx, _)| (row.track, track.id, idx))
    }

    pub(in crate::ui) fn update_automation_hover(
        &mut self,
        position: Point<Pixels>,
        track_height: f32,
        tracks: &[elevate::musical::track::Track],
    ) -> bool {
        let next = if self.automation_mode {
            self.hit_automation_point(position, track_height, tracks)
                .map(|(_, track_id, point_index)| AutomationHover {
                    track_id,
                    point_index,
                })
        } else {
            None
        };
        let changed = self.automation_hover != next;
        self.automation_hover = next;
        changed
    }

    /// Clic izquierdo en modo automatización: selecciona/arrastra un punto, doble clic lo borra,
    /// o clic en espacio vacío inserta un nuevo punto y comienza a arrastrarlo.
    pub(in crate::ui) fn begin_automation_edit(
        &mut self,
        position: Point<Pixels>,
        click_count: usize,
        shift: bool,
        track_height: f32,
        tracks: &[elevate::musical::track::Track],
    ) -> Option<(usize, TrackId, String)> {
        let bounds = self.lanes_bounds.get();
        if !bounds.contains(&position) {
            return None;
        }
        let param = self.automation_param;
        if let Some((track_index, track_id, point_index)) =
            self.hit_automation_point(position, track_height, tracks)
        {
            if click_count >= 2 {
                self.automation
                    .lane_mut(track_id, param)
                    .remove(point_index);
                self.automation.cleanup_empty();
                self.automation_drag = None;
                self.automation_hover = None;
                return Some((
                    track_index,
                    track_id,
                    format!("Automatización {} · punto eliminado", param.label()),
                ));
            }
            let pt = self.automation.lane(track_id, param)?.points()[point_index];
            self.automation_drag = Some(AutomationDrag {
                track_index,
                track_id,
                param,
                point_index,
            });
            return Some((
                track_index,
                track_id,
                format!(
                    "{} · {} @ {:.2} negras",
                    param.label(),
                    param.format_value(pt.value),
                    pt.beat
                ),
            ));
        }

        let x_rel = f64::from(f32::from(position.x - bounds.left()));
        let y_rel = f32::from(position.y - bounds.top());
        let row = self.row_at_y(y_rel, track_height)?;
        let track = tracks.get(row.track)?;
        let raw_beat = self.viewport.beat_at(x_rel).max(0.0);
        let beat = if shift {
            raw_beat
        } else {
            self.magnet_start(raw_beat)
        };
        let value = y_value(&row, y_rel);
        let point_index = self
            .automation
            .lane_mut(track.id, param)
            .upsert(beat, value);
        self.automation_drag = Some(AutomationDrag {
            track_index: row.track,
            track_id: track.id,
            param,
            point_index,
        });
        self.automation_hover = Some(AutomationHover {
            track_id: track.id,
            point_index,
        });
        Some((
            row.track,
            track.id,
            format!(
                "{} · {} @ {:.2} negras",
                param.label(),
                param.format_value(value),
                beat
            ),
        ))
    }

    pub(in crate::ui) fn move_automation_drag(
        &mut self,
        position: Point<Pixels>,
        shift: bool,
        track_height: f32,
    ) -> Option<(TrackId, String)> {
        let drag = self.automation_drag?;
        let bounds = self.lanes_bounds.get();
        let x_rel = f64::from(f32::from(position.x - bounds.left()));
        let y_rel = f32::from(position.y - bounds.top());
        let row = self
            .layout
            .as_ref()
            .and_then(|layout| layout.row(drag.track_index))
            .unwrap_or(TrackRow {
                track: drag.track_index,
                top: drag.track_index as f32 * track_height,
                height: track_height,
                lanes: 1,
                bands: 0,
                selected_band: None,
            });
        let raw_beat = self.viewport.beat_at(x_rel).max(0.0);
        let beat = if shift {
            raw_beat
        } else {
            self.magnet_start(raw_beat)
        };
        let value = y_value(&row, y_rel);
        let new_idx = self
            .automation
            .lane_mut(drag.track_id, drag.param)
            .move_point(drag.point_index, beat, value)?;
        self.automation_drag = Some(AutomationDrag {
            point_index: new_idx,
            ..drag
        });
        self.automation_hover = Some(AutomationHover {
            track_id: drag.track_id,
            point_index: new_idx,
        });
        Some((
            drag.track_id,
            format!(
                "{} · {} @ {:.2} negras",
                drag.param.label(),
                drag.param.format_value(value),
                beat
            ),
        ))
    }

    pub(in crate::ui) fn end_automation_drag(&mut self) -> Option<TrackId> {
        let drag = self.automation_drag.take()?;
        self.snap_guide.release();
        Some(drag.track_id)
    }

    /// Clic derecho en modo automatización: si hay un punto debajo, lo elimina.
    pub(in crate::ui) fn remove_automation_point_at(
        &mut self,
        position: Point<Pixels>,
        track_height: f32,
        tracks: &[elevate::musical::track::Track],
    ) -> Option<(TrackId, String)> {
        let (_, track_id, point_index) =
            self.hit_automation_point(position, track_height, tracks)?;
        let param = self.automation_param;
        self.automation
            .lane_mut(track_id, param)
            .remove(point_index);
        self.automation.cleanup_empty();
        self.automation_hover = None;
        Some((
            track_id,
            format!("Automatización {} · punto eliminado", param.label()),
        ))
    }
}

pub(super) struct PaintTrackDefaults {
    pub id: TrackId,
    pub vol_norm: f32,
    pub pan_norm: f32,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    automation: &ArrangementAutomation,
    active_mode: bool,
    active_param: AutomationParam,
    hover: Option<AutomationHover>,
    drag: Option<AutomationDrag>,
    track_defaults: &[PaintTrackDefaults],
    viewport: Viewport,
    bounds: Bounds<Pixels>,
    layout: &TrackLayout,
    ds: &DesignSystem,
    window: &mut Window,
    cx: &mut App,
) {
    let width = f32::from(bounds.size.width);
    if width <= 1.0 {
        return;
    }
    if active_mode {
        // Atenuar ligeramente los clips para que las curvas y nodos sean protagonistas.
        window.paint_quad(fill(bounds, rgba(0x00000050)));
    }
    for row in &layout.rows {
        let Some(defaults) = track_defaults.get(row.track) else {
            continue;
        };
        let track_id = defaults.id;
        let (param, lane_opt) = if active_mode {
            (active_param, automation.lane(track_id, active_param))
        } else if let Some(lane) = automation
            .lane(track_id, active_param)
            .filter(|l| !l.is_empty())
        {
            (active_param, Some(lane))
        } else if let Some(lane) = automation
            .lane(track_id, active_param.toggle())
            .filter(|l| !l.is_empty())
        {
            (active_param.toggle(), Some(lane))
        } else {
            continue;
        };

        let color = param_color(param, ds);
        let default_val = match param {
            AutomationParam::Volume => defaults.vol_norm,
            AutomationParam::Pan => defaults.pan_norm,
        };
        let points = lane_opt.map(|l| l.points()).unwrap_or(&[]);

        // Guía central sutil para Pan cuando el modo automatización está activo.
        if active_mode && param == AutomationParam::Pan {
            let cy = bounds.top() + px(value_y(row, 0.5));
            window.paint_quad(fill(
                Bounds::new(point(bounds.left(), cy), size(bounds.size.width, px(1.0))),
                rgba((ds.text << 8) | 0x20),
            ));
        }

        if points.is_empty() {
            if active_mode {
                let y = bounds.top() + px(value_y(row, default_val));
                window.paint_quad(fill(
                    Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.5))),
                    rgba((color << 8) | 0x55),
                ));
            }
            continue;
        }

        let left_x = bounds.left();
        let right_x = bounds.right();
        let first_y = bounds.top() + px(value_y(row, points[0].value));
        let last_y = bounds.top() + px(value_y(row, points[points.len() - 1].value));
        let base_y = bounds.top()
            + px(value_y(
                row,
                match param {
                    AutomationParam::Volume => 0.0,
                    AutomationParam::Pan => 0.5,
                },
            ));

        // Construimos la polilínea visible de izquierda a derecha de la ventana.
        let mut poly: Vec<Point<Pixels>> = Vec::with_capacity(points.len() + 2);
        let first_pt_x = bounds.left() + px(viewport.x_at(points[0].beat) as f32);
        if first_pt_x > left_x {
            poly.push(point(left_x, first_y));
        }
        for pt in points {
            let sx = bounds.left() + px(viewport.x_at(pt.beat) as f32);
            let sy = bounds.top() + px(value_y(row, pt.value));
            poly.push(point(sx, sy));
        }
        let last_pt_x = bounds.left() + px(viewport.x_at(points[points.len() - 1].beat) as f32);
        if last_pt_x < right_x {
            poly.push(point(right_x, last_y));
        }

        if active_mode && poly.len() >= 2 {
            let mut fill_path = PathBuilder::fill();
            fill_path.move_to(point(poly[0].x, base_y));
            for &p in &poly {
                fill_path.line_to(p);
            }
            fill_path.line_to(point(poly[poly.len() - 1].x, base_y));
            fill_path.close();
            if let Ok(path) = fill_path.build() {
                window.paint_path(path, rgba((color << 8) | 0x22));
            }
        }

        if poly.len() >= 2 {
            let mut stroke = PathBuilder::stroke(px(if active_mode { 2.0 } else { 1.3 }));
            stroke.move_to(poly[0]);
            for &p in &poly[1..] {
                stroke.line_to(p);
            }
            if let Ok(path) = stroke.build() {
                window.paint_path(
                    path,
                    rgba((color << 8) | if active_mode { 0xf0 } else { 0x85 }),
                );
            }
        }

        if !active_mode {
            continue;
        }

        for (idx, pt) in points.iter().enumerate() {
            let x_rel = viewport.x_at(pt.beat) as f32;
            if x_rel < -12.0 || x_rel > width + 12.0 {
                continue;
            }
            let cx_pt = bounds.left() + px(x_rel);
            let cy_pt = bounds.top() + px(value_y(row, pt.value));
            let is_drag = drag.is_some_and(|d| {
                d.track_id == track_id && d.param == param && d.point_index == idx
            });
            let is_hover = hover.is_some_and(|h| h.track_id == track_id && h.point_index == idx);
            let outer = if is_drag || is_hover { 6.0 } else { 4.5 };
            let inner = if is_drag || is_hover { 3.5 } else { 2.8 };
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(cx_pt - px(outer), cy_pt - px(outer)),
                        size(px(outer * 2.0), px(outer * 2.0)),
                    ),
                    rgb(ds.panel_dark),
                )
                .corner_radii(px(outer)),
            );
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(cx_pt - px(inner), cy_pt - px(inner)),
                        size(px(inner * 2.0), px(inner * 2.0)),
                    ),
                    rgb(if is_drag { ds.text } else { color }),
                )
                .corner_radii(px(inner)),
            );

            if is_drag || is_hover {
                let text = param.format_value(pt.value);
                let font_size = px(ds.font_size_xs.max(9.5));
                let run = TextRun {
                    len: text.len(),
                    font: window.text_style().font(),
                    color: rgb(ds.text).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let shaped = window
                    .text_system()
                    .shape_line(text.into(), font_size, &[run], None);
                let pad_x = px(4.0);
                let badge_w = shaped.width + pad_x * 2.0;
                let badge_h = px(15.0);
                let bx = (cx_pt + px(8.0)).min(bounds.right() - badge_w - px(4.0));
                let by = (cy_pt - badge_h - px(4.0)).max(bounds.top() + px(row.top) + px(2.0));
                window.paint_quad(
                    fill(
                        Bounds::new(point(bx, by), size(badge_w, badge_h)),
                        rgba((ds.panel_dark << 8) | 0xe8),
                    )
                    .corner_radii(px(3.0)),
                );
                let _ = shaped.paint(
                    point(bx + pad_x, by + px(1.5)),
                    badge_h,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
    }
}
