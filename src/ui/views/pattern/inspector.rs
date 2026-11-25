//! Inspector plegable de la pieza seleccionada, compartido entre Pasos y el roll ligero.
use crate::ui::{
    DawApp,
    components::{
        button::flat_control,
        curve_overlay::{
            CurveAnchor, CurveGesture, CurveModel, CurvePath, GestureHandler, interactive_curve_box,
        },
        knob::{KnobSpec, rotary_knob},
    },
    pattern_editor::{DrumSampleTarget, PatternAction},
};
use elevate::{
    devices::{
        filter::FilterMode,
        sampler::{
            ATTACK, CUTOFF, DECAY, HOLD, PARAMS, PlayMode, RELEASE, RESONANCE, ROOT, SUSTAIN,
            SamplerParams, TUNE,
        },
    },
    musical::{
        drums::{DrumLayer, DrumPiece},
        model::InstrumentId,
        pitch::note_name,
    },
    primitives::filter::Filter,
};
use gpui::{
    Context, CursorStyle, Div, MouseButton, Stateful, component::tooltip::Tooltip, div, prelude::*,
    px, rgb,
};
use std::{path::Path, rc::Rc};

impl DawApp {
    pub(super) fn render_drum_inspector(&self, piece: &DrumPiece, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let id = piece.id;
        let choke = piece
            .choke_group
            .map_or_else(|| "—".into(), |group| group.to_string());
        let full_sample =
            piece.sampler.mode == PlayMode::OneShot && piece.sampler.get(SUSTAIN) == 100.0;
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .flex_shrink_0()
            .p_1()
            .gap_1()
            .text_xs()
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(ds.accent)).child(piece.name.clone()))
                    .child(self.drum_action_chip(
                        "Filtro",
                        piece.sampler.filter,
                        PatternAction::Filter(id),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_color(rgb(ds.muted))
                            .child(format!("Grupo de corte {choke}"))
                            .children([("−", -1), ("+", 1)].into_iter().map(|(label, delta)| {
                                self.drum_action_chip(label, false, PatternAction::Choke { piece: id, delta }, cx)
                            })),
                    )
                    .child(
                        flat_control(ds, false, ds.cyan)
                            .h(px(22.0))
                            .px_2()
                            .text_color(rgb(ds.cyan))
                            .child(if piece.layers.is_empty() { "Cargar muestra" } else { "+ Capa" })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                this.choose_drum_sample(DrumSampleTarget::AddLayer(id), cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .child(self.drum_action_chip(
                        "Cola completa",
                        full_sample,
                        PatternAction::FullSample(id),
                        cx,
                    )
                    .id(("drum-full-sample", id.0 as usize))
                    .tooltip(|window, cx| {
                        gpui::component::tooltip::Tooltip::new(
                            "One-shot con Sustain al 100 %. La duración del paso no corta el audio.",
                        ).build(window, cx)
                    }))
                    .child(self.drum_action_chip(
                        "Cortar al repetir",
                        piece.sampler.choke,
                        PatternAction::RetriggerChoke(id),
                        cx,
                    )
                    .id(("drum-retrigger-choke", id.0 as usize))
                    .tooltip(|window, cx| {
                        gpui::component::tooltip::Tooltip::new(
                            "El nuevo golpe corta la cola de esta pieza, aunque cambie la nota.",
                        ).build(window, cx)
                    })),
            )
            .child(self.render_drum_wave_filter(piece, cx))
            .child(
                div()
                    .id("drum-inspector-knobs")
                    .flex()
                    .gap_1()
                    .overflow_x_scroll()
                    .children(
                        [ATTACK, HOLD, DECAY, SUSTAIN, RELEASE, ROOT, TUNE, CUTOFF, RESONANCE]
                            .into_iter()
                            .map(|param| self.render_drum_knob(piece, param, cx)),
                    ),
            )
            .when(!piece.layers.is_empty(), |panel| {
                panel.child(
                    div()
                        .id("drum-sample-layers")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .max_h(px(94.0))
                        .overflow_y_scroll()
                        .children(piece.layers.iter().enumerate().map(|(layer, sample)| {
                            let name = Path::new(&sample.source)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or(&sample.source)
                                .to_owned();
                            let source = sample.source.clone();
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .h(px(28.0))
                                .px_1()
                                .flex_shrink_0()
                                .bg(rgb(ds.panel_dark))
                                .child(div().flex_1().min_w_0().truncate().child(name))
                                .child(self.render_drum_offset_knob(id, layer, sample.start_offset, cx))
                                .child(div().text_color(rgb(ds.muted)).child("Vel"))
                                .children(
                                    [(false, sample.velocity_min), (true, sample.velocity_max)].into_iter().map(
                                        |(upper, value)| {
                                            flat_control(ds, false, ds.cyan)
                                                .cursor(CursorStyle::ResizeUpDown)
                                                .px_1()
                                                .font_family("monospace")
                                                .text_color(rgb(ds.cyan))
                                                .child(value.to_string())
                                                .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                                                    let scroll = f32::from(event.delta.pixel_delta(px(16.0)).y);
                                                    if scroll != 0.0 {
                                                        this.apply_pattern_action(PatternAction::LayerVelocity {
                                                            piece: id,
                                                            layer,
                                                            upper,
                                                            delta: scroll.signum() as i16,
                                                        });
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    }
                                                }))
                                        },
                                    ),
                                )
                                .child(
                                    flat_control(ds, false, ds.cyan)
                                        .h(px(18.0))
                                        .px_2()
                                        .text_color(rgb(ds.cyan))
                                        .child("Cambiar")
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                            this.choose_drum_sample(
                                                DrumSampleTarget::ReplaceLayer(id, layer, source.clone()),
                                                cx,
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(self.drum_action_chip("×", false, PatternAction::RemoveSample { piece: id, layer }, cx))
                        })),
                )
            })
    }

    /// Onda de la muestra con la respuesta del filtro encima: arrastrar fija Hz y resonancia.
    fn render_drum_wave_filter(&self, piece: &DrumPiece, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let id = piece.id;
        let sampler = &piece.sampler;
        let cutoff = sampler.get(CUTOFF);
        let res_db = sampler.get(RESONANCE);
        let q = (0.707 * 10.0_f32.powf(res_db / 24.0)).clamp(0.5, 10.0);
        let mode_idx = match sampler.filter_mode {
            FilterMode::LowPass => 0.0,
            FilterMode::HighPass => 1.0,
            FilterMode::BandPass => 2.0,
        };
        let points = Filter::response(48_000.0, [cutoff, q, mode_idx])
            .into_iter()
            .map(|(x, db)| (x, ((db + 60.0) / 84.0).clamp(0.0, 1.0)))
            .collect();
        let on = sampler.filter;
        let wave = piece
            .layers
            .first()
            .map(|layer| {
                let offset = self.pattern_editor.layer_offset(id, 0, layer.start_offset);
                (
                    self.drum_wave(&layer.source, cx),
                    f32::from(offset) / f32::from(DrumLayer::MAX_START_OFFSET),
                )
            })
            .unwrap_or_default();
        let hz_label = if cutoff >= 1000.0 {
            format!("{:.2} kHz", cutoff / 1000.0)
        } else {
            format!("{cutoff:.0} Hz")
        };
        let mode_label = match sampler.filter_mode {
            FilterMode::LowPass => "LP",
            FilterMode::HighPass => "HP",
            FilterMode::BandPass => "BP",
        };
        let model = CurveModel {
            wave: wave.0.to_vec(),
            wave_dim_until: wave.1,
            spectrum: Vec::new(),
            grid_x: [100_f32, 1000., 10000.]
                .into_iter()
                .map(|hz| (hz / 20.0).ln() / 1000.0_f32.ln())
                .collect(),
            grid_y: [0.0_f32, -24., -48.]
                .into_iter()
                .map(|db| (db + 60.0) / 84.0)
                .collect(),
            baseline: Some(60.0 / 84.0),
            curves: vec![CurvePath {
                points,
                color: if on { ds.accent } else { ds.muted },
                stroke_px: 2.0,
                fill_alpha: if on { 0x24 } else { 0x0c },
            }],
            anchors: vec![CurveAnchor {
                x: sampler.fraction(CUTOFF),
                y: sampler.fraction(RESONANCE),
                stem: false,
                crosshair: true,
                color: if on { ds.cyan } else { ds.muted },
            }],
            header: "Onda · arrastra para filtrar (Hz / Res) · Shift: fino".into(),
            readout: format!("{mode_label} · {hz_label} · {res_db:.1} dB").into(),
            x_labels: ["20 Hz", "200", "2k", "20k"].map(Into::into).to_vec(),
            height_px: 120.0,
        };
        let on_gesture: GestureHandler =
            Rc::new(cx.listener(move |this, gesture: &CurveGesture, _, cx| {
                let Some(sampler) = this
                    .pattern_editor
                    .session
                    .kit
                    .pieces
                    .iter()
                    .find(|piece| piece.id == id)
                    .map(|piece| piece.sampler.clone())
                else {
                    return;
                };
                let set = |this: &mut Self, param, value| {
                    this.apply_pattern_action(PatternAction::SampleParam {
                        piece: id,
                        param,
                        value,
                    });
                };
                match *gesture {
                    CurveGesture::Move { x, y } => {
                        if !sampler.filter {
                            this.apply_pattern_action(PatternAction::Filter(id));
                        }
                        set(this, CUTOFF, SamplerParams::value_at(CUTOFF, x));
                        set(this, RESONANCE, SamplerParams::value_at(RESONANCE, y));
                    }
                    CurveGesture::Wheel { delta, fine } => {
                        let step = if fine { 0.01 } else { 0.04 };
                        let next = (sampler.fraction(RESONANCE) + delta * step).clamp(0.0, 1.0);
                        set(this, RESONANCE, SamplerParams::value_at(RESONANCE, next));
                    }
                    CurveGesture::Reset => {
                        set(this, CUTOFF, PARAMS[CUTOFF].default);
                        set(this, RESONANCE, PARAMS[RESONANCE].default);
                    }
                    CurveGesture::Commit => {}
                }
                cx.notify();
            }));
        div().px_1().child(interactive_curve_box(
            ("drum-wave-filter", id.0 as usize).into(),
            ds,
            model,
            on_gesture,
        ))
    }

    fn render_drum_offset_knob(
        &self,
        piece: InstrumentId,
        layer: usize,
        stored: u16,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let offset = self.pattern_editor.layer_offset(piece, layer, stored);
        div()
            .id(("drum-layer-offset", layer))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .px_1()
            .cursor(CursorStyle::ResizeUpDown)
            .tooltip(|window, cx| {
                Tooltip::new("Salta el inicio de esta muestra · arrastra o usa la rueda · Shift para ajustar fino · doble clic para volver a cero").build(window, cx)
            })
            .child(div().text_color(rgb(ds.muted)).child("Offset"))
            .child(rotary_knob(ds, KnobSpec {
                fraction: f32::from(offset) / f32::from(DrumLayer::MAX_START_OFFSET),
                bipolar: false,
                color: ds.accent,
                diameter: 20.0,
            }))
            .child(div().w(px(40.0)).font_family("monospace").text_color(rgb(ds.cyan))
                .child(format!("{:.1}%", f32::from(offset) / 10.0)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                if event.click_count == 2 {
                    this.apply_pattern_action(PatternAction::LayerOffset { piece, layer, value: 0 });
                } else {
                    this.begin_drum_offset_drag(piece, layer, offset, event.position.y);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(16.0)).y);
                if delta != 0.0 {
                    let step = if event.modifiers.shift { 1 } else { 10 };
                    let value = (offset as i16 + delta.signum() as i16 * step)
                        .clamp(0, DrumLayer::MAX_START_OFFSET as i16) as u16;
                    this.apply_pattern_action(PatternAction::LayerOffset { piece, layer, value });
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
    }

    fn render_drum_knob(&self, piece: &DrumPiece, param: usize, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let id = piece.id;
        let spec = PARAMS[param];
        let fraction = piece.sampler.fraction(param);
        let value = piece.sampler.get(param);
        let display = match param {
            ROOT => note_name(value as u8),
            CUTOFF => format!("{value:.0} Hz"),
            _ => format!("{value:.1} {}", spec.unit),
        };
        div()
            .w(px(63.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .cursor(CursorStyle::ResizeUpDown)
            .child(div().text_color(rgb(ds.muted)).child(spec.name))
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction,
                    bipolar: param == TUNE,
                    color: ds.accent,
                    diameter: 24.0,
                },
            ))
            .child(
                div()
                    .font_family("monospace")
                    .text_color(rgb(ds.cyan))
                    .child(display),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let scroll = f32::from(event.delta.pixel_delta(px(16.0)).y);
                if scroll == 0.0 {
                    return;
                }
                let direction = scroll.signum();
                let fine = event.modifiers.shift;
                let adjusted = match param {
                    ROOT => value + direction,
                    ATTACK | HOLD => value + direction * if fine { 0.5 } else { 5.0 },
                    DECAY | RELEASE => value + direction * if fine { 5.0 } else { 50.0 },
                    _ => {
                        let scale = if fine { 0.004 } else { 0.025 };
                        SamplerParams::value_at(param, fraction + direction * scale)
                    }
                };
                this.apply_pattern_action(PatternAction::SampleParam {
                    piece: id,
                    param,
                    value: adjusted,
                });
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count == 2 && param != ROOT {
                        let value = spec.default;
                        this.apply_pattern_action(PatternAction::SampleParam {
                            piece: id,
                            param,
                            value,
                        });
                        cx.notify();
                    }
                }),
            )
    }
}
