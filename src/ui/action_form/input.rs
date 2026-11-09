//! Solo eventos de formulario. Selección, edición, IME y portapapeles pertenecen a Kit.
use crate::ui::{DawApp, components::parameter_field::ParameterField};
use gpui::{
    Context, Entity, Subscription, Window,
    component::input::{InputEvent, InputState, NumberInputEvent},
};

pub(super) fn subscribe_fields(
    fields: &[ParameterField],
    window: &mut Window,
    cx: &mut Context<DawApp>,
) -> Vec<Subscription> {
    let mut subscriptions = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if let ParameterField::Bool { focus, .. } = field {
            subscriptions.push(cx.on_focus_in(focus, window, move |this, _, cx| {
                if let Some(form) = &this.action_form {
                    form.scroll.scroll_to_item(index);
                }
                cx.notify();
            }));
        }
        let ParameterField::Input(input) = field else {
            continue;
        };
        subscriptions.push(cx.subscribe_in(
            input,
            window,
            move |this, input, event: &InputEvent, window, cx| {
                if !this.current_form_input(index, &input) {
                    return;
                }
                match event {
                    InputEvent::PressEnter { .. } => this.submit_action_form(window, cx),
                    InputEvent::Change => {
                        if let Some(form) = this.action_form.as_mut() {
                            form.error = None;
                        }
                        cx.notify();
                    }
                    InputEvent::Focus => {
                        if let Some(form) = &this.action_form {
                            form.scroll.scroll_to_item(index);
                        }
                        cx.notify();
                    }
                    InputEvent::Blur => {}
                }
            },
        ));
        subscriptions.push(cx.subscribe_in(
            input,
            window,
            move |this, input, event: &NumberInputEvent, window, cx| {
                if !this.current_form_input(index, &input) {
                    return;
                }
                let NumberInputEvent::Step(action) = event;
                if let Some(form) = this.action_form.as_mut() {
                    form.fields[index].step(&form.spec.parameters[index], *action, window, cx);
                    form.error = None;
                }
                cx.notify();
            },
        ));
    }
    subscriptions
}

impl DawApp {
    fn current_form_input(&self, index: usize, input: &Entity<InputState>) -> bool {
        self.action_form
            .as_ref()
            .and_then(|form| form.fields.get(index))
            .is_some_and(
                |field| matches!(field, ParameterField::Input(current) if current == input),
            )
    }
}
