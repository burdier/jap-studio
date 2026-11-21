//! Inspector plegable de la pieza seleccionada, compartido entre Pasos y el roll ligero.
use crate::ui::{
    DawApp,
    components::{
        button::flat_control,
        knob::{KnobSpec, rotary_knob},
    },
    pattern_editor::{DrumSampleTarget, PatternAction},
};
use elevate::{
    devices::sampler::{
        ATTACK, CUTOFF, DECAY, HOLD, PARAMS, PlayMode, RELEASE, RESONANCE, ROOT, SUSTAIN,
        SamplerParams, TUNE,
    },
    musical::{drums::DrumPiece, pitch::note_name},
};
use gpui::{Context, CursorStyle, Div, MouseButton, div, prelude::*, px, rgb};
use std::path::Path;

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
                        .max_h(px(72.0))
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
                                .h(px(20.0))
                                .px_1()
                                .flex_shrink_0()
                                .bg(rgb(ds.panel_dark))
                                .child(div().flex_1().min_w_0().truncate().child(name))
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
