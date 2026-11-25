//! Editor de audio acoplado: onda Peak+RMS, regla de tiempo, escala dB, Tijeras y procesos.
use crate::{
    audio::{AudioClip, PEAK_FRAMES, sample_edit::Coefficients},
    ui::{
        DawApp,
        components::button::flat_control,
        components::curve_overlay::{
            CurveAnchor, CurveGesture, CurveModel, CurvePath, interactive_curve_box,
        },
        sample_editor::SampleTool,
    },
};
use elevate::musical::sample_edit::{Bell, Process};
use gpui::{
    Bounds, Context, Div, IntoElement, MouseButton, ScrollDelta, canvas,
    component::tooltip::Tooltip, div, fill, point, prelude::*, px, rgb, rgba, size,
};
use std::rc::Rc;

impl DawApp {
    fn sample_button(
        &self,
        label: &'static str,
        active: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut DawApp, &mut Context<DawApp>) + 'static,
    ) -> Div {
        flat_control(&self.ds, active, self.ds.accent)
            .px_2()
            .py(px(3.))
            .text_size(px(11.))
            .text_color(rgb(if active { self.ds.accent } else { self.ds.text }))
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    action(this, cx);
                    cx.stop_propagation();
                    cx.notify()
                }),
            )
    }

    fn sample_tool_button(
        &self,
        id: &'static str,
        label: &'static str,
        tip: &'static str,
        active: bool,
        accent_color: u32,
        cx: &mut Context<Self>,
        action: impl Fn(&mut DawApp, &mut Context<DawApp>) + 'static,
    ) -> gpui::AnyElement {
        div()
            .id(id)
            .px(px(7.))
            .py(px(3.))
            .rounded(px(4.))
            .border_1()
            .border_color(if active {
                rgb(accent_color)
            } else {
                rgba((self.ds.text << 8) | 0x20)
            })
            .bg(if active {
                rgba((accent_color << 8) | 0x2e)
            } else {
                rgba((self.ds.text << 8) | 0x0a)
            })
            .hover(move |b| b.bg(rgba((accent_color << 8) | 0x24)))
            .text_size(px(11.))
            .text_color(rgb(if active { accent_color } else { self.ds.text }))
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

    pub(super) fn render_sample_editor(
        &self,
        c: &AudioClip,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let e = &self.timeline.sample_editor;
        let ds = &self.ds;
        let rate = f64::from(c.file.sample_rate).max(1.0);
        let (a, b) = e.span(c, e.selection);
        let (va, vb) = e.span(c, e.zoom);
        let has_selection = e.selection.is_some();
        let tool = e.tool;
        let chord_cuts = e.chord_draft(c).map(|d| d.cuts.clone()).unwrap_or_default();
        let chord_count = chord_cuts.len().saturating_sub(1);
        let hover_frame = e.hover_frame;
        let edit_count = c.processing.edit.ranges.len();
        let play = e.listening.is_some()
            || e.audition
                .as_ref()
                .is_some_and(|a| a.finished() == Ok(false));
        let paused = e.audition.as_ref().is_some_and(|p| p.paused());
        let cursor = e
            .audition
            .as_ref()
            .filter(|p| p.finished() == Ok(false))
            .zip(e.preview_range)
            .map(|(p, (start, end))| {
                start + (p.position() * (end - start) as f64).round() as usize
            });
        let tab = e.tab;
        let file = c.file.clone();
        let bounds = e.bounds.clone();
        let accent = ds.accent;
        let signal = ds.cyan;
        let warning = ds.warning;
        let grid = ds.grid;
        let cursor_color = ds.text;
        let surface_bg = ds.arrangement_surface();
        let header_bg = ds.arrangement_header();
        let border_col = ds.frame_border();

        // Regla superior de tiempo (6 marcas equidistantes sobre [va, vb])
        let span_sec = vb.saturating_sub(va) as f64 / rate;
        let start_sec = va as f64 / rate;
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

        let time_ruler = div()
            .h(px(18.))
            .flex()
            .items_center()
            .bg(rgb(header_bg))
            .border_b_1()
            .border_color(rgb(border_col))
            .child(
                div()
                    .w(px(30.))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_r_1()
                    .border_color(rgb(border_col))
                    .text_size(px(9.))
                    .text_color(rgb(ds.muted))
                    .child("dB"),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .font_family("monospace")
                    .text_size(px(9.5))
                    .text_color(rgb(ds.muted))
                    .children(tick_labels.into_iter().map(|lbl| div().child(lbl))),
            );

        let db_scale = div()
            .w(px(30.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .justify_between()
            .items_end()
            .pr(px(4.))
            .py(px(3.))
            .bg(rgb(header_bg))
            .border_r_1()
            .border_color(rgb(border_col))
            .font_family("monospace")
            .text_size(px(8.5))
            .text_color(rgb(ds.muted))
            .child("0")
            .child("-6")
            .child("−∞")
            .child("-6")
            .child("0");

        let wave_canvas = div()
            .id("sample-editor-wave")
            .flex_1()
            .h_full()
            .bg(rgb(surface_bg))
            .cursor(match tool {
                SampleTool::Select => gpui::CursorStyle::IBeam,
                SampleTool::Scissors => gpui::CursorStyle::Crosshair,
            })
            .child(
                canvas(
                    move |rect, _, _| bounds.set(rect),
                    move |rect, _, win, _| {
                        let width = f32::from(rect.size.width).max(1.) as usize;
                        let height = f32::from(rect.size.height);
                        let scale = width as f64 / vb.saturating_sub(va).max(1) as f64;
                        let xpos = |frame: usize| {
                            ((frame as f64 - va as f64) * scale).clamp(0., width as f64) as f32
                        };
                        let paint =
                            |win: &mut gpui::Window, x: f32, y: f32, w: f32, h: f32, col: u32| {
                                if w > 0.0 && h > 0.0 {
                                    win.paint_quad(fill(
                                        Bounds::new(
                                            point(rect.left() + px(x), rect.top() + px(y)),
                                            size(px(w), px(h)),
                                        ),
                                        rgba(col),
                                    ))
                                }
                            };

                        // Rejilla vertical (6 divisiones sincronizadas con la regla superior)
                        for i in 1..6 {
                            let gx = width as f32 * i as f32 / 6.0;
                            paint(win, gx, 0., 1., height, (grid << 8) | 0x60);
                        }
                        // Líneas horizontales de referencia: ±6 dB (0.5) y cruce por cero
                        paint(
                            win,
                            0.,
                            height * 0.265,
                            width as f32,
                            1.,
                            (grid << 8) | 0x45,
                        );
                        paint(
                            win,
                            0.,
                            height * 0.735,
                            width as f32,
                            1.,
                            (grid << 8) | 0x45,
                        );
                        paint(
                            win,
                            0.,
                            height * 0.5,
                            width as f32,
                            1.,
                            (signal << 8) | 0x44,
                        );

                        // Fondo de selección activa
                        if has_selection {
                            let sx0 = xpos(a);
                            let sx1 = xpos(b);
                            paint(win, sx0, 0., sx1 - sx0, height, (accent << 8) | 0x1e);
                        }

                        // Onda de alta resolución: Peak translúcido + núcleo RMS brillante
                        let sy = |v: f32| height * (0.5 - 0.47 * v.clamp(-1., 1.));
                        for x in 0..width {
                            let start = va + (x as f64 / scale) as usize;
                            let end = (va + ((x + 1) as f64 / scale).ceil() as usize).min(vb);
                            if start >= end {
                                continue;
                            }
                            let (lo, hi, rms) = if end - start <= PEAK_FRAMES {
                                let mut lo = 0f32;
                                let mut hi = 0f32;
                                let mut sum_sq = 0f32;
                                let mut count = 0u32;
                                for &v in file.frames[start..end].iter().flatten() {
                                    lo = lo.min(v);
                                    hi = hi.max(v);
                                    sum_sq += v * v;
                                    count += 1;
                                }
                                let rms = if count > 0 {
                                    (sum_sq / count as f32).sqrt()
                                } else {
                                    0.0
                                };
                                (lo, hi, rms)
                            } else {
                                let slice =
                                    &file.peaks[start / PEAK_FRAMES..end.div_ceil(PEAK_FRAMES)];
                                let (lo, hi) = slice
                                    .iter()
                                    .step_by(slice.len().div_ceil(64).max(1))
                                    .fold((0f32, 0f32), |(l, h), p| (l.min(p[0]), h.max(p[1])));
                                let rms = (hi.abs().max(lo.abs())) * 0.58;
                                (lo, hi, rms)
                            };
                            let y_top = sy(hi);
                            let y_bot = sy(lo);
                            // Cuerpo externo Peak
                            paint(
                                win,
                                x as f32,
                                y_top,
                                1.0,
                                (y_bot - y_top).max(1.0),
                                (signal << 8) | 0x66,
                            );
                            // Núcleo RMS brillante
                            let r_top = sy(rms);
                            let r_bot = sy(-rms);
                            paint(
                                win,
                                x as f32,
                                r_top,
                                1.0,
                                (r_bot - r_top).max(1.0),
                                (signal << 8) | 0xf2,
                            );
                        }

                        // Atenuar lo que queda fuera de la selección + bordes con tiradores
                        if has_selection {
                            let sx0 = xpos(a);
                            let sx1 = xpos(b);
                            paint(win, 0., 0., sx0, height, 0x00000058);
                            paint(
                                win,
                                sx1,
                                0.,
                                (width as f32 - sx1).max(0.),
                                height,
                                0x00000058,
                            );
                            for x in [sx0, sx1] {
                                let ex = x.min(width.saturating_sub(2) as f32);
                                paint(win, ex, 0., 1.5, height, (accent << 8) | 0xe5);
                                paint(win, (ex - 3.).max(0.), 0., 7., 5., (accent << 8) | 0xff);
                            }
                        }

                        // Guía de hover (Tijeras en ámbar / Selección sutil)
                        if let Some(frame) = hover_frame.filter(|f| (va..=vb).contains(f)) {
                            let hx = xpos(frame);
                            match tool {
                                SampleTool::Scissors => {
                                    paint(win, hx, 0., 1.5, height, (warning << 8) | 0xee);
                                    paint(
                                        win,
                                        (hx - 4.).max(0.),
                                        0.,
                                        9.,
                                        6.,
                                        (warning << 8) | 0xff,
                                    );
                                }
                                SampleTool::Select => {
                                    paint(win, hx, 0., 1.0, height, (cursor_color << 8) | 0x38);
                                }
                            }
                        }

                        for &frame in chord_cuts
                            .iter()
                            .skip(1)
                            .take(chord_count.saturating_sub(1))
                        {
                            if (va..=vb).contains(&frame) {
                                let x = xpos(frame);
                                paint(win, x, 0., 1.5, height, (warning << 8) | 0xc0);
                                paint(win, x - 3., 0., 7., 6., (warning << 8) | 0xff);
                            }
                        }
                        // Cursor de preescucha
                        if let Some(frame) = cursor.filter(|f| (va..=vb).contains(f)) {
                            let cx_pos = xpos(frame);
                            paint(win, cx_pos, 0., 1.5, height, (cursor_color << 8) | 0xff);
                            paint(
                                win,
                                (cx_pos - 3.).max(0.),
                                0.,
                                7.,
                                4.,
                                (cursor_color << 8) | 0xff,
                            );
                        }
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                    if let Some(p) = this.sample_position(ev.position.x) {
                        if chord_count > 0 {
                            let e = &mut this.timeline.sample_editor;
                            let tolerance = (8. * (vb - va) as f32
                                / f32::from(e.bounds.get().size.width).max(1.))
                                as usize;
                            if let Some(d) = &mut e.chords {
                                d.dragging = (1..d.cuts.len() - 1)
                                    .min_by_key(|i| d.cuts[*i].abs_diff(p))
                                    .filter(|i| d.cuts[*i].abs_diff(p) <= tolerance);
                            }
                        } else if this.timeline.sample_editor.tool == SampleTool::Scissors {
                            this.split_sample_at_frame(p);
                        } else {
                            let e = &mut this.timeline.sample_editor;
                            if ev.click_count == 2 {
                                e.selection = None;
                                e.anchor = None;
                            } else {
                                e.anchor = Some(p);
                            }
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _, cx| {
                let pos = this.sample_position(ev.position.x);
                let prev = this.timeline.sample_editor.hover_frame;
                this.timeline.sample_editor.hover_frame = pos;
                let mut dirty = pos != prev;
                if chord_count > 0 {
                    let e = &mut this.timeline.sample_editor;
                    if ev.dragging()
                        && let Some(d) = &mut e.chords
                        && let Some(i) = d.dragging
                        && let Some(p) = pos
                    {
                        let value = p.clamp(d.cuts[i - 1] + 1, d.cuts[i + 1] - 1);
                        if d.cuts[i] != value {
                            d.cuts[i] = value;
                            e.silence();
                            dirty = true;
                        }
                    }
                } else if ev.dragging()
                    && this.timeline.sample_editor.tool == SampleTool::Select
                    && let (Some(a), Some(b)) = (this.timeline.sample_editor.anchor, pos)
                    && a != b
                {
                    this.timeline.sample_editor.selection = Some((a.min(b), a.max(b)));
                    dirty = true;
                }
                if dirty {
                    cx.notify();
                }
            }))
            .on_scroll(cx.listener(|this, ev: &gpui::ScrollEvent, _, cx| {
                let dy = match ev.delta {
                    ScrollDelta::Lines(p) => p.y,
                    ScrollDelta::Pixels(p) => f32::from(p.y) / 24.0,
                };
                if dy.abs() > 0.01 {
                    let factor = if dy > 0.0 { 0.72 } else { 1.38 };
                    let anchor = this.sample_position(ev.position.x);
                    this.zoom_sample_by(factor, anchor);
                    cx.stop_propagation();
                    cx.notify();
                }
            }));

        let wave_box = div()
            .flex_shrink_0()
            .rounded(px(ds.radius_md))
            .border_1()
            .border_color(rgb(border_col))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(time_ruler)
            .child(div().h(px(172.)).flex().child(db_scale).child(wave_canvas));

        // Barra de herramientas principal
        let toolbar = div()
            .flex()
            .items_center()
            .justify_between()
            .flex_wrap()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_1()
                    // Transporte
                    .child(self.sample_tool_button(
                        "sample-play",
                        if play && !paused { "⏸" } else { "▶" },
                        "Preescuchar selección o clip",
                        play && !paused,
                        ds.success,
                        cx,
                        |s, c| s.listen_sample(c),
                    ))
                    .child(self.sample_tool_button(
                        "sample-stop",
                        "⏹",
                        "Detener preescucha",
                        false,
                        ds.muted,
                        cx,
                        |s, _| s.timeline.sample_editor.silence(),
                    ))
                    .child(self.sample_tool_button(
                        "sample-rewind",
                        "⏮",
                        "Volver al inicio",
                        false,
                        ds.muted,
                        cx,
                        |s, _| {
                            if let Some(p) = s
                                .timeline
                                .sample_editor
                                .audition
                                .as_ref()
                                .filter(|p| p.finished() == Ok(false))
                            {
                                p.rewind()
                            }
                        },
                    ))
                    .child(self.sample_tool_button(
                        "sample-loop",
                        "↻",
                        "Alternar bucle de preescucha",
                        e.looping,
                        ds.cyan,
                        cx,
                        |s, _| {
                            let e = &mut s.timeline.sample_editor;
                            e.looping = !e.looping;
                            if let Some(p) = &e.audition {
                                p.set_looping(e.looping)
                            }
                        },
                    ))
                    // Separador
                    .child(div().w(px(1.)).h(px(16.)).mx(px(2.)).bg(rgb(border_col)))
                    // Herramientas: Selección y Tijeras
                    .child(self.sample_tool_button(
                        "sample-tool-select",
                        "⌶ Sel",
                        "Herramienta Selección: arrastra sobre la onda para marcar rango",
                        tool == SampleTool::Select,
                        ds.accent,
                        cx,
                        |s, _| s.timeline.sample_editor.tool = SampleTool::Select,
                    ))
                    .child(self.sample_tool_button(
                        "sample-tool-scissors",
                        "✂ Tijeras",
                        "Herramienta Tijeras: haz clic en la onda para cortar el clip en ese punto",
                        tool == SampleTool::Scissors,
                        ds.warning,
                        cx,
                        |s, _| s.timeline.sample_editor.tool = SampleTool::Scissors,
                    ))
                    // Separador
                    .child(div().w(px(1.)).h(px(16.)).mx(px(2.)).bg(rgb(border_col)))
                    // Procesos rápidos
                    .when(has_selection, |row| {
                        row.child(self.sample_tool_button(
                            "sample-crop",
                            "Recortar",
                            "Recortar el clip en el Arrangement a la selección actual",
                            false,
                            ds.accent,
                            cx,
                            |s, _| s.crop_sample_to_selection(),
                        ))
                    })
                    .child(self.sample_tool_button(
                        "sample-fade-in",
                        "Fade In",
                        "Aplicar fundido de entrada a la selección o al clip",
                        false,
                        ds.cyan,
                        cx,
                        |s, c| s.sample_operation(Some(Process::FadeIn), c),
                    ))
                    .child(self.sample_tool_button(
                        "sample-fade-out",
                        "Fade Out",
                        "Aplicar fundido de salida a la selección o al clip",
                        false,
                        ds.cyan,
                        cx,
                        |s, c| s.sample_operation(Some(Process::FadeOut), c),
                    ))
                    .child(self.sample_tool_button(
                        "sample-silence",
                        "Silenciar",
                        "Silenciar el rango seleccionado",
                        false,
                        ds.muted,
                        cx,
                        |s, c| s.sample_operation(Some(Process::Silence), c),
                    ))
                    .child(self.sample_tool_button(
                        "sample-reverse",
                        "Invertir",
                        "Invertir audio en el rango",
                        false,
                        ds.accent,
                        cx,
                        |s, c| s.sample_operation(Some(Process::Reverse), c),
                    ))
                    .child(self.sample_tool_button(
                        "sample-normalize",
                        "Norm",
                        "Normalizar picos a -0.1 dBFS",
                        false,
                        ds.accent,
                        cx,
                        |s, c| s.sample_operation(Some(Process::Normalize), c),
                    ))
                    .child(self.sample_tool_button(
                        "sample-audio-to-midi",
                        if self.audio_to_midi_task.is_some() {
                            "⏳ MIDI…"
                        } else {
                            "♪ Audio → MIDI"
                        },
                        "Transcribir el clip o selección a MIDI polifónico (Basic Pitch)",
                        self.audio_to_midi_task.is_some(),
                        ds.success,
                        cx,
                        |s, c| s.convert_audio_clip_to_midi(c),
                    ))
                    .when(edit_count > 0, |row| {
                        row.child(self.sample_tool_button(
                            "sample-undo",
                            "↶ Deshacer",
                            "Deshacer el último proceso aplicado",
                            false,
                            ds.warning,
                            cx,
                            |s, c| s.sample_operation(None, c),
                        ))
                    }),
            )
            // Controles de zoom a la derecha
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.sample_tool_button(
                        "sample-zoom-out",
                        "−",
                        "Alejar onda (o rueda del ratón)",
                        false,
                        ds.muted,
                        cx,
                        |s, _| s.zoom_sample_by(1.6, None),
                    ))
                    .child(self.sample_tool_button(
                        "sample-zoom-in",
                        "+",
                        "Acercar onda (o rueda del ratón)",
                        false,
                        ds.muted,
                        cx,
                        |s, _| s.zoom_sample_by(0.6, None),
                    ))
                    .child(self.sample_tool_button(
                        "sample-zoom-fit",
                        if e.zoom.is_some() { "100%" } else { "Ajustar" },
                        "Ampliar a la selección o ver clip completo",
                        e.zoom.is_some(),
                        ds.accent,
                        cx,
                        |s, _| {
                            let e = &mut s.timeline.sample_editor;
                            e.zoom = if e.zoom.is_some() { None } else { e.selection }
                        },
                    )),
            );

        // Barra de telemetría debajo de la onda
        let cursor_frame = cursor.or(hover_frame).unwrap_or(a);
        let status_bar = div()
            .flex()
            .items_center()
            .justify_between()
            .flex_wrap()
            .gap_2()
            .px_1()
            .font_family("monospace")
            .text_size(px(10.5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_color(rgb(ds.cyan))
                            .child(format!("Cursor {:.3} s", cursor_frame as f64 / rate)),
                    )
                    .child(div().text_color(rgb(ds.accent)).child(format!(
                        "Rango {:.3}–{:.3} s ({:.0} ms)",
                        a as f64 / rate,
                        b as f64 / rate,
                        (b - a) as f64 * 1000. / rate
                    ))),
            )
            .child(
                div()
                    .text_color(rgb(if tool == SampleTool::Scissors {
                        ds.warning
                    } else {
                        ds.muted
                    }))
                    .child(if tool == SampleTool::Scissors {
                        "✂ Clic en la onda para dividir el clip"
                    } else if has_selection {
                        "Doble clic: limpiar selección · Rueda: zoom"
                    } else {
                        "Arrastra: seleccionar · Rueda: zoom"
                    }),
            );

        let track_index = c.track;
        let tabs = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(self.sample_button("EQ del clip", tab == 2, cx, |s, _| {
                s.timeline.sample_editor.tab = 2
            }))
            .child(self.sample_button("Afinación", tab == 3, cx, |s, _| {
                s.timeline.sample_editor.tab = 3
            }))
            .child(self.sample_button("Tejer EQ", false, cx, move |s, _| {
                s.select_tejer_band(track_index, None)
            }))
            .when(has_selection, |row| {
                row.child(self.sample_button("Limpiar selección", false, cx, |s, _| {
                    s.timeline.sample_editor.selection = None
                }))
            });

        let peer = div();
        #[cfg(feature = "p2p")]
        let peer = peer.children(
            self.timeline
                .peer_clip
                .as_ref()
                .filter(|(id, _)| *id == c.id.value())
                .map(|(_, name)| div().text_color(rgb(ds.cyan)).child(name.clone())),
        );

        let dur_sec = c.length_seconds();
        div()
            .id("sample-edit-dock")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(ds.panel))
            .text_color(rgb(ds.text))
            .text_sm()
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    // Encabezado compacto
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(1.))
                                            .rounded(px(3.))
                                            .bg(rgba((ds.cyan << 8) | 0x24))
                                            .text_size(px(10.))
                                            .text_color(rgb(ds.cyan))
                                            .child("AUDIO"),
                                    )
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(c.name.clone()),
                                    )
                                    .child(div().text_xs().text_color(rgb(ds.muted)).child(
                                        format!("{:.1} kHz · {:.2} s", rate / 1000.0, dur_sec),
                                    )),
                            )
                            .child(div().flex().items_center().gap_2().child(peer).child(
                                self.sample_tool_button(
                                    "sample-close",
                                    "✕",
                                    "Cerrar editor de audio",
                                    false,
                                    ds.muted,
                                    cx,
                                    |s, _| {
                                        let e = &mut s.timeline.sample_editor;
                                        e.silence();
                                        e.id = None;
                                        e.draft = None;
                                    },
                                ),
                            )),
                    )
                    .when_else(
                        chord_count > 0,
                        |panel| {
                            panel.child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_1()
                                    .child(format!("{chord_count} tramos · Arrastra un corte"))
                                    .child(self.sample_button("Preescuchar", play, cx, |s, c| {
                                        s.listen_sample(c)
                                    }))
                                    .child(self.sample_button("Aplicar", true, cx, |s, c| {
                                        s.apply_chords(c)
                                    }))
                                    .child(self.sample_button("Cancelar", false, cx, |s, _| {
                                        s.timeline.sample_editor.silence();
                                        s.timeline.sample_editor.chords = None;
                                    })),
                            )
                        },
                        |panel| panel.child(toolbar),
                    )
                    .child(wave_box)
                    .child(status_bar)
                    .child(tabs)
                    .when(tab == 3, |v| v.child(self.sample_tuning(c, cx)))
                    .when(tab != 3, |v| v.child(self.sample_bell(c, cx)))
                    .when(c.props.locked, |v| {
                        v.child(
                            div()
                                .text_xs()
                                .text_color(rgb(ds.warning))
                                .child("Clip bloqueado"),
                        )
                    })
                    .when(e.job.is_some(), |v| {
                        v.child(
                            div()
                                .text_xs()
                                .text_color(rgb(ds.cyan))
                                .child("Procesando audio…"),
                        )
                    }),
            )
    }

    fn sample_tuning(&self, c: &AudioClip, cx: &mut Context<Self>) -> Div {
        use crate::ui::components::slider::param_slider;
        let tuning = self
            .timeline
            .sample_editor
            .tuning
            .unwrap_or((c.props.pitch_semitones, c.processing.edit.fine_cents));
        let mut row = div().flex().flex_wrap().gap_3();
        for (label, value, limit, fine) in [
            ("Transposición", tuning.0, 24_i8, false),
            ("Tune fino", tuning.1, 100_i8, true),
        ] {
            let unit = if fine { "ct" } else { "st" };
            let change = move |s: &mut DawApp, v: i8| {
                if let Some(c) = s.shown_sample().filter(|c| !c.props.locked) {
                    let mut pair = s
                        .timeline
                        .sample_editor
                        .tuning
                        .unwrap_or((c.props.pitch_semitones, c.processing.edit.fine_cents));
                    if fine {
                        pair.1 = v
                    } else {
                        pair.0 = v
                    }
                    s.timeline.sample_editor.tuning = Some(pair);
                }
            };
            row = row.child(param_slider(
                &self.ds,
                label.into(),
                format!("{value:+} {unit}"),
                (f32::from(value) / f32::from(limit) + 1.) / 2.,
                Rc::new(cx.listener(move |s, &fraction: &f32, _, cx| {
                    change(s, ((fraction * 2. - 1.) * f32::from(limit)).round() as i8);
                    cx.notify()
                })),
                Rc::new(cx.listener(move |s, _: &(), _, cx| {
                    change(s, 0);
                    cx.notify()
                })),
            ));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(row)
            .child(div().flex().child(self.sample_button(
                "Aplicar afinación",
                self.timeline.sample_editor.tuning.is_some(),
                cx,
                |s, cx| s.apply_sample_tuning(cx),
            )))
    }

    fn sample_bell(&self, c: &AudioClip, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let b = self
            .timeline
            .sample_editor
            .draft
            .unwrap_or(c.processing.edit.bell);
        let rate = c.file.sample_rate;
        let max_hz = (f64::from(rate) * 0.49).clamp(21., 20000.);
        let span = max_hz / 20.;
        let hz = move |x: f32| 20. * span.powf(f64::from(x));
        let coeff = Coefficients::bell(b, rate);
        let points = (0..=128)
            .map(|i| {
                let x = i as f32 / 128.;
                (x, (0.5 + coeff.db(hz(x), rate) / 36.) as f32)
            })
            .collect();
        let model = CurveModel {
            wave: Vec::new(),
            wave_dim_until: 0.0,
            spectrum: Vec::new(),
            header: "Campana · rueda para Q".into(),
            readout: format!("{:.0} Hz   {:+.1} dB   Q {:.2}", b.hz, b.db, b.q).into(),
            curves: vec![CurvePath {
                points,
                color: self.ds.cyan,
                stroke_px: 2.,
                fill_alpha: 32,
            }],
            grid_x: vec![0.25, 0.5, 0.75],
            grid_y: vec![0.25, 0.75],
            baseline: Some(0.5),
            anchors: vec![CurveAnchor {
                x: ((b.hz / 20.).ln() / span.ln()).clamp(0., 1.) as f32,
                y: (0.5 + b.db / 36.) as f32,
                stem: false,
                crosshair: false,
                color: self.ds.accent,
            }],
            x_labels: vec!["20 Hz".into(), format!("{:.1} kHz", max_hz / 1000.).into()],
            height_px: 110.,
        };
        let id = c.id;
        let weak = cx.weak_entity();
        let curve = interactive_curve_box(
            ("sample-bell", id.value() as usize).into(),
            &self.ds,
            model,
            Rc::new(move |g, _, cx| {
                let _ = weak.update(cx, |s, cx| {
                    let Some(clip) = s.shown_sample().filter(|c| c.id == id && !c.props.locked)
                    else {
                        return;
                    };
                    let mut bell = clip.processing.edit.bell;
                    match *g {
                        CurveGesture::Move { x, y } => {
                            bell.hz = hz(x);
                            bell.db = f64::from(y * 36. - 18.);
                            s.timeline.sample_editor.draft = Some(bell);
                        }
                        CurveGesture::Commit => {
                            if let Some(draft) = s.timeline.sample_editor.draft.take() {
                                s.commit_bell(draft, cx)
                            }
                        }
                        CurveGesture::Wheel { delta, fine } => {
                            bell.q = (bell.q + f64::from(delta) * if fine { 0.05 } else { 0.25 })
                                .clamp(0.25, 8.);
                            s.commit_bell(bell, cx)
                        }
                        CurveGesture::Reset => {
                            s.timeline.sample_editor.draft = None;
                            s.commit_bell(Bell::default(), cx)
                        }
                    }
                    cx.notify();
                });
            }),
        );
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(curve)
            .child(self.sample_button(
                if b.enabled {
                    "EQ activo"
                } else {
                    "EQ en bypass"
                },
                b.enabled,
                cx,
                |s, cx| {
                    if let Some(clip) = s.shown_sample() {
                        let mut bell = clip.processing.edit.bell;
                        bell.enabled = !bell.enabled;
                        s.commit_bell(bell, cx)
                    }
                },
            ))
    }
}
