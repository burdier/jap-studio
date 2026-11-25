//! Formulario flotante: controles GPUI Kit y presentación del tema Hay.
use crate::ui::{
    DawApp,
    components::{
        modal::{ModalClose, modal_backdrop},
        parameter_field::ParameterField,
    },
    kit::{CancelActionForm, ConfirmActionForm},
};
use elevate::actions::ParameterKind;
use gpui::{
    AnyElement, Context, Div, IntoElement, Pixels, Size,
    base::FocusTrapElement,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Escape, Input, NumberInput},
    },
    div,
    prelude::*,
    px, rgb,
};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_action_form(
        &self,
        viewport: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let form = self.action_form.as_ref().expect("formulario abierto");
        let ds = &self.ds;
        let export = form.export.as_ref();
        let running = export.is_some_and(|export| export.running);
        let completed = export.is_some_and(|export| export.completed.is_some());
        let close: ModalClose = Rc::new(cx.listener(|this, _, window, cx| {
            this.close_action_form(window, cx);
            cx.stop_propagation();
        }));
        let field_content = if export.is_some() {
            self.render_export_fields(cx)
        } else {
            div().children((0..form.fields.len()).map(|index| self.render_action_field(index, cx)))
        };
        let fields = div()
            .id("action-parameter-fields")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&form.scroll)
            .px_4()
            .py_2()
            .child(field_content);
        let footer = div()
            .flex_shrink_0()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(rgb(ds.border))
            .flex()
            .flex_col()
            .gap_2()
            .when_some(form.error.clone(), |footer, error| {
                footer.child(div().text_sm().text_color(rgb(ds.pink)).child(error))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .when(!running && !completed, |row| {
                        row.child(div().text_xs().text_color(rgb(ds.muted)).child(
                            if export.is_some() {
                                "Ctrl+Enter: exportar"
                            } else {
                                "Tab: recorrer · ↑↓: cantidad · Ctrl+Enter: OK"
                            },
                        ))
                    })
                    .child(
                        div()
                            .ml_auto()
                            .flex()
                            .gap_2()
                            .when(!completed, |buttons| {
                                buttons.child(
                                    Button::new("cancel-action-form")
                                        .small()
                                        .label(if running {
                                            "Cancelar exportación"
                                        } else {
                                            "Cancelar"
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if running {
                                                this.cancel_export();
                                            } else {
                                                this.close_action_form(window, cx);
                                            }
                                        })),
                                )
                            })
                            .when(!running, |buttons| {
                                buttons.child(
                                    Button::new("submit-action-form")
                                        .primary()
                                        .small()
                                        .label(if completed {
                                            "Cerrar"
                                        } else if export.is_some() {
                                            "Exportar audio"
                                        } else {
                                            "OK"
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.submit_action_form(window, cx)
                                        })),
                                )
                            }),
                    ),
            );
        let height = match export {
            Some(_) if running || completed => 260.0,
            Some(export) if export.advanced => 690.0,
            Some(_) => 495.0,
            None => (230.0 + form.fields.len() as f32 * 68.0).min(690.0),
        };
        let card = div()
            .key_context("HayActionForm")
            .on_action(cx.listener(|this, _: &CancelActionForm, window, cx| {
                this.close_action_form(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Escape, window, cx| this.close_action_form(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ConfirmActionForm, window, cx| {
                this.submit_action_form(window, cx)
            }))
            .w((viewport.width - px(32.0)).min(px(620.0)))
            .h((viewport.height - px(32.0)).min(px(height)))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.border))
            .rounded(px(ds.radius_lg))
            .child(
                div()
                    .flex_shrink_0()
                    .px_4()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .child(
                        div()
                            .text_lg()
                            .text_color(rgb(ds.text))
                            .child(form.title.clone()),
                    )
                    .when(export.is_none(), |header| {
                        header.child(
                            div()
                                .text_sm()
                                .text_color(rgb(ds.muted))
                                .child(form.spec.description.clone()),
                        )
                    }),
            )
            .child(fields)
            .child(footer)
            .focus_trap("action-form-focus", &form.focus);
        modal_backdrop(0x05070bd9, close, card)
    }

    pub(super) fn render_action_field(&self, index: usize, cx: &mut Context<Self>) -> Div {
        let form = self.action_form.as_ref().expect("formulario abierto");
        let parameter = &form.spec.parameters[index];
        div()
            .py_2()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(190.0))
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(rgb(self.ds.text))
                            .child(parameter.label.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(self.render_parameter_control(index, cx)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(self.ds.muted))
                    .child(parameter.hint.clone()),
            )
    }

    pub(super) fn render_parameter_control(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let form = self.action_form.as_ref().expect("formulario abierto");
        let parameter = &form.spec.parameters[index];
        match &form.fields[index] {
            ParameterField::Input(state) => match &parameter.kind {
                ParameterKind::Number { unit, .. } => NumberInput::new(state)
                    .small()
                    .w_full()
                    .accessibility_label(parameter.label.clone())
                    .suffix(
                        div()
                            .text_xs()
                            .text_color(rgb(self.ds.muted))
                            .child(unit.clone()),
                    )
                    .into_any_element(),
                _ => Input::new(state)
                    .small()
                    .w_full()
                    .aria_label(parameter.label.clone())
                    .into_any_element(),
            },
            ParameterField::Bool { checked, focus } => div()
                .track_focus(focus)
                .child(
                    Checkbox::new(("action-bool", index))
                        .small()
                        .checked(*checked)
                        .label(if *checked { "Sí" } else { "No" })
                        .accessibility_label(parameter.label.clone())
                        .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                            if let Some(form) = this.action_form.as_mut() {
                                if let Some(ParameterField::Bool { checked: value, .. }) =
                                    form.fields.get_mut(index)
                                {
                                    *value = *checked;
                                }
                                form.error = None;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        }
    }
}
