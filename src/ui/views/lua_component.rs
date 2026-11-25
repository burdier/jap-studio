//! Controles declarados por el componente seleccionado (Hay Filter / EQ y scripts Lua).
mod response;
use crate::ui::{
    DawApp,
    components::{
        button::flat_control,
        slider::{FractionSet, Reset, param_slider},
    },
    fx::FxTarget,
};
use elevate::musical::fx::{FxEdit, FxKind};
use gpui::{Context, Div, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn selected_lua_component(&self) -> Option<FxTarget> {
        let target = self.fx.current?;
        let selected_track = self.active_fx_track_id()?;
        (target.track == selected_track
            && self.lua_components.instances.contains_key(&target.node?)
            && self.fx_node(target).is_some())
        .then_some(target)
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
