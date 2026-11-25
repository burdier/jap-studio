//! Controles declarados por el componente seleccionado (Hay Filter / EQ y scripts Lua).
mod response;
use crate::ui::{
    DawApp, DesignSystem,
    components::{
        button::flat_control,
        curve_overlay::{
            CurveAnchor, CurveGesture, CurveModel, CurvePath, GestureHandler, interactive_curve_box,
        },
        slider::{FractionSet, Reset, param_slider},
    },
    fx::FxTarget,
};
use elevate::musical::{
    fx::{FxEdit, FxId, FxKind, SidechainSource},
    track::{MASTER_TRACK_ID, TrackDevice, TrackId},
};
use gpui::{Context, Div, MouseButton, div, prelude::*, px, relative, rgb};
use std::rc::Rc;

const COMP_ID: &str = "hay.fx.compressor";
const LIMITER_ID: &str = "hay.fx.limiter";

impl DawApp {
    pub(in crate::ui) fn selected_lua_component(&self) -> Option<FxTarget> {
        let target = self.fx.current?;
        let selected_track = self.active_fx_track_id()?;
        (target.track == selected_track
            && self.lua_components.instances.contains_key(&target.node?)
            && self.fx_node(target).is_some())
        .then_some(target)
    }

    pub(in crate::ui) fn selected_vst3_fx(&self) -> Option<FxTarget> {
        let target = self.fx.current?;
        let selected_track = self.active_fx_track_id()?;
        if target.track != selected_track {
            return None;
        }
        let node = self.fx_node(target)?;
        matches!(
            node.kind,
            FxKind::Effect {
                plugin_id: TrackDevice::Vst3 { .. },
                ..
            }
        )
        .then_some(target)
    }

    pub(super) fn render_vst3_fx_controls(&self, target: FxTarget, cx: &mut Context<Self>) -> Div {
        let id = target.node.unwrap();
        let node = self.fx_node(target).unwrap();
        let ds = &self.ds;
        div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(ds.panel_dark))
            .border_t_1()
            .border_color(rgb(ds.frame_border()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.text))
                            .child(node.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("VST3 · Bus Aux #2"),
                    ),
            )
            .child(self.render_sidechain_bar(
                target.track,
                id,
                node.sidechain,
                "SIDECHAIN · AUDIO FROM (VST3)",
                cx,
            ))
    }

    pub(super) fn render_sidechain_bar(
        &self,
        track: TrackId,
        fx_id: FxId,
        current: Option<SidechainSource>,
        title: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let active_amber = 0xf59e0b_u32;
        let pill = |label: String, source: Option<SidechainSource>, cx: &mut Context<Self>| {
            let selected = current == source;
            let accent = if source.is_some() {
                active_amber
            } else {
                ds.accent
            };
            flat_control(ds, selected, accent)
                .px(px(7.))
                .py(px(3.))
                .text_xs()
                .text_color(rgb(if selected { accent } else { ds.text }))
                .child(label)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if let Err(e) = this.edit_fx(track, FxEdit::Sidechain(fx_id, source)) {
                            this.status_message = e;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
        };

        let mut pills = vec![
            pill("Off".into(), None, cx),
            pill("Drums".into(), Some(SidechainSource::Drums), cx),
        ];
        for candidate in &self.tracks {
            if candidate.id == track || track == MASTER_TRACK_ID {
                continue;
            }
            pills.push(pill(
                candidate.name.to_string(),
                Some(SidechainSource::Track(candidate.id)),
                cx,
            ));
        }

        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded(px(ds.radius_sm))
            .bg(rgb(ds.panel))
            .border_1()
            .border_color(rgb(if current.is_some() {
                active_amber
            } else {
                ds.frame_border()
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(if current.is_some() {
                                active_amber
                            } else {
                                ds.muted
                            }))
                            .child(title),
                    )
                    .when(current.is_some(), |row| {
                        row.child(
                            div()
                                .text_xs()
                                .text_color(rgb(active_amber))
                                .child("ACTIVO"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .children(pills),
            )
    }

    pub(super) fn render_lua_component_controls(
        &self,
        target: FxTarget,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = target.node.unwrap();
        let effect = &self.lua_components.instances[&id];
        let node = self.fx_node(target).unwrap();
        let FxKind::Effect { parameters, .. } = &node.kind else {
            unreachable!()
        };
        let location = if target.track == elevate::musical::track::MASTER_TRACK_ID {
            Some("Bus Master".to_string())
        } else {
            self.tracks.get(self.cursor_row).map(|track| {
                let bands = track.chain.bands();
                let band = bands.iter().position(|band| {
                    track
                        .chain
                        .find(band.chain)
                        .is_some_and(|branch| branch.find(id).is_some())
                });
                match band {
                    Some(i) => format!(
                        "Pista {} · {}",
                        track.name,
                        super::tejer::band_name(i, bands.len())
                    ),
                    None => format!("Pista {}", track.name),
                }
            })
        };

        let ds = &self.ds;
        let track = target.track;
        let desc = effect.descriptor();
        let mut choice_rows = Vec::new();
        let mut slider_items = Vec::new();

        for control in &desc.controls {
            let parameter = control.id;
            let spec = control.clone();
            let default = control.default;
            let value = parameters.get(&parameter).copied().unwrap_or(default);
            if !control.choices.is_empty() {
                choice_rows.push(div().flex().flex_wrap().items_center().gap_1().children(
                    control.choices.iter().enumerate().map(|(i, label)| {
                        let active = value == i as f32;
                        let accent = if i >= 4 { ds.cyan } else { ds.accent };
                        flat_control(ds, active, accent)
                            .px(px(7.))
                            .py(px(3.))
                            .text_xs()
                            .text_color(rgb(if active { accent } else { ds.text }))
                            .child(label.clone())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    if let Err(e) = this
                                        .edit_fx(track, FxEdit::Parameter(id, parameter, i as f32))
                                    {
                                        this.status_message = e;
                                    }
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                    }),
                ));
            } else {
                let set: FractionSet = Rc::new(cx.listener(move |this, fraction: &f32, _, cx| {
                    if this.midi_mapper.learn_mode() {
                        this.arm_midi_learn(elevate::musical::midi_map::MidiBindTarget::FxParam {
                            track,
                            fx: id,
                            param: parameter,
                        });
                        cx.notify();
                        return;
                    }
                    if let Err(error) = this.edit_fx(
                        track,
                        FxEdit::Parameter(id, parameter, spec.from_fraction(*fraction)),
                    ) {
                        this.status_message = error;
                    }
                    cx.notify();
                }));
                let reset: Reset = Rc::new(cx.listener(move |this, _, _, cx| {
                    if let Err(error) =
                        this.edit_fx(track, FxEdit::Parameter(id, parameter, default))
                    {
                        this.status_message = error;
                    }
                    cx.notify();
                }));
                let readout = if control.unit == "Hz" {
                    if value >= 1000.0 {
                        format!("{:.2} kHz", value / 1000.0)
                    } else {
                        format!("{value:.0} Hz")
                    }
                } else if control.unit == "dB" {
                    format!("{value:+.1} dB")
                } else if control.unit == "ms" {
                    if value < 1.0 {
                        format!("{value:.2} ms")
                    } else if value < 10.0 {
                        format!("{value:.1} ms")
                    } else {
                        format!("{value:.0} ms")
                    }
                } else if control.unit == ":1" {
                    format!("{value:.1}:1")
                } else if control.unit == "%" {
                    format!("{value:.0}%")
                } else {
                    format!("{value:.2} {}", control.unit)
                };
                slider_items.push(param_slider(
                    ds,
                    control.label.clone().into(),
                    readout,
                    control.fraction(value),
                    set,
                    reset,
                ));
            }
        }

        let mut panel = div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(ds.panel_dark))
            .border_t_1()
            .border_color(rgb(ds.frame_border()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.text))
                            .child(desc.name.clone()),
                    )
                    .when_some(location, |header, location| {
                        header.child(div().text_xs().text_color(rgb(ds.muted)).child(location))
                    }),
            )
            .children(choice_rows);

        let is_comp = desc.id == COMP_ID;
        let is_limiter = desc.id == LIMITER_ID;
        if is_comp || is_limiter {
            panel = panel.child(render_gr_meter(ds, effect.gain_reduction_db()));
        }
        if is_comp {
            let threshold = parameters.get(&0).copied().unwrap_or(-18.0);
            let ratio = parameters.get(&1).copied().unwrap_or(4.0);
            let knee = parameters.get(&4).copied().unwrap_or(6.0);
            panel = panel.child(render_compressor_curve(
                ds, track, id, threshold, ratio, knee, cx,
            ));
            if track != MASTER_TRACK_ID {
                panel = panel.child(self.render_sidechain_bar(
                    track,
                    id,
                    node.sidechain,
                    "SIDECHAIN · AUDIO FROM",
                    cx,
                ));
            }
        }

        // Curva interactiva protagonista arriba (con espectro Pre/Post en vivo)
        if let Some(resp_node) = desc.response_node {
            let rate = effect.sample_rate() as f64;
            let on_edit: response::ParameterEditFn = Rc::new(cx.listener(
                move |this, &(parameter, value): &(u32, f32), _, cx| {
                    if let Err(error) = this.edit_fx(track, FxEdit::Parameter(id, parameter, value))
                    {
                        this.status_message = error;
                    }
                    cx.notify();
                },
            ));
            panel = panel
                .child(response::plot(
                    ds,
                    desc,
                    parameters,
                    resp_node,
                    rate,
                    self.fx.analyzers.comparison(id, id, ds),
                    on_edit,
                ))
                .child(crate::ui::components::curve_overlay::spectrum_legend(
                    ds,
                    "Entrada",
                    "Salida".into(),
                    "Curva",
                ));
        }

        if !slider_items.is_empty() {
            panel = panel.child(div().flex().flex_wrap().gap_2().children(slider_items));
        }

        panel
    }
}

/// Medidor de Gain Reduction (GR) estilo Ableton: barra ámbar que desciende de derecha a izquierda.
fn render_gr_meter(ds: &DesignSystem, gr_db: f32) -> Div {
    let gr_mag = gr_db.abs().clamp(0.0, 30.0);
    let frac = (gr_mag / 24.0).clamp(0.0, 1.0);
    let amber = 0xf59e0b_u32;
    let active = gr_mag > 0.1;
    let label = if active {
        format!("-{gr_mag:.1} dB")
    } else {
        "0.0 dB".to_owned()
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child(div().text_color(rgb(ds.muted)).child("GR (Reducción)"))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(if active { amber } else { ds.muted }))
                        .child(label),
                ),
        )
        .child(
            div()
                .h(px(8.))
                .w_full()
                .rounded(px(3.))
                .bg(rgb(ds.panel))
                .border_1()
                .border_color(rgb(ds.frame_border()))
                .flex()
                .justify_end()
                .overflow_hidden()
                .child(div().h_full().w(relative(frac)).bg(rgb(amber))),
        )
}

/// Curva de transferencia interactiva (Entrada vs Salida dB) estilo Ableton Live Compressor.
fn render_compressor_curve(
    ds: &DesignSystem,
    track: TrackId,
    fx_id: FxId,
    threshold_db: f32,
    ratio: f32,
    knee_db: f32,
    cx: &mut Context<DawApp>,
) -> impl IntoElement {
    let db_to_norm = |db: f32| ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let transfer_out = |in_db: f32| -> f32 {
        let half_knee = 0.5 * knee_db;
        let over = in_db - threshold_db;
        let excess = if knee_db <= 0.01 {
            over.max(0.0)
        } else if over <= -half_knee {
            0.0
        } else if over >= half_knee {
            over
        } else {
            let x = over + half_knee;
            (x * x) / (2.0 * knee_db)
        };
        let gr = excess * (1.0 - 1.0 / ratio.max(1.0));
        (in_db - gr).clamp(-60.0, 0.0)
    };

    let curve_pts: Vec<(f32, f32)> = (0..=60)
        .map(|i| {
            let in_db = -60.0 + i as f32;
            (db_to_norm(in_db), db_to_norm(transfer_out(in_db)))
        })
        .collect();
    let thresh_norm = db_to_norm(threshold_db);

    let model = CurveModel {
        grid_x: vec![0.25, 0.5, 0.75],
        grid_y: vec![0.25, 0.5, 0.75],
        curves: vec![
            CurvePath {
                points: vec![(0.0, 0.0), (1.0, 1.0)],
                color: ds.muted,
                stroke_px: 1.0,
                fill_alpha: 0,
            },
            CurvePath {
                points: curve_pts,
                color: 0xf59e0b,
                stroke_px: 2.0,
                fill_alpha: 24,
            },
        ],
        anchors: vec![CurveAnchor {
            x: thresh_norm,
            y: thresh_norm,
            stem: false,
            crosshair: true,
            color: ds.cyan,
        }],
        header: "TRANSFERENCIA · Arrastra Umbral/Ratio · Rueda Knee".into(),
        readout: format!("{threshold_db:.1} dB · {ratio:.1}:1 · Knee {knee_db:.1} dB").into(),
        x_labels: vec![
            "-60".into(),
            "-45".into(),
            "-30".into(),
            "-15".into(),
            "0 dB".into(),
        ],
        height_px: 120.0,
        ..Default::default()
    };

    let on_gesture: GestureHandler =
        Rc::new(cx.listener(move |this, gesture: &CurveGesture, _, cx| {
            match *gesture {
                CurveGesture::Move { x, y } => {
                    let next_thresh = (-60.0 + x.clamp(0.0, 1.0) * 60.0).clamp(-60.0, 0.0);
                    // Arrastrar verticalmente hacia abajo respecto a la diagonal sube el ratio.
                    let pull = (x - y).clamp(0.0, 1.0);
                    let next_ratio = (1.0 + pull * 19.0).clamp(1.0, 20.0);
                    let _ = this.edit_fx(track, FxEdit::Parameter(fx_id, 0, next_thresh));
                    let _ = this.edit_fx(track, FxEdit::Parameter(fx_id, 1, next_ratio));
                }
                CurveGesture::Wheel { delta, fine } => {
                    let step = if fine { 0.5 } else { 1.5 };
                    let next_knee = (knee_db + delta.signum() * step).clamp(0.0, 18.0);
                    let _ = this.edit_fx(track, FxEdit::Parameter(fx_id, 4, next_knee));
                }
                CurveGesture::Reset => {
                    let _ = this.edit_fx(track, FxEdit::Parameter(fx_id, 0, -18.0));
                    let _ = this.edit_fx(track, FxEdit::Parameter(fx_id, 1, 4.0));
                    let _ = this.edit_fx(track, FxEdit::Parameter(fx_id, 4, 6.0));
                }
                CurveGesture::Commit => {}
            }
            cx.notify();
        }));

    interactive_curve_box(
        gpui::ElementId::Name(format!("comp-curve-{}", fx_id.0).into()),
        ds,
        model,
        on_gesture,
    )
}
