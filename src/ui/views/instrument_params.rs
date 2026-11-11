//! Bloques y perillas del piano; MIDI Learn conserva los índices originales de SPECS.
use crate::instruments::piano_params::{ParamId, SPECS, spec_of};
use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        knob::{KnobSpec, rotary_knob},
    },
};
use gpui::{Context, IntoElement, MouseButton, Pixels, div, prelude::*, px, rgb};

#[derive(Default)]
pub(in crate::ui) struct PianoUi {
    pub drag: Option<(ParamId, Pixels, f32)>,
    pub banks_open: bool,
    pub credits_open: bool,
    pub spectrum: Option<crate::audio::analyzer::SpectrumAnalyzer>,
    pub spectrum_rate: u32,
    pub spectrum_mode: crate::ui::components::spectrum::SpectrumMode,
}

impl DawApp {
    pub(in crate::ui) fn render_instrument_params(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use ParamId::*;
        let groups: [(&str, &[ParamId]); 4] = [
            (
                "AFINACIÓN / VOCES",
                &[
                    CoarseTune, FineTune, Polyphony, PitchBend, BendRange, Sustain,
                ],
            ),
            (
                "FILTRO ESTÉREO",
                &[FilterEnabled, FilterMode, Cutoff, Resonance],
            ),
            (
                "EFECTOS / MODULACIÓN",
                &[Effects, ReverbSend, ChorusSend, Modulation],
            ),
            ("SALIDA", &[MasterVolume, Volume, Expression, Pan]),
        ];
        div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .bg(rgb(self.ds.panel))
            .children(groups.map(|(title, ids)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .border_t_1()
                    .border_r_1()
                    .border_color(rgb(self.ds.border))
                    .child(div().text_color(rgb(self.ds.muted)).child(title))
                    .child(
                        div().flex().flex_wrap().items_center().gap_2().children(
                            ids.iter()
                                .map(|&id| self.piano_knob(id, cx).into_any_element())
                                .collect::<Vec<_>>(),
                        ),
                    )
                    .when(title == "FILTRO ESTÉREO", |group| {
                        let analyzer = self.piano_ui.spectrum.as_ref();
                        let mut data = crate::ui::spectrum::data(
                            analyzer,
                            self.piano_ui.spectrum_mode,
                            &self.ds,
                        );
                        data.guide_hz = Some(self.instrument.param(Cutoff));
                        data.width = Some(206.);
                        data.height = 56.;
                        group.child(crate::ui::components::spectrum::spectrum_panel(
                            data,
                            self.piano_ui.spectrum_mode,
                            cx.listener(|this, _, _, cx| {
                                this.piano_ui.spectrum_mode =
                                    this.piano_ui.spectrum_mode.previous();
                                cx.stop_propagation();
                                cx.notify();
                            }),
                            cx.listener(|this, _, _, cx| {
                                this.piano_ui.spectrum_mode = this.piano_ui.spectrum_mode.next();
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ))
                    })
            }))
    }

    fn learn_piano_param(&mut self, id: ParamId) -> bool {
        if !self.midi_mapper.learn_mode() {
            return false;
        }
        let index = SPECS
            .iter()
            .position(|spec| spec.id == id)
            .expect("parámetro registrado");
        self.arm_midi_learn(elevate::musical::midi_map::MidiBindTarget::InstrumentParam(
            index as u8,
        ));
        true
    }

    fn piano_knob(&self, id: ParamId, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let spec = spec_of(id);
        let value = self.instrument.param(id);
        if spec.is_toggle() || id == ParamId::FilterMode {
            return div()
                .w(px(87.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(ds.muted)).child(spec.label))
                .child(
                    pill_button(
                        ds,
                        spec.display(value),
                        value >= 0.5 || id == ParamId::FilterMode,
                        ds.accent,
                    )
                    .id(("piano-switch", id as usize))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            if !this.learn_piano_param(id) {
                                let value = if event.click_count == 2 {
                                    spec.default
                                } else if id == ParamId::FilterMode {
                                    (this.instrument.param(id) + 1.) % 3.
                                } else {
                                    1. - this.instrument.param(id)
                                };
                                this.set_piano_param(id, value);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element();
        }
        div()
            .id(("piano-param", id as usize))
            .w(px(87.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .cursor(gpui::CursorStyle::ResizeUpDown)
            .child(div().text_color(rgb(ds.muted)).child(spec.label))
            .child(rotary_knob(
                ds,
                KnobSpec {
                    fraction: spec.fraction(value),
                    bipolar: spec.min < 0. || id == ParamId::Pan,
                    color: ds.accent,
                    diameter: 26.,
                },
            ))
            .child(div().text_color(rgb(ds.cyan)).child(spec.display(value)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if !this.learn_piano_param(id) {
                        if event.click_count == 2 {
                            this.set_piano_param(id, spec.default);
                        } else {
                            this.piano_ui.drag = Some((
                                id,
                                event.position.y,
                                spec.fraction(this.instrument.param(id)),
                            ));
                        }
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
                if delta != 0. && !this.learn_piano_param(id) {
                    let value = this.instrument.param(id);
                    let next = if id == ParamId::Cutoff {
                        spec.from_fraction(
                            spec.fraction(value)
                                + delta.signum() / if event.modifiers.shift { 1000. } else { 100. },
                        )
                    } else {
                        let step = if event.modifiers.shift {
                            spec.step
                        } else {
                            spec.step.max((spec.max - spec.min) / 100.)
                        };
                        value + delta.signum() * step
                    };
                    this.set_piano_param(id, next);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .into_any_element()
    }
}
