//! Resumen del instrumento externo; sus controles los dibuja el plugin en IPlugView.
use crate::ui::{DawApp, components::button::pill_button};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, rgb};
impl DawApp {
    pub(in crate::ui) fn render_vst(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .bg(rgb(ds.bg))
            .text_color(rgb(ds.text))
            .child(self.vst.name.clone())
            .child("Usa la ventana original del plugin para editar su sonido.")
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        pill_button(ds, "Abrir editor original", false, ds.accent).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.execute_vst_command("instrument.vst3.editor", cx);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(pill_button(ds, "Quitar", false, ds.danger).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.execute_vst_command("instrument.vst3.remove", cx);
                            cx.notify();
                        }),
                    )),
            )
            .child(self.status_message.clone())
    }
}
