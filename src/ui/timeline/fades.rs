//! Fundidos de instancia. Dibujo y gestos comparten las coordenadas de la onda.
use super::{DesignSystem, Timeline, clip_frame, viewport::Viewport};
use crate::{
    audio::AudioClip,
    ui::{
        DawApp,
        components::curve_overlay::{CurvePath, paint::paint_curve},
        track_layout::TrackLayout,
    },
};
use elevate::musical::{clip::ClipId, sample_edit::ClipFade};
use gpui::{Bounds, Pixels, Point, Window, fill, point, px, rgb, size};

#[derive(Clone, Copy)]
pub(super) struct FadeDrag {
    pub(super) id: ClipId,
    side: usize,
    bend: bool,
}

struct Geometry {
    left: f64,
    width: f64,
    top: f32,
    height: f32,
    origin: f64,
    duration: f64,
}
impl Geometry {
    fn new(
        clip: &AudioClip,
        bpm: f64,
        vp: Viewport,
        bounds: Bounds<Pixels>,
        layout: &TrackLayout,
        ds: &DesignSystem,
    ) -> Option<Self> {
        let row = layout.clip_row(clip.track, clip.id)?;
        let left = vp.x_at(clip.start_beat);
        let width = vp.x_at(clip.end_beat(bpm)) - left;
        let inset = clip_frame::title_height(ds) + 5.;
        let height = row.height - inset - 7.;
        let duration = clip.length_seconds() * 1e6;
        (width >= 14. && height >= 5. && duration > 0.).then(|| Self {
            left: f64::from(bounds.left()) + left,
            width,
            top: f32::from(bounds.top()) + row.top + inset,
            height,
            origin: clip.trim_start * 1e6,
            duration,
        })
    }
    fn x(&self, us: f64) -> f64 {
        self.left + self.width * ((us - self.origin) / self.duration)
    }
    fn y(&self, gain: f32) -> f32 {
        self.top + self.height * (1. - gain)
    }
    fn handles(&self, fade: ClipFade, side: usize) -> [Option<(f32, f32)>; 2] {
        let first = self.x(fade.start_us as f64);
        let last = self.x(fade.end_us as f64);
        let right = self.left + self.width;
        let visible = fade.active() && first < right && last > self.left;
        let end = if visible {
            if side == 0 { last } else { first }
        } else if side == 0 {
            self.left
        } else {
            right
        };
        let mid = (first + last) * 0.5;
        [
            Some((end.clamp(self.left + 4., right - 4.) as f32, self.top)),
            (visible && last - first >= 12. && mid >= self.left && mid <= right)
                .then(|| (mid as f32, self.y(ClipFade::curve(0.5, fade.tension())))),
        ]
    }
}

pub(super) fn paint(
    clip: &AudioClip,
    bpm: f64,
    vp: Viewport,
    bounds: Bounds<Pixels>,
    layout: &TrackLayout,
    ds: &DesignSystem,
    window: &mut Window,
) {
    let Some(g) = Geometry::new(clip, bpm, vp, bounds, layout, ds) else {
        return;
    };
    for (side, fade) in clip.processing.edit.fades.into_iter().enumerate() {
        let first = g.x(fade.start_us as f64);
        let last = g.x(fade.end_us as f64);
        let left = first.max(g.left).max(f64::from(bounds.left()));
        let right = last.min(g.left + g.width).min(f64::from(bounds.right()));
        if fade.active() && right > left {
            let tension = fade.tension();
            let points = (0..=40)
                .map(|i| {
                    let nx = i as f32 / 40.;
                    let t =
                        ((left - first + (right - left) * f64::from(nx)) / (last - first)) as f32;
                    (
                        nx,
                        ClipFade::curve(if side == 1 { 1. - t } else { t }, tension),
                    )
                })
                .collect();
            let area = Bounds::new(
                point(px(left as f32), px(g.top)),
                size(px((right - left) as f32), px(g.height)),
            );
            paint_curve(
                area,
                &CurvePath {
                    points,
                    color: ds.accent,
                    stroke_px: 1.7,
                    fill_alpha: 0x1c,
                },
                0.,
                window,
            );
        }
        for (i, handle) in g.handles(fade, side).into_iter().enumerate() {
            if let Some((x, y)) = handle {
                for (r, color) in [
                    (4.5, ds.panel_dark),
                    (2.8, if i == 0 { ds.accent } else { ds.cyan }),
                ] {
                    window.paint_quad(
                        fill(
                            Bounds::new(point(px(x - r), px(y - r)), size(px(2. * r), px(2. * r))),
                            rgb(color),
                        )
                        .corner_radii(px(if i == 1 { r } else { 1. })),
                    );
                }
            }
        }
    }
}
impl Timeline {
    pub(super) fn fade_handle_at(&self, at: Point<Pixels>, ds: &DesignSystem) -> Option<FadeDrag> {
        let id = self.clip_hover_at(at, ds.track_header_height)?.id;
        let clip = self
            .clip
            .iter()
            .chain(&self.other_audio_clips)
            .find(|c| c.id == id && !c.props.locked)?;
        let g = Geometry::new(
            clip,
            self.tempo.bpm(),
            self.viewport,
            self.lanes_bounds(),
            self.layout.as_ref()?,
            ds,
        )?;
        let mut nearest = None;
        let mut distance = 36.;
        for (side, fade) in clip.processing.edit.fades.into_iter().enumerate() {
            for (i, handle) in g.handles(fade, side).into_iter().enumerate() {
                if let Some((x, y)) = handle {
                    let d = (x - f32::from(at.x)).powi(2) + (y - f32::from(at.y)).powi(2);
                    if d <= distance {
                        distance = d;
                        nearest = Some(FadeDrag {
                            id,
                            side,
                            bend: i == 1,
                        });
                    }
                }
            }
        }
        nearest
    }
}
impl DawApp {
    pub(in crate::ui) fn begin_clip_fade(&mut self, at: Point<Pixels>, reset: bool) -> bool {
        let Some(drag) = self.timeline.fade_handle_at(at, &self.ds) else {
            return false;
        };
        if reset {
            if let Some(c) = self
                .timeline
                .clip
                .iter_mut()
                .chain(&mut self.timeline.other_audio_clips)
                .find(|c| c.id == drag.id)
            {
                let f = &mut c.processing.edit.fades[drag.side];
                if drag.bend {
                    f.bend = 0;
                } else {
                    *f = ClipFade::default();
                }
                self.publish_clip_edit();
            }
        } else {
            self.timeline.fade_drag = Some(drag);
        }
        true
    }
    pub(in crate::ui) fn move_clip_fade(&mut self, at: Point<Pixels>) -> bool {
        let tl = &mut self.timeline;
        let Some(drag) = tl.fade_drag else {
            return false;
        };
        let bounds = tl.lanes_bounds();
        let Some(clip) = tl
            .clip
            .iter_mut()
            .chain(&mut tl.other_audio_clips)
            .find(|c| c.id == drag.id && !c.props.locked)
        else {
            return true;
        };
        let Some(layout) = tl.layout.as_ref() else {
            return true;
        };
        let Some(g) = Geometry::new(clip, tl.tempo.bpm(), tl.viewport, bounds, layout, &self.ds)
        else {
            return true;
        };
        let f = &mut clip.processing.edit.fades[drag.side];
        let previous = *f;
        if drag.bend {
            let gain = (1. - (f32::from(at.y) - g.top) / g.height).clamp(1. / 17., 16. / 17.);
            f.bend = (25. * (gain / (1. - gain)).log2())
                .round()
                .clamp(-100., 100.) as i8;
        } else {
            let fraction = ((f64::from(at.x) - g.left) / g.width).clamp(0., 1.);
            let length = if drag.side == 0 {
                fraction
            } else {
                1. - fraction
            };
            if length * g.width <= 4. {
                *f = ClipFade::default();
            } else {
                let pivot = (g.origin + fraction * g.duration).round() as i64;
                let (first, last) = if drag.side == 0 {
                    (g.origin.round() as i64, pivot)
                } else {
                    (pivot, (g.origin + g.duration).round() as i64)
                };
                f.start_us = first;
                f.end_us = last;
            }
        }
        if *f != previous {
            let side = if drag.side == 0 { "Entrada" } else { "Salida" };
            self.status_message = format!(
                "{side} · {:.1} ms · curva {:+}",
                (f.end_us - f.start_us) as f64 / 1000.,
                f.bend
            );
            self.publish_clip_edit();
        }
        true
    }
}
