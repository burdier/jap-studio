//! Panel de un dispositivo de salida; todos los controles llaman al controlador común.
use crate::ui::{
    DawApp,
    components::{
        button::pill_button,
        slider::{FractionSet, Reset, param_slider},
        spectrum::{SpectrumMode, spectrum_graph},
    },
};
use elevate::devices::{utility::DESCRIPTOR, utility_slot::UtilityAction};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(super) fn master_lufs(&self) -> [String; 2] {
        self.player
            .as_ref()
            .map_or([None, None], |player| player.master_loudness())
            .map(|value| value.map_or("—".into(), |lufs| format!("{lufs:.1}")))
    }

    pub(in crate::ui) fn render_utility(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let state = self.utility.state();
        let spec = DESCRIPTOR.parameters[0];
        let set: FractionSet = Rc::new(cx.listener(move |this, fraction: &f32, _, cx| {
            this.apply_utility_action(UtilityAction::SetGain(
                spec.min + fraction * (spec.max - spec.min),
            ));
            cx.stop_propagation();
            cx.notify();
        }));
        let [momentary, short_term] = self.master_lufs();
        let mut spectrum = crate::ui::spectrum::data(
            self.player.as_ref().map(|player| player.master_spectrum()),
            SpectrumMode::Spectrum,
            ds,
        );
        spectrum.height = 48.0;
        let reset: Reset = Rc::new(cx.listener(move |this, _, _, cx| {
            this.apply_utility_action(UtilityAction::SetGain(spec.default));
            cx.stop_propagation();
            cx.notify();
        }));
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_4()
            .p_3()
            .flex_shrink_0()
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .rounded(px(ds.radius_md))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_color(rgb(ds.accent)).child(DESCRIPTOR.name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("Salida máster"),
                    ),
            )
            .child(param_slider(
                ds,
                "Ganancia · clic para 1x".into(),
                format!("{:.2}x", state.gain),
                (state.gain - spec.min) / (spec.max - spec.min),
                set,
                reset,
            ))
            .child(
                pill_button(
                    ds,
                    if state.bypassed {
                        "Bypass ON"
                    } else {
                        "Bypass OFF"
                    },
                    state.bypassed,
                    ds.warning,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.apply_utility_action(UtilityAction::ToggleBypass);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .child(pill_button(ds, "Reset", false, ds.text).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.apply_utility_action(UtilityAction::Reset);
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
            .child(
                pill_button(
                    ds,
                    if state.inserted {
                        "Retirar"
                    } else {
                        "Insertar"
                    },
                    false,
                    ds.text,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.apply_utility_action(if this.utility.state().inserted {
                            UtilityAction::Remove
                        } else {
                            UtilityAction::Insert
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .child(
                div()
                    .w(px(240.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.cyan))
                            .child(format!("Máster · M {momentary} / S {short_term} LUFS")),
                    )
                    .child(spectrum_graph(spectrum))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("20 Hz – 20 kHz · M 400 ms · S 3 s"),
                    ),
            )
            .child(pill_button(ds, "Cerrar", false, ds.muted).on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.utility_open = false;
                    cx.stop_propagation();
                    cx.notify();
                }),
            ))
    }
}
