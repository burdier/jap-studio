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
            CUTOFF, PAN, PARAMS, PITCH_ENV, RESONANCE, ROOT, SamplerParams, TUNE, VEL_FILTER,
            VOICES,
        },
    },
    primitives::filter::Filter,
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
        } else {
            format!("{value:.1} {}", spec.unit)
        };
        div()
            .id(("sampler-param", id))
            .w(px(87.))
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
                    bipolar: matches!(id, TUNE | PAN | PITCH_ENV | VEL_FILTER),
                    color: ds.accent,
                    diameter: 26.,
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

    /// Bloque Hay Filter reutilizando `CurveOverlay`; el sampler define sus ejes (Hz log + dB).
    pub(super) fn sample_filter_card(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let params = &self.sampler.params;
        let cutoff = params.get(CUTOFF);
        let res_db = params.get(RESONANCE);
        let q = (0.707 * 10.0_f32.powf(res_db / 24.0)).clamp(0.5, 10.0);
        let mode_idx = match params.filter_mode {
            FilterMode::LowPass => 0.0,
            FilterMode::HighPass => 1.0,
            FilterMode::BandPass => 2.0,
        };
        let points: Vec<(f32, f32)> = Filter::response(48_000.0, [cutoff, q, mode_idx])
            .into_iter()
            .map(|(x, db)| (x, ((db + 60.0) / 84.0).clamp(0.0, 1.0)))
            .collect();
        let nx = params.fraction(CUTOFF);
        let ny = params.fraction(RESONANCE);
        let mode_label = match params.filter_mode {
            FilterMode::LowPass => "LP 12",
            FilterMode::HighPass => "HP 12",
            FilterMode::BandPass => "BP 12",
        };
        let hz_label = if cutoff >= 1000.0 {
            format!("{:.2} kHz", cutoff / 1000.0)
        } else {
            format!("{cutoff:.0} Hz")
        };
        let grid_x = [100_f32, 1000., 10000.]
            .into_iter()
            .map(|hz| (hz / 20.0).ln() / 1000.0_f32.ln())
            .collect();
        let grid_y = [0.0_f32, -24., -48.]
            .into_iter()
            .map(|db| (db + 60.0) / 84.0)
            .collect();
        let active_color = if params.filter { ds.accent } else { ds.muted };
        let model = CurveModel {
            grid_x,
            grid_y,
            baseline: Some(60.0 / 84.0),
            curves: vec![CurvePath {
                points,
                color: active_color,
                stroke_px: 2.0,
                fill_alpha: if params.filter { 0x24 } else { 0x0c },
            }],
            anchors: vec![CurveAnchor {
                x: nx,
                y: ny,
                stem: false,
                crosshair: true,
                color: if params.filter { ds.cyan } else { ds.muted },
            }],
            header: "Hay Filter · arrastra Hz/Res · Shift: fino".into(),
            readout: format!("{mode_label} · {hz_label} · {res_db:.1} dB").into(),
            x_labels: ["20 Hz", "200", "2k", "20k"].map(Into::into).to_vec(),
            height_px: 108.0,
        };
        let on_gesture: GestureHandler = Rc::new(cx.listener(
            move |this, gesture: &CurveGesture, _, cx| match *gesture {
                CurveGesture::Move { x, y, .. } => {
                    if !this.sampler.params.filter {
                        this.sampler.params.filter = true;
                        this.prepare_sampler(cx);
                    }
                    this.set_sample_param(CUTOFF, SamplerParams::value_at(CUTOFF, x), cx);
                    this.set_sample_param(RESONANCE, SamplerParams::value_at(RESONANCE, y), cx);
                }
                CurveGesture::Wheel { delta, fine, .. } => {
                    let step = if fine { 0.01 } else { 0.04 };
                    let next =
                        (this.sampler.params.fraction(RESONANCE) + delta * step).clamp(0.0, 1.0);
                    this.set_sample_param(RESONANCE, SamplerParams::value_at(RESONANCE, next), cx);
                }
                CurveGesture::Reset => {
                    this.set_sample_param(CUTOFF, PARAMS[CUTOFF].default, cx);
                    this.set_sample_param(RESONANCE, PARAMS[RESONANCE].default, cx);
                }
                CurveGesture::Commit => {}
            },
        ));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .w(px(330.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        pill_button(ds, "Filtro", params.filter, ds.accent)
                            .id("sampler-filter")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.sampler.params.filter = !this.sampler.params.filter;
                                    this.prepare_sampler(cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div().flex().gap_1().children(
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
                                            this.prepare_sampler(cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                            }),
                        ),
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
