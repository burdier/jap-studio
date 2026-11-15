//! Controles declarados por el componente seleccionado; comparten la acción FX.
mod response;
use crate::ui::{
    DawApp,
    components::slider::{FractionSet, Reset, param_slider},
    fx::FxTarget,
};
use elevate::musical::fx::{FxEdit, FxKind};
use gpui::{Context, Div, MouseButton, div, prelude::*, rgb};
use std::rc::Rc;

impl DawApp {
    pub(super) fn selected_lua_component(&self) -> Option<FxTarget> {
        let target = self.fx.current?;
        let selected_track = self.tracks.get(self.cursor_row)?.id;
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
        let panel = div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(self.ds.panel_dark))
            .border_t_1()
            .border_color(rgb(self.ds.frame_border()))
            .child(
                div()
                    .text_color(rgb(self.ds.text))
                    .child(effect.descriptor().name.clone()),
            )
            .children(effect.descriptor().controls.iter().map(|control| {
                let track = target.track;
                let parameter = control.id;
                let spec = control.clone();
                let default = control.default;
                let value = parameters.get(&parameter).copied().unwrap_or(default);
                let set: FractionSet = Rc::new(cx.listener(move |this, fraction: &f32, _, cx| {
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
                if !control.choices.is_empty() {
                    return div()
                        .flex()
                        .gap_2()
                        .child(control.label.clone())
                        .children(control.choices.iter().enumerate().map(|(i, label)| {
                            div()
                                .px_2()
                                .py_1()
                                .border_1()
                                .border_color(rgb(self.ds.frame_border()))
                                .bg(rgb(if value == i as f32 {
                                    self.ds.accent
                                } else {
                                    self.ds.panel_dark
                                }))
                                .text_color(rgb(self.ds.text))
                                .cursor_pointer()
                                .child(label.clone())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        if let Err(e) = this.edit_fx(
                                            track,
                                            FxEdit::Parameter(id, parameter, i as f32),
                                        ) {
                                            this.status_message = e;
                                        }
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                        }))
                        .into_any_element();
                }
                param_slider(
                    &self.ds,
                    control.label.clone().into(),
                    format!("{value:.2}{}", control.unit),
                    control.fraction(value),
                    set,
                    reset,
                )
                .into_any_element()
            }));
        if let Some(node) = effect.descriptor().response_node {
            let rate = effect.sample_rate() as f64;
            let track = target.track;
            let on_edit: response::ParameterEditFn = Rc::new(cx.listener(
                move |this, &(parameter, value): &(u32, f32), _, cx| {
                    if let Err(error) = this.edit_fx(track, FxEdit::Parameter(id, parameter, value))
                    {
                        this.status_message = error;
                    }
                    cx.notify();
                },
            ));
            panel.child(response::plot(
                &self.ds,
                effect.descriptor(),
                parameters,
                node,
                rate,
                on_edit,
            ))
        } else {
            panel
        }
    }
}
