//! Controles declarados por el componente seleccionado; comparten la acción FX.
use crate::ui::{
    DawApp,
    components::slider::{FractionSet, Reset, param_slider},
    fx::FxTarget,
};
use elevate::musical::fx::{FxEdit, FxKind};
use gpui::{Context, Div, div, prelude::*, rgb};
use std::rc::Rc;

impl DawApp {
    pub(super) fn selected_lua_component(&self) -> Option<FxTarget> {
        let target = self.fx.current?;
        self.lua_components
            .instances
            .contains_key(&target.node?)
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
        div()
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
                let min = control.min;
                let max = control.max;
                let default = control.default;
                let value = parameters.get(&parameter).copied().unwrap_or(default);
                let set: FractionSet = Rc::new(cx.listener(move |this, fraction: &f32, _, cx| {
                    if let Err(error) = this.edit_fx(
                        track,
                        FxEdit::Parameter(id, parameter, min + fraction * (max - min)),
                    ) {
                        this.status_message = error;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }));
                let reset: Reset = Rc::new(cx.listener(move |this, _, _, cx| {
                    if let Err(error) =
                        this.edit_fx(track, FxEdit::Parameter(id, parameter, default))
                    {
                        this.status_message = error;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }));
                param_slider(
                    &self.ds,
                    control.label.clone().into(),
                    format!("{value:.2}{}", control.unit),
                    ((value - min) / (max - min)).clamp(0.0, 1.0),
                    set,
                    reset,
                )
            }))
    }
}
