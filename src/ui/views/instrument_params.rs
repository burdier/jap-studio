//! Panel de parámetros del instrumento. Dibuja `piano_params::SPECS` con los componentes
//! genéricos; no conoce RustySynth: cada cambio pasa por `DawApp::set_piano_param`.
use super::super::{
    DawApp,
    components::slider::{FractionSet, Reset, param_slider, param_toggle},
};
use crate::instruments::piano_params::SPECS;
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_instrument_params(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let items = SPECS.iter().map(|spec| {
            let id = spec.id;
            let value = self.instrument.param(id);
            let on_set: FractionSet = Rc::new(cx.listener(move |this, fraction: &f32, _, cx| {
                let spec = crate::instruments::piano_params::spec_of(id);
                this.set_piano_param(id, spec.from_fraction(*fraction));
                cx.notify();
            }));
            let on_reset: Reset = Rc::new(cx.listener(move |this, _: &(), _, cx| {
                this.set_piano_param(id, crate::instruments::piano_params::spec_of(id).default);
                cx.notify();
            }));
            if spec.is_toggle() {
                param_toggle(ds, spec.label, value >= 0.5, on_set, on_reset).into_any_element()
            } else {
                param_slider(
                    ds,
                    spec.label,
                    spec.display(value),
                    spec.fraction(value),
                    on_set,
                    on_reset,
                )
                .into_any_element()
            }
        });
        div()
            .id("instrument-params")
            .flex_shrink_0()
            .max_h(px(170.0))
            .overflow_y_scroll()
            .px(px(ds.padding_lg))
            .py_2()
            .flex()
            .flex_wrap()
            .gap_x_4()
            .gap_y_2()
            .border_t_1()
            .border_color(rgb(ds.border))
            .children(items.collect::<Vec<AnyElement>>())
    }
}
