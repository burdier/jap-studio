//! Waveform de alta resolución (Peak + RMS), zoom, regla de tiempo, envolvente y marcadores arrastrables.
use super::{WaveDrag, WaveHandle};
use crate::{audio::PEAK_FRAMES, ui::DawApp};
use elevate::devices::sampler::{
    ATTACK, DECAY, END, GAIN, LOOP_END, LOOP_START, PARAMS, ROOT, START, SUSTAIN,
};
use gpui::{
    Bounds, Context, IntoElement, MouseButton, Pixels, Point, ScrollDelta, canvas,
    component::tooltip::Tooltip, div, fill, point, prelude::*, px, rgb, rgba, size,
};
use std::{cell::Cell, rc::Rc};

impl DawApp {
    pub(in crate::ui) fn update_sampler_wave_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.sampler_ui.wave_drag else {
            return;
        };
        if !dragging {
            self.finish_sample_drag(cx);
            return;
        }
        let (v0, v1) = self.sampler_ui.visible_range();
        let frac = (f32::from(position.x - drag.bounds.left())
            / f32::from(drag.bounds.size.width).max(1.0))
        .clamp(0.0, 1.0);
        let pct = v0 + frac * (v1 - v0);
        match drag.handle {
            WaveHandle::Start => self.set_sample_param(START, pct, cx),
            WaveHandle::End => self.set_sample_param(END, pct, cx),
            WaveHandle::LoopStart => self.set_sample_param(LOOP_START, pct, cx),
            WaveHandle::LoopEnd => self.set_sample_param(LOOP_END, pct, cx),
            WaveHandle::SlideRange => {
                let span = (drag.initial_end - drag.initial_start).max(0.2);
                let delta = pct - drag.anchor_pct;
                let new_start = (drag.initial_start + delta).clamp(0.0, 100.0 - span);
                let new_end = (new_start + span).min(100.0);
                self.sampler.params.set(START, new_start);
                self.sampler.params.set(END, new_end);
                self.track_revision = self.track_revision.wrapping_add(1);
                self.status_message =
                    format!("Rango: {new_start:.1}% — {new_end:.1}% (suelta para aplicar)");
                cx.notify();
            }
        }
    }

    pub(super) fn zoom_sampler_wave(&mut self, factor: f32, anchor_pct: Option<f32>) {
        let (v0, v1) = self.sampler_ui.visible_range();
        let cur_span = (v1 - v0).max(0.5);
        let next_span = (cur_span * factor).clamp(1.0, 100.0);
        if next_span >= 99.5 {
            self.sampler_ui.wave_zoom = None;
            return;
        }
        let anchor = anchor_pct.unwrap_or((v0 + v1) * 0.5).clamp(v0, v1);
        let ratio = (anchor - v0) / cur_span;
        let new_v0 = (anchor - next_span * ratio).clamp(0.0, 100.0 - next_span);
        self.sampler_ui.wave_zoom = Some((new_v0, (new_v0 + next_span).min(100.0)));
    }

    fn wave_tool_btn(
        &self,
        id: &'static str,
        label: &'static str,
        tip: &'static str,
        accent_color: u32,
        cx: &mut Context<Self>,
        action: impl Fn(&mut DawApp, &mut Context<DawApp>) + 'static,
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .px(px(6.))
            .py(px(2.))
            .rounded(px(3.))
            .border_1()
            .border_color(rgba((self.ds.text << 8) | 0x20))
            .bg(rgba((self.ds.text << 8) | 0x0a))
            .hover(move |b| {
                b.bg(rgba((accent_color << 8) | 0x2a))
                    .border_color(rgb(accent_color))
            })
            .text_size(px(10.))
            .text_color(rgb(self.ds.text))
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    action(this, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    pub(super) fn sample_waveform(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let bounds_cell = Rc::new(Cell::new(None::<Bounds<Pixels>>));
        let paint_bounds = bounds_cell.clone();
        let hover_bounds = bounds_cell.clone();
        let scroll_bounds = bounds_cell.clone();
        let asset = self.sampler.asset.clone();
        let params = self.sampler.params.clone();
        let ds = self.ds.clone();
        let (v0, v1) = self.sampler_ui.visible_range();
        let hover_pct = self.sampler_ui.wave_hover;
        let zoomed = self.sampler_ui.wave_zoom.is_some();

        let duration_sec = asset.as_ref().map_or(0.0, |a| a.file.duration());
        let start_sec = duration_sec * f64::from(v0) / 100.0;
        let span_sec = duration_sec * f64::from(v1 - v0) / 100.0;

        let tick_labels: Vec<String> = (0..=6)
            .map(|i| {
                let t = start_sec + span_sec * (i as f64 / 6.0);
                if span_sec < 1.0 {
                    format!("{:.0} ms", t * 1000.0)
                } else {
                    format!("{t:.2} s")
                }
            })
            .collect();

        let hover_readout = hover_pct.map(|p| {
            let t = duration_sec * f64::from(p) / 100.0;
            if duration_sec < 2.0 {
                format!("{:.0} ms ({p:.1}%)", t * 1000.0)
            } else {
                format!("{t:.3} s ({p:.1}%)")
            }
        });

        let bar = div()
            .h(px(24.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .px_2()
            .bg(rgb(ds.arrangement_header()))
            .border_b_1()
            .border_color(rgb(ds.frame_border()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.wave_tool_btn(
                        "sampler-audition",
                        "▶ Probar",
                        "Disparar la nota raíz del sample",
                        ds.success,
                        cx,
                        |this, _| {
                            let note = this.sampler.params.get(ROOT) as u8;
                            this.sample_note_on(note, 110);
                        },
                    ))
                    .child(self.wave_tool_btn(
                        "sampler-zoom-range",
                        "✂ Ver rango",
                        "Acercar la vista al rango Inicio–Final",
                        ds.accent,
                        cx,
                        |this, _| {
                            let s = this.sampler.params.get(START);
                            let e = this.sampler.params.get(END);
                            let pad = ((e - s) * 0.05).max(0.5);
                            this.sampler_ui.wave_zoom =
                                Some(((s - pad).max(0.0), (e + pad).min(100.0)));
                        },
                    ))
                    .child(self.wave_tool_btn(
                        "sampler-loop-range",
                        "Loop = Rango",
                        "Igualar los límites del Loop al rango Inicio–Final",
                        ds.cyan,
                        cx,
                        |this, cx| {
                            let s = this.sampler.params.get(START);
                            let e = this.sampler.params.get(END);
                            this.sampler.params.mode = elevate::devices::sampler::PlayMode::Classic;
                            this.sampler.params.looped = true;
                            this.sampler.params.set(LOOP_START, s);
                            this.sampler.params.set(LOOP_END, e);
                            this.prepare_sampler(cx);
                            this.status_message =
                                format!("Loop ajustado al rango ({s:.1}%–{e:.1}%)");
                        },
                    ))
                    .child(self.wave_tool_btn(
                        "sampler-normalize",
                        "Normalizar",
                        "Ajustar ganancia del sampler al pico del sample (-0.5 dB)",
                        ds.warning,
                        cx,
                        |this, cx| {
                            if let Some(asset) = &this.sampler.asset {
                                let peak = asset.file.max_peak.max(1e-5);
                                let db = (-20.0 * peak.log10() - 0.5).clamp(-60.0, 6.0);
                                this.set_sample_param(GAIN, db, cx);
                                this.status_message = format!("Volumen normalizado: {db:+.1} dB");
                            }
                        },
                    ))
                    .child(self.wave_tool_btn(
                        "sampler-reset-range",
                        "⟲ Rango",
                        "Restablecer Inicio 0% y Final 100%",
                        ds.muted,
                        cx,
                        |this, cx| {
                            this.sampler.params.set(START, PARAMS[START].default);
                            this.sampler.params.set(END, PARAMS[END].default);
                            this.sampler
                                .params
                                .set(LOOP_START, PARAMS[LOOP_START].default);
                            this.sampler.params.set(LOOP_END, PARAMS[LOOP_END].default);
                            this.sampler_ui.wave_zoom = None;
                            this.prepare_sampler(cx);
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when_some(hover_readout, |row, text| {
                        row.child(
                            div()
                                .font_family("monospace")
                                .text_size(px(10.))
                                .text_color(rgb(ds.cyan))
                                .child(text),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.))
                            .child(self.wave_tool_btn(
                                "sampler-zoom-out",
                                "−",
                                "Alejar onda (o rueda del ratón)",
                                ds.muted,
                                cx,
                                |this, _| this.zoom_sampler_wave(1.6, None),
                            ))
                            .child(self.wave_tool_btn(
                                "sampler-zoom-in",
                                "+",
                                "Acercar onda (o rueda del ratón)",
                                ds.muted,
                                cx,
                                |this, _| this.zoom_sampler_wave(0.6, None),
                            ))
                            .when(zoomed, |r| {
                                r.child(self.wave_tool_btn(
                                    "sampler-zoom-full",
                                    "100%",
                                    "Ver toda la onda",
                                    ds.accent,
                                    cx,
                                    |this, _| this.sampler_ui.wave_zoom = None,
                                ))
                            }),
                    ),
            );

        let ruler = div()
            .h(px(16.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_2()
            .bg(rgb(ds.panel_dark))
            .border_b_1()
            .border_color(rgb(ds.frame_border()))
            .font_family("monospace")
            .text_size(px(9.))
            .text_color(rgb(ds.muted))
            .children(tick_labels.into_iter().map(|lbl| div().child(lbl)));

        let surface_bg = ds.arrangement_surface();
        let canvas_el = div()
            .id("sampler-waveform-canvas")
            .flex_1()
            .min_h_0()
            .bg(rgb(surface_bg))
            .cursor(gpui::CursorStyle::Crosshair)
            .child(
                canvas(
                    move |bounds, _, _| {
                        paint_bounds.set(Some(bounds));
                    },
                    move |bounds, _, window, _| {
                        let Some(asset) = &asset else {
                            return;
                        };
                        let width = f32::from(bounds.size.width).max(1.0);
                        let height = f32::from(bounds.size.height).max(1.0);
                        let mid = height * 0.5;
                        let span_pct = (v1 - v0).max(0.1);
                        let x_px = |pct: f32| ((pct - v0) / span_pct) * width;
                        let paint =
                            |win: &mut gpui::Window, x: f32, y: f32, w: f32, h: f32, col: u32| {
                                if w > 0.0 && h > 0.0 {
                                    win.paint_quad(fill(
                                        Bounds::new(
                                            point(bounds.left() + px(x), bounds.top() + px(y)),
                                            size(px(w), px(h)),
                                        ),
                                        rgba(col),
                                    ));
                                }
                            };

                        // Rejilla vertical (6 divisiones) y líneas horizontales (±6 dB + cero)
                        for i in 1..6 {
                            paint(
                                window,
                                width * i as f32 / 6.0,
                                0.0,
                                1.0,
                                height,
                                (ds.grid << 8) | 0x55,
                            );
                        }
                        paint(
                            window,
                            0.0,
                            height * 0.26,
                            width,
                            1.0,
                            (ds.grid << 8) | 0x35,
                        );
                        paint(
                            window,
                            0.0,
                            height * 0.74,
                            width,
                            1.0,
                            (ds.grid << 8) | 0x35,
                        );
                        paint(window, 0.0, mid, width, 1.0, (ds.cyan << 8) | 0x44);

                        // Banda de Loop resaltada
                        if params.looped {
                            let lx0 = x_px(params.get(LOOP_START)).clamp(0.0, width);
                            let lx1 = x_px(params.get(LOOP_END)).clamp(0.0, width);
                            if lx1 > lx0 {
                                paint(window, lx0, 0.0, lx1 - lx0, height, (ds.cyan << 8) | 0x1a);
                            }
                        }

                        // Onda Peak + RMS a resolución de 1 px sobre [v0, v1]
                        let total_frames = asset.file.frames.len();
                        if total_frames > 0 {
                            let f_start = ((f64::from(v0) / 100.0) * total_frames as f64) as usize;
                            let f_end = (((f64::from(v1) / 100.0) * total_frames as f64).ceil()
                                as usize)
                                .clamp(f_start + 1, total_frames);
                            let vis_frames = (f_end - f_start).max(1);
                            let scale_amp = asset.file.max_peak.max(0.05);
                            let sy =
                                |v: f32| height * (0.5 - 0.45 * (v / scale_amp).clamp(-1.0, 1.0));
                            let w_usize = width as usize;
                            for col in 0..w_usize {
                                let a = f_start
                                    + (col as u64 * vis_frames as u64 / w_usize as u64) as usize;
                                let b = (f_start
                                    + ((col + 1) as u64 * vis_frames as u64 / w_usize as u64)
                                        as usize)
                                    .max(a + 1)
                                    .min(total_frames);
                                let (lo, hi, rms) = if b - a <= PEAK_FRAMES {
                                    let mut lo = 0.0_f32;
                                    let mut hi = 0.0_f32;
                                    let mut sq = 0.0_f32;
                                    let mut cnt = 0u32;
                                    for &s in asset.file.frames[a..b].iter().flatten() {
                                        lo = lo.min(s);
                                        hi = hi.max(s);
                                        sq += s * s;
                                        cnt += 1;
                                    }
                                    let rms = if cnt > 0 {
                                        (sq / cnt as f32).sqrt()
                                    } else {
                                        0.0
                                    };
                                    (lo, hi, rms)
                                } else {
                                    let p0 = (a / PEAK_FRAMES)
                                        .min(asset.file.peaks.len().saturating_sub(1));
                                    let p1 = b
                                        .div_ceil(PEAK_FRAMES)
                                        .min(asset.file.peaks.len())
                                        .max(p0 + 1);
                                    let slice = &asset.file.peaks[p0..p1];
                                    let (lo, hi) = slice
                                        .iter()
                                        .step_by(slice.len().div_ceil(48).max(1))
                                        .fold((0.0_f32, 0.0_f32), |(l, h), p| {
                                            (l.min(p[0]), h.max(p[1]))
                                        });
                                    (lo, hi, hi.abs().max(lo.abs()) * 0.58)
                                };
                                let yt = sy(hi);
                                let yb = sy(lo);
                                paint(
                                    window,
                                    col as f32,
                                    yt,
                                    1.0,
                                    (yb - yt).max(1.0),
                                    (ds.accent << 8) | 0x68,
                                );
                                let rt = sy(rms);
                                let rb = sy(-rms);
                                paint(
                                    window,
                                    col as f32,
                                    rt,
                                    1.0,
                                    (rb - rt).max(1.0),
                                    (ds.accent << 8) | 0xf0,
                                );
                            }
                        }

                        // Envolvente visual Attack / Decay / Sustain sobre el rango [START, END]
                        let sx0 = x_px(params.get(START));
                        let sx1 = x_px(params.get(END));
                        if duration_sec > 0.001 && sx1 > sx0 + 2.0 {
                            let px_per_sec = width / (span_sec.max(0.001) as f32);
                            let atk_x = (sx0 + (params.get(ATTACK) / 1000.0) * px_per_sec).min(sx1);
                            let dec_x =
                                (atk_x + (params.get(DECAY) / 1000.0) * px_per_sec).min(sx1);
                            let sus_y = height * (1.0 - (params.get(SUSTAIN) / 100.0) * 0.82) - 4.0;
                            // Pequeños marcadores de envolvente en la parte superior
                            if atk_x > sx0 + 2.0 && atk_x < width {
                                paint(
                                    window,
                                    atk_x,
                                    4.0,
                                    1.0,
                                    height - 8.0,
                                    (ds.success << 8) | 0x55,
                                );
                            }
                            if dec_x > atk_x + 2.0 && dec_x < width {
                                paint(
                                    window,
                                    dec_x,
                                    sus_y.clamp(4.0, height - 6.0),
                                    (sx1 - dec_x).clamp(0.0, width - dec_x),
                                    1.5,
                                    (ds.success << 8) | 0x80,
                                );
                            }
                        }

                        // Atenuar fuera de [START, END]
                        let dim_l = sx0.clamp(0.0, width);
                        if dim_l > 0.0 {
                            paint(window, 0.0, 0.0, dim_l, height, 0x00000088);
                        }
                        let dim_r = sx1.clamp(0.0, width);
                        if dim_r < width {
                            paint(window, dim_r, 0.0, width - dim_r, height, 0x00000088);
                        }

                        // Marcadores Start / End / Loop Start / Loop End
                        let markers: [(f32, u32, bool); 4] = [
                            (params.get(START), ds.text, true),
                            (params.get(END), ds.text, true),
                            (params.get(LOOP_START), ds.cyan, params.looped),
                            (params.get(LOOP_END), ds.cyan, params.looped),
                        ];
                        for (idx, (pct, color, visible)) in markers.into_iter().enumerate() {
                            if !visible {
                                continue;
                            }
                            let mx = x_px(pct);
                            if !(-4.0..=width + 4.0).contains(&mx) {
                                continue;
                            }
                            let cx_pos = mx.clamp(0.0, width - 1.5);
                            paint(window, cx_pos, 0.0, 1.5, height, (color << 8) | 0xf0);
                            let handle_y = if idx >= 2 { height - 9.0 } else { 0.0 };
                            paint(
                                window,
                                (cx_pos - 4.0).clamp(0.0, width - 9.0),
                                handle_y,
                                9.0,
                                9.0,
                                (color << 8) | 0xff,
                            );
                        }

                        // Hover crosshair
                        if let Some(hp) = hover_pct {
                            let hx = x_px(hp);
                            if (0.0..=width).contains(&hx) {
                                paint(window, hx, 0.0, 1.0, height, (ds.text << 8) | 0x44);
                            }
                        }
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    let Some(bounds) = bounds_cell.get() else {
                        return;
                    };
                    let (v0, v1) = this.sampler_ui.visible_range();
                    let frac = (f32::from(event.position.x - bounds.left())
                        / f32::from(bounds.size.width).max(1.0))
                    .clamp(0.0, 1.0);
                    let pct = v0 + frac * (v1 - v0);
                    let start = this.sampler.params.get(START);
                    let end = this.sampler.params.get(END);
                    let ls = this.sampler.params.get(LOOP_START);
                    let le = this.sampler.params.get(LOOP_END);
                    let looped = this.sampler.params.looped;
                    let rel_y = f32::from(event.position.y - bounds.top())
                        / f32::from(bounds.size.height).max(1.0);

                    let handle = if event.modifiers.shift && pct >= start && pct <= end {
                        WaveHandle::SlideRange
                    } else if looped && (event.modifiers.alt || rel_y > 0.72) {
                        if (pct - ls).abs() <= (pct - le).abs() {
                            WaveHandle::LoopStart
                        } else {
                            WaveHandle::LoopEnd
                        }
                    } else if (pct - start).abs() <= (pct - end).abs() {
                        WaveHandle::Start
                    } else {
                        WaveHandle::End
                    };

                    this.sampler_ui.wave_drag = Some(WaveDrag {
                        handle,
                        bounds,
                        anchor_pct: pct,
                        initial_start: start,
                        initial_end: end,
                    });
                    match handle {
                        WaveHandle::Start => this.set_sample_param(START, pct, cx),
                        WaveHandle::End => this.set_sample_param(END, pct, cx),
                        WaveHandle::LoopStart => this.set_sample_param(LOOP_START, pct, cx),
                        WaveHandle::LoopEnd => this.set_sample_param(LOOP_END, pct, cx),
                        WaveHandle::SlideRange => {}
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_move(
                cx.listener(move |this, event: &gpui::MouseMoveEvent, _, cx| {
                    if let Some(bounds) = hover_bounds.get() {
                        let (v0, v1) = this.sampler_ui.visible_range();
                        let frac = (f32::from(event.position.x - bounds.left())
                            / f32::from(bounds.size.width).max(1.0))
                        .clamp(0.0, 1.0);
                        let next = Some(v0 + frac * (v1 - v0));
                        if this.sampler_ui.wave_hover != next {
                            this.sampler_ui.wave_hover = next;
                            cx.notify();
                        }
                    }
                }),
            )
            .on_scroll(cx.listener(move |this, ev: &gpui::ScrollEvent, _, cx| {
                let dy = match ev.delta {
                    ScrollDelta::Lines(p) => p.y,
                    ScrollDelta::Pixels(p) => f32::from(p.y) / 24.0,
                };
                if dy.abs() > 0.01 {
                    let anchor = scroll_bounds.get().map(|bounds| {
                        let (v0, v1) = this.sampler_ui.visible_range();
                        let frac = (f32::from(ev.position.x - bounds.left())
                            / f32::from(bounds.size.width).max(1.0))
                        .clamp(0.0, 1.0);
                        v0 + frac * (v1 - v0)
                    });
                    let factor = if dy > 0.0 { 0.70 } else { 1.42 };
                    this.zoom_sampler_wave(factor, anchor);
                    cx.stop_propagation();
                    cx.notify();
                }
            }));

        div()
            .size_full()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(ds.frame_border()))
            .rounded(px(ds.radius_sm))
            .overflow_hidden()
            .child(bar)
            .child(ruler)
            .child(canvas_el)
    }
}
