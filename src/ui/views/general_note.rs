//! Editor modal del apunte general; conserva la presentación común de los formularios.
use crate::ui::{
    DawApp,
    components::modal::{ModalClose, modal_backdrop},
    kit::{CancelGeneralNote, SaveGeneralNote},
};
use gpui::{
    Context, IntoElement, Pixels, Size,
    base::FocusTrapElement,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        input::{Escape, Textarea},
    },
    div,
    prelude::*,
    px, rgb,
};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_general_note(
        &self,
        viewport: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let note = self.general_note.as_ref().expect("apunte abierto");
        let ds = &self.ds;
        let close: ModalClose = Rc::new(cx.listener(|this, _, window, cx| {
            this.close_general_note(window, cx);
            cx.stop_propagation();
        }));
        let card = div()
            .key_context("HayGeneralNote")
            .on_action(cx.listener(|this, _: &CancelGeneralNote, window, cx| {
                this.close_general_note(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &Escape, window, cx| this.close_general_note(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SaveGeneralNote, window, cx| {
                this.save_general_note(window, cx)
            }))
            .w((viewport.width - px(32.0)).min(px(760.0)))
            .h((viewport.height - px(32.0)).min(px(620.0)))
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
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .child(
                        div()
                            .text_lg()
                            .text_color(rgb(ds.text))
                            .child("Nota general"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(ds.muted))
                            .child("Un apunte persistente. Vuelve a :note para editarlo."),
                    ),
            )
            .child(
                div()
                    .id("general-note-editor")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .m_3()
                    .p_2()
                    .bg(rgb(ds.panel))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .child(Textarea::new(&note.input).w_full().h_full()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(ds.border))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("Ctrl+Enter: guardar · Esc: cerrar"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when_some(note.error.clone(), |row, error| {
                                row.child(div().text_sm().text_color(rgb(ds.pink)).child(error))
                            })
                            .child(
                                Button::new("cancel-general-note")
                                    .small()
                                    .label("Cancelar")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_general_note(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("save-general-note")
                                    .primary()
                                    .small()
                                    .label("Guardar")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_general_note(window, cx)
                                    })),
                            ),
                    ),
            )
            .focus_trap("general-note-focus", &note.focus);
        modal_backdrop(0x05070bd9, close, card)
    }
}
