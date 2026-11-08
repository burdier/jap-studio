//! Editor reutilizado en el panel inferior del Arrangement y en una ventana propia.
mod grid;
mod grid_background;
mod inspector;
mod toolbar;

use crate::ui::{
    DawApp,
    components::{button::chip_option, context_menu::context_menu},
    piano_roll::RollAction,
};
use elevate::musical::time::PPQ;
use gpui::{
    Context, FocusHandle, IntoElement, KeyDownEvent, MouseButton, div, prelude::*, px, rgb,
};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_midi_clip_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.piano_roll.menu.expect("menú visible");
        let ds = &self.ds;
        let close = Rc::new(cx.listener(|this, _, _, cx| {
            this.piano_roll.menu = None;
            cx.notify();
        }));
        context_menu(
            menu.position,
            close,
            div()
                .w(px(240.0))
                .p_1()
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.border))
                .rounded(px(ds.radius_md))
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child(format!(
                            "{} · {:.2} beats",
                            self.tracks[menu.track].name, menu.beat
                        )),
                )
                .child(
                    chip_option(ds, "＋ Insertar MIDI clip", true, ds.accent)
                        .w_full()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.insert_midi_clip_from_menu();
                                this.focus_piano_roll(window, cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child("8 compases · mock de referencia"),
                ),
        )
    }

    pub(in crate::ui) fn render_piano_roll(
        &self,
        focus: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let clip = self.timeline.midi_clip.as_ref().expect("clip del editor");
        let cursor = self.piano_roll.cursor.0;
        let position = format!(
            "{}.{}.{}",
            cursor / (4 * PPQ) + 1,
            cursor / PPQ % 4 + 1,
            cursor % PPQ / (PPQ / 4) + 1
        );
        let focus = focus.clone();
        div()
            .id("piano-roll-editor")
            .track_focus(&focus)
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(ds.bg))
            .text_color(rgb(ds.text))
            .font_family("monospace")
            .capture_any_mouse_down(move |_, window, cx| window.focus(&focus, cx))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event
                    .keystroke
                    .key_char
                    .as_deref()
                    .unwrap_or(&event.keystroke.key);
                this.handle_roll_key(key, window, cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(rgb(ds.panel_dark))
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(ds.accent))
                            .child("PIANO ROLL EDITOR"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(ds.muted))
                                    .child("MOCK"),
                            )
                            .child(self.roll_button(
                                if self.piano_roll_window.is_some() {
                                    "Acoplar · Esc"
                                } else {
                                    "Separar en ventana"
                                },
                                false,
                                RollAction::ToggleWindow,
                                cx,
                            ))
                            .when(self.piano_roll_window.is_none(), |header| {
                                header.child(self.roll_button(
                                    "Cerrar · Esc",
                                    false,
                                    RollAction::Close,
                                    cx,
                                ))
                            }),
                    ),
            )
            .child(self.render_roll_toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.render_roll_inspector(cx))
                    .child(self.render_roll_grid(cx)),
            )
            .child(
                div()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_4()
                    .bg(rgb(ds.panel_dark))
                    .border_t_1()
                    .border_color(rgb(ds.border))
                    .text_size(px(10.0))
                    .child(div().text_color(rgb(ds.success)).child("NORMAL"))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(ds.muted))
                            .child(self.piano_roll.hint.clone()),
                    )
                    .child(div().text_color(rgb(ds.warning)).child(format!(
                        "Pista {} · Clip: {}  │  {position}  │  {} notas  │  C minor",
                        clip.track + 1,
                        clip.name,
                        clip.notes().len()
                    ))),
            )
    }
}
