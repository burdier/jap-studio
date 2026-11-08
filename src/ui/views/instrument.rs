//! Teclado gráfico: solicita notas al controlador; no contiene síntesis.
use super::super::{DawApp, piano_keys::note_name};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, relative, rgb};

const WHITE_NOTES: [u8; 14] = [60, 62, 64, 65, 67, 69, 71, 72, 74, 76, 77, 79, 81, 83];
const BLACK_NOTES: [u8; 10] = [61, 63, 66, 68, 70, 73, 75, 78, 80, 82];

impl DawApp {
    pub(in crate::ui) fn render_instrument(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let heading =
            div()
                .flex_shrink_0()
                .p(px(ds.padding_lg))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .text_color(rgb(ds.accent))
                        .child("Piano · Upright Piano KW"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child("RustySynth · SoundFont CC0 · instrumento → Volume → Master → CPAL"),
                )
                .child(div().text_xs().text_color(rgb(ds.muted)).child(
                    "Teclado: a w s e d f t g y h u j k = C4…C5 · click: nota · Space: liberar",
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.success))
                        .child(self.status_message.clone()),
                );
        let keyboard = div().relative().flex_1().flex().gap(px(2.0)).children(
            WHITE_NOTES.iter().enumerate().map(|(index, &note)| {
                let black = (index + 1 < WHITE_NOTES.len() && BLACK_NOTES.contains(&(note + 1)))
                    .then_some(note + 1);
                self.render_piano_key(note, black, cx)
            }),
        );
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .flex_col()
            .bg(rgb(ds.bg))
            .child(heading)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p(px(ds.padding_lg))
                    .flex()
                    .items_stretch()
                    .child(keyboard),
            )
            .child(div().flex_shrink_0().h(px(ds.padding_lg)))
    }

    fn render_piano_key(
        &self,
        note: u8,
        black: Option<u8>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let pressed = self.held_notes.contains(&note);
        let white_key = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .flex_col()
            .justify_end()
            .items_center()
            .pb(px(8.0))
            .bg(rgb(if pressed { ds.accent } else { ds.text }))
            .border_1()
            .border_color(rgb(ds.border))
            .rounded_b(px(ds.radius_lg))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.release_mouse_note();
                    this.mouse_note = Some(note);
                    this.note_on(note);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(if pressed { ds.bg } else { ds.muted }))
                    .child(note_name(note)),
            );
        div()
            .relative()
            .flex_1()
            .h_full()
            .child(white_key)
            .when_some(black, |key, black| {
                let pressed = self.held_notes.contains(&black);
                key.child(
                    div()
                        .absolute()
                        .top_0()
                        .left(relative(0.38))
                        .w(relative(0.62))
                        .h(relative(0.62))
                        .bg(rgb(if pressed { ds.accent } else { ds.panel_dark }))
                        .border_1()
                        .border_color(rgb(ds.border))
                        .rounded_b(px(ds.radius_md))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.release_mouse_note();
                                this.mouse_note = Some(black);
                                this.note_on(black);
                                cx.notify();
                            }),
                        ),
                )
            })
    }
}
