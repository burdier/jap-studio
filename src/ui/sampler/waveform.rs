//! Waveform acotada a picos precalculados; selección y marcadores expresados en porcentaje.
use crate::ui::DawApp;
use elevate::devices::sampler::{END, LOOP_START, START};
use gpui::{
    Bounds, Context, IntoElement, MouseButton, PathBuilder, canvas, div, fill, point, prelude::*,
    px, rgb, rgba, size,
};
use std::{cell::Cell, rc::Rc};
impl DawApp {
    pub(super) fn sample_waveform(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let bounds_cell = Rc::new(Cell::new(None::<Bounds<gpui::Pixels>>));
        let paint_bounds = bounds_cell.clone();
        let asset = self.sampler.asset.clone();
        let params = self.sampler.params.clone();
        let ds = self.ds.clone();
        div()
            .size_full()
            .child(
                canvas(
                    move |bounds, _, _| {
                        paint_bounds.set(Some(bounds));
                    },
                    move |bounds, _, window, _| {
                        let Some(asset) = &asset else {
                            return;
                        };
                        let width = f32::from(bounds.size.width);
                        let height = f32::from(bounds.size.height);
                        let mid = height * 0.5;
                        let x = |f: f32| bounds.left() + px(width * f / 100.);
                        let y = |v: f32| bounds.top() + px(mid - v * (mid - 12.));
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left(), y(0.)),
                                size(bounds.size.width, px(1.)),
                            ),
                            rgb(ds.border),
                        ));
                        let peaks = &asset.display_peaks;
                        let scale = asset.file.max_peak.max(0.01);
                        let mut path = PathBuilder::fill();
                        for (i, peak) in peaks.iter().enumerate() {
                            let pos = point(
                                x(i as f32 * 100. / peaks.len().max(1) as f32),
                                y(peak[1] / scale),
                            );
                            if i == 0 {
                                path.move_to(pos);
                            } else {
                                path.line_to(pos);
                            }
                        }
                        for (i, peak) in peaks.iter().enumerate().rev() {
                            path.line_to(point(
                                x(i as f32 * 100. / peaks.len().max(1) as f32),
                                y(peak[0] / scale),
                            ));
                        }
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, rgb(ds.accent));
                        }
                        for (from, to) in [(0., params.get(START)), (params.get(END), 100.)] {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x(from), bounds.top()),
                                    size(px(width * (to - from) / 100.), bounds.size.height),
                                ),
                                rgba((ds.bg << 8) | 0xbb),
                            ));
                        }
                        for id in [START, END, LOOP_START] {
                            if id == LOOP_START && !params.looped {
                                continue;
                            }
                            let color = if id == LOOP_START { ds.cyan } else { ds.text };
                            let pos = x(params.get(id));
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(pos, bounds.top()),
                                    size(px(1.), bounds.size.height),
                                ),
                                rgb(color),
                            ));
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(pos - px(3.), bounds.top()),
                                    size(px(7.), px(7.)),
                                ),
                                rgb(color),
                            ));
                        }
                        for step in 0..=8 {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x(step as f32 * 12.5), bounds.bottom() - px(6.)),
                                    size(px(1.), px(6.)),
                                ),
                                rgb(ds.muted),
                            ));
                        }
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if let Some(bounds) = bounds_cell.get() {
                        let percent = (f32::from(event.position.x - bounds.left())
                            / f32::from(bounds.size.width).max(1.)
                            * 100.)
                            .clamp(0., 100.);
                        let id = if event.modifiers.alt && this.sampler.params.looped {
                            LOOP_START
                        } else if (percent - this.sampler.params.get(START)).abs()
                            < (percent - this.sampler.params.get(END)).abs()
                        {
                            START
                        } else {
                            END
                        };
                        this.sampler_ui.wave_drag = Some((id, bounds));
                        this.set_sample_param(id, percent, cx);
                    }
                    cx.stop_propagation();
                }),
            )
    }
}
