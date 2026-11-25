//! Knobs y tarjeta combinada Hay Filter + EQ Paramétrico de 3 Bandas para Hay Sampler.
use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        curve_overlay::{
            CurveAnchor, CurveGesture, CurveModel, CurvePath, GestureHandler, interactive_curve_box,
        },
        knob::{KnobSpec, rotary_knob},
    },
};
use elevate::{
    devices::{
        filter::FilterMode,
        sampler::{
            CUTOFF, EQ_HIGH_FREQ, EQ_HIGH_GAIN, EQ_LOW_FREQ, EQ_LOW_GAIN, EQ_MID_FREQ, EQ_MID_GAIN,
            PAN, PARAMS, PITCH_ENV, RESONANCE, ROOT, SamplerParams, TUNE, VEL_FILTER, VOICES,
        },
    },
    primitives::filter::{Filter, MODE_BELL},
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(super) fn sample_knob(
        &self,
        id: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let spec = PARAMS[id];
        let value = self.sampler.params.get(id);
        let text = if id == ROOT {
            elevate::musical::pitch::note_name(value as u8)
        } else if id == VOICES {
            format!("{}", value as u32)
        } else if spec.unit == "Hz" {
            if value >= 1000.0 {
                format!("{:.2} kHz", value / 1000.0)
            } else {
                format!("{value:.0} Hz")
            }
        } else if spec.unit == "dB" {
            format!("{value:+.1} dB")
        } else {
            format!("{value:.1} {}", spec.unit)
        };
        div()
            .id(("sampler-param", id))
            .w(px(82.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .items_center()
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(div().text_color(rgb(ds.muted)).child(spec.name))
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction: self.sampler.params.fraction(id),
                    bipolar: matches!(
                        id,
                        TUNE | PAN
                            | PITCH_ENV
                            | VEL_FILTER
                            | EQ_LOW_GAIN
                            | EQ_MID_GAIN
                            | EQ_HIGH_GAIN
                    ),
                    color: if matches!(
                        id,
                        EQ_LOW_FREQ
                            | EQ_LOW_GAIN
                            | EQ_MID_FREQ
                            | EQ_MID_GAIN
                            | EQ_HIGH_FREQ
                            | EQ_HIGH_GAIN
                    ) {
                        ds.cyan
                    } else {
                        ds.accent
                    },
                    diameter: 25.,
                },
            ))
            .child(div().text_color(rgb(ds.cyan)).child(text))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count == 2 {
                        this.set_sample_param(id, PARAMS[id].default, cx);
                    } else {
                        this.sampler_ui.drag =
                            Some((id, event.position.y, this.sampler.params.fraction(id)));
                    }
                    cx.stop_propagation();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                let step = match id {
                    ROOT | VOICES => 1.,
                    TUNE | PITCH_ENV => {
                        if event.modifiers.shift {
                            0.1
                        } else {
                            1.
                        }
                    }
                    _ => {
                        (PARAMS[id].max - PARAMS[id].min)
                            / if event.modifiers.shift { 1000. } else { 100. }
                    }
                };
                if delta != 0. {
                    this.set_sample_param(
                        id,
                        this.sampler.params.get(id) + delta.signum() * step,
                        cx,
                    );
                }
                cx.stop_propagation();
            }))
    }

    /// Tarjeta combinada Hay Filter + EQ Paramétrico de 3 Bandas (Low / Mid / High) en vivo.
    pub(super) fn sample_filter_card(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.sampler.params;
        let band = self.sampler_ui.eq_band.min(3);
        let cutoff = params.get(CUTOFF);
        let res_db = params.get(RESONANCE);
        let q = (0.707 * 10.0_f32.powf(res_db / 24.0)).clamp(0.5, 10.0);
        let mode_idx = match params.filter_mode {
            FilterMode::LowPass => 0.0,
            FilterMode::HighPass => 1.0,
            FilterMode::BandPass => 2.0,
        };
        let low_f = params.get(EQ_LOW_FREQ);
        let low_g = params.get(EQ_LOW_GAIN);
        let mid_f = params.get(EQ_MID_FREQ);
        let mid_g = params.get(EQ_MID_GAIN);
        let hi_f = params.get(EQ_HIGH_FREQ);
        let hi_g = params.get(EQ_HIGH_GAIN);

        let filter_resp = if params.filter {
            Filter::response(48_000.0, [cutoff, q, mode_idx])
        } else {
            (0..160).map(|i| (i as f32 / 159.0, 0.0_f32)).collect()
        };
        let eq1 = Filter::response_eq(48_000.0, [low_f, 0.95, MODE_BELL as f32, low_g]);
        let eq2 = Filter::response_eq(48_000.0, [mid_f, 1.20, MODE_BELL as f32, mid_g]);
        let eq3 = Filter::response_eq(48_000.0, [hi_f, 0.95, MODE_BELL as f32, hi_g]);

        let min_db = -36.0_f32;
        let max_db = 24.0_f32;
        let db_span = max_db - min_db;
        let db_to_y = |db: f32| ((db - min_db) / db_span).clamp(0.0, 1.0);
        let hz_to_x = |hz: f32| ((hz / 20.0).ln() / 1000.0_f32.ln()).clamp(0.0, 1.0);

        let points: Vec<(f32, f32)> = (0..160)
            .map(|i| {
                let x = filter_resp[i].0;
                let total_db = filter_resp[i].1 + eq1[i].1 + eq2[i].1 + eq3[i].1;
                (x, db_to_y(total_db))
            })
            .collect();

        let eq_only_points: Vec<(f32, f32)> = (0..160)
            .map(|i| {
                let x = eq1[i].0;
                let eq_db = eq1[i].1 + eq2[i].1 + eq3[i].1;
                (x, db_to_y(eq_db))
            })
            .collect();

        let anchors = vec![
            CurveAnchor {
                x: hz_to_x(cutoff),
                y: db_to_y(res_db.min(20.0)),
                stem: false,
                crosshair: band == 0,
                color: if band == 0 {
                    ds.cyan
                } else if params.filter {
                    ds.accent
                } else {
                    ds.muted
                },
            },
            CurveAnchor {
                x: hz_to_x(low_f),
                y: db_to_y(low_g),
                stem: band == 1 || low_g.abs() > 0.2,
                crosshair: band == 1,
                color: if band == 1 { ds.cyan } else { ds.warning },
            },
            CurveAnchor {
                x: hz_to_x(mid_f),
                y: db_to_y(mid_g),
                stem: band == 2 || mid_g.abs() > 0.2,
                crosshair: band == 2,
                color: if band == 2 { ds.cyan } else { ds.success },
            },
            CurveAnchor {
                x: hz_to_x(hi_f),
                y: db_to_y(hi_g),
                stem: band == 3 || hi_g.abs() > 0.2,
                crosshair: band == 3,
                color: if band == 3 { ds.cyan } else { ds.accent },
            },
        ];

        let fmt_hz = |hz: f32| {
            if hz >= 1000.0 {
                format!("{:.2} kHz", hz / 1000.0)
            } else {
                format!("{hz:.0} Hz")
            }
        };
        let mode_label = match params.filter_mode {
            FilterMode::LowPass => "LP 12",
            FilterMode::HighPass => "HP 12",
            FilterMode::BandPass => "BP 12",
        };
        let readout = match band {
            1 => format!("EQ Low · {} · {low_g:+.1} dB", fmt_hz(low_f)),
            2 => format!("EQ Mid · {} · {mid_g:+.1} dB", fmt_hz(mid_f)),
            3 => format!("EQ High · {} · {hi_g:+.1} dB", fmt_hz(hi_f)),
            _ => format!("{mode_label} · {} · {res_db:.1} dB", fmt_hz(cutoff)),
        };
        let header = match band {
            0 => "Filtro · arrastra Hz/Res · Shift: fino",
            _ => "EQ 3 Bandas · arrastra Hz/dB · Doble clic: reset",
        };

        let grid_x = [100_f32, 1000., 10000.].into_iter().map(hz_to_x).collect();
        let grid_y = [12.0_f32, 0.0, -12.0, -24.0]
            .into_iter()
            .map(db_to_y)
            .collect();

        let has_eq = low_g.abs() > 0.1 || mid_g.abs() > 0.1 || hi_g.abs() > 0.1;
        let mut curves = Vec::with_capacity(2);
        if has_eq && params.filter {
            curves.push(CurvePath {
                points: eq_only_points,
                color: ds.cyan,
                stroke_px: 1.2,
                fill_alpha: 0x10,
            });
        }
        curves.push(CurvePath {
            points,
            color: if params.filter || has_eq {
                ds.accent
            } else {
                ds.muted
            },
            stroke_px: 2.2,
            fill_alpha: if params.filter || has_eq { 0x26 } else { 0x0c },
        });

        let model = CurveModel {
            wave: Vec::new(),
            wave_dim_until: 0.0,
            spectrum: Vec::new(),
            grid_x,
            grid_y,
            baseline: Some(db_to_y(0.0)),
            curves,
            anchors,
            header: header.into(),
            readout: readout.into(),
            x_labels: ["20 Hz", "200", "2k", "20k"].map(Into::into).to_vec(),
            height_px: 124.0,
        };

        let on_gesture: GestureHandler =
            Rc::new(cx.listener(move |this, gesture: &CurveGesture, _, cx| {
                let active_band = this.sampler_ui.eq_band.min(3);
                match *gesture {
                    CurveGesture::Move { x, y, .. } => {
                        let hz = 20.0 * 1000.0_f32.powf(x.clamp(0.0, 1.0));
                        let db = min_db + y.clamp(0.0, 1.0) * db_span;
                        match active_band {
                            0 => {
                                if !this.sampler.params.filter {
                                    this.sampler.params.filter = true;
                                    this.prepare_sampler(cx);
                                }
                                this.set_sample_param(CUTOFF, hz, cx);
                                this.set_sample_param(
                                    RESONANCE,
                                    SamplerParams::value_at(RESONANCE, y),
                                    cx,
                                );
                            }
                            1 => {
                                this.set_sample_param(EQ_LOW_FREQ, hz, cx);
                                this.set_sample_param(EQ_LOW_GAIN, db, cx);
                            }
                            2 => {
                                this.set_sample_param(EQ_MID_FREQ, hz, cx);
                                this.set_sample_param(EQ_MID_GAIN, db, cx);
                            }
                            _ => {
                                this.set_sample_param(EQ_HIGH_FREQ, hz, cx);
                                this.set_sample_param(EQ_HIGH_GAIN, db, cx);
                            }
                        }
                    }
                    CurveGesture::Wheel { delta, fine, .. } => {
                        let step = if fine { 0.01 } else { 0.04 };
                        let target_id = match active_band {
                            1 => EQ_LOW_GAIN,
                            2 => EQ_MID_GAIN,
                            3 => EQ_HIGH_GAIN,
                            _ => RESONANCE,
                        };
                        let next = (this.sampler.params.fraction(target_id) + delta * step)
                            .clamp(0.0, 1.0);
                        this.set_sample_param(
                            target_id,
                            SamplerParams::value_at(target_id, next),
                            cx,
                        );
                    }
                    CurveGesture::Reset => match active_band {
                        1 => {
                            this.set_sample_param(EQ_LOW_FREQ, PARAMS[EQ_LOW_FREQ].default, cx);
                            this.set_sample_param(EQ_LOW_GAIN, PARAMS[EQ_LOW_GAIN].default, cx);
                        }
                        2 => {
                            this.set_sample_param(EQ_MID_FREQ, PARAMS[EQ_MID_FREQ].default, cx);
                            this.set_sample_param(EQ_MID_GAIN, PARAMS[EQ_MID_GAIN].default, cx);
                        }
                        3 => {
                            this.set_sample_param(EQ_HIGH_FREQ, PARAMS[EQ_HIGH_FREQ].default, cx);
                            this.set_sample_param(EQ_HIGH_GAIN, PARAMS[EQ_HIGH_GAIN].default, cx);
                        }
                        _ => {
                            this.set_sample_param(CUTOFF, PARAMS[CUTOFF].default, cx);
                            this.set_sample_param(RESONANCE, PARAMS[RESONANCE].default, cx);
                        }
                    },
                    CurveGesture::Commit => {}
                }
            }));

        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .w(px(390.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                pill_button(ds, "Filtro", params.filter, ds.accent)
                                    .id("sampler-filter")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.sampler.params.filter =
                                                !this.sampler.params.filter;
                                            this.prepare_sampler(cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .children(
                                [
                                    (FilterMode::LowPass, "LP"),
                                    (FilterMode::HighPass, "HP"),
                                    (FilterMode::BandPass, "BP"),
                                ]
                                .into_iter()
                                .enumerate()
                                .map(|(idx, (mode, label))| {
                                    pill_button(ds, label, params.filter_mode == mode, ds.cyan)
                                        .id(("sampler-filter-mode", idx))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.sampler.params.filter = true;
                                                this.sampler.params.filter_mode = mode;
                                                this.sampler_ui.eq_band = 0;
                                                this.prepare_sampler(cx);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .children(["Flt", "Low", "Mid", "High"].into_iter().enumerate().map(
                                |(i, label)| {
                                    let active = band as usize == i;
                                    let color = match i {
                                        1 => ds.warning,
                                        2 => ds.success,
                                        3 => ds.cyan,
                                        _ => ds.accent,
                                    };
                                    pill_button(ds, label, active, color)
                                        .id(("sampler-eq-band", i))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.sampler_ui.eq_band = i as u8;
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                },
                            ))
                            .when(has_eq, |row| {
                                row.child(
                                    pill_button(ds, "Reset EQ", false, ds.muted)
                                        .id("sampler-eq-reset")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                for id in [
                                                    EQ_LOW_FREQ,
                                                    EQ_LOW_GAIN,
                                                    EQ_MID_FREQ,
                                                    EQ_MID_GAIN,
                                                    EQ_HIGH_FREQ,
                                                    EQ_HIGH_GAIN,
                                                ] {
                                                    this.set_sample_param(
                                                        id,
                                                        PARAMS[id].default,
                                                        cx,
                                                    );
                                                }
                                                cx.stop_propagation();
                                            }),
                                        ),
                                )
                            }),
                    ),
            )
            .child(interactive_curve_box(
                "sampler-filter-curve".into(),
                ds,
                model,
                on_gesture,
            ))
    }
}
