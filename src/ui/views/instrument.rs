//! Piano SoundFont: misma jerarquía compacta de dispositivo que Hay Sampler.
use crate::ui::{
    DawApp,
    components::keyboard::{NotePress, midi_keyboard},
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_instrument(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.vst_selected() {
            return self.render_vst(cx).into_any_element();
        }
        if self.sampler_selected() {
            return self.render_sampler(cx).into_any_element();
        }
        let ds = &self.ds;
        let header = div()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .bg(rgb(ds.panel))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(div().text_color(rgb(ds.accent)).child("HAY PIANO"))
                    .child(
                        div()
                            .text_color(rgb(ds.text))
                            .child(self.instrument.active_name().to_owned()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.piano_action(
                        "piano-banks",
                        "Bancos",
                        self.piano_ui.banks_open,
                        ds.accent,
                        cx,
                        |this| {
                            this.piano_ui.banks_open = !this.piano_ui.banks_open;
                        },
                    ))
                    .child(self.piano_action(
                        "piano-refresh",
                        "Recargar bancos",
                        false,
                        ds.muted,
                        cx,
                        |this| {
                            this.instrument.refresh_banks();
                        },
                    ))
                    .child(self.render_window_button(cx)),
            );
        let assigned = self
            .tracks
            .iter()
            .find(|track| track.instrument == Some(self.instrument.id()))
            .map(|track| format!("Pista · {}", track.name))
            .unwrap_or_else(|| "Sin pista asignada · usa [+]".into());
        let source = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(rgb(ds.panel_dark))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(div().text_color(rgb(ds.muted)).child("FUENTE / SOUNDFONT"))
                    .child(
                        div()
                            .text_color(rgb(if self.instrument.loading() {
                                ds.warning
                            } else {
                                ds.muted
                            }))
                            .child(self.instrument.resource_status().to_owned()),
                    ),
            )
            .child(self.render_bank_and_preset_bar(cx))
            .when(self.piano_ui.banks_open, |el| {
                el.child(self.render_bank_pills(cx))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(div().text_color(rgb(ds.cyan)).child(assigned))
                    .child(self.piano_action(
                        "piano-credits",
                        "Créditos",
                        self.piano_ui.credits_open,
                        ds.muted,
                        cx,
                        |this| {
                            this.piano_ui.credits_open = !this.piano_ui.credits_open;
                        },
                    )),
            )
            .when(self.piano_ui.credits_open, |el| {
                el.child(
                    div()
                        .text_color(rgb(ds.muted))
                        .child(self.instrument.active_credits().to_owned()),
                )
            });
        let press: NotePress = Rc::new(cx.listener(|this, note: &u8, _, cx| {
            this.release_mouse_note();
            this.instrument.set_mouse_note(Some(*note));
            this.note_on(*note);
            cx.notify();
        }));
        let keyboard = midi_keyboard(
            self.instrument.layout(),
            ds,
            &|note| self.instrument.held(note),
            press,
        );
        let performance = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(self.render_midi_button(cx))
                    .child(self.render_midi_learn_button(cx))
                    .child(self.render_record_button(cx))
                    .child(self.render_capture_button(cx)),
            )
            .child(
                div()
                    .text_color(rgb(ds.muted))
                    .child(self.controller_status()),
            )
            .child(
                div()
                    .text_color(rgb(ds.muted))
                    .child(elevate::musical::controller::HINT),
            );
        div().id("hay-piano").flex_1().min_h_0().min_w_0().flex().flex_col()
            .text_size(px(11.)).bg(rgb(ds.bg))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some((id, origin, fraction)) = this.piano_ui.drag {
                    if event.dragging() {
                        let delta = f32::from(origin - event.position.y) / if event.modifiers.shift { 1600. } else { 160. };
                        let spec = crate::instruments::piano_params::spec_of(id);
                        this.set_piano_param(id, spec.from_fraction(fraction + delta));
                    } else { this.piano_ui.drag = None; }
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.piano_ui.drag = None; this.release_mouse_note(); cx.notify();
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.piano_ui.drag = None; this.release_mouse_note(); cx.notify();
            }))
            .child(header)
            .child(div().id("piano-body").overflow_y_scroll().flex_1().min_h_0().flex().flex_col()
                .child(source)
                .child(self.render_instrument_params(cx))
                .child(div().flex_shrink_0().px_3().py_2().text_color(rgb(ds.muted))
                    .child("Arrastra perillas o usa la rueda · Shift: precisión · doble clic: restablecer"))
                .child(performance)
                .child(div().h(px(135.)).flex_shrink_0().py_3().px(px(ds.padding_lg)).flex().child(keyboard)))
            .child(div().flex_shrink_0().px_3().py_2().text_color(rgb(ds.muted)).child(self.status_message.clone()))
            .into_any_element()
    }

    pub(in crate::ui) fn render_instrument_placeholder(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .bg(rgb(self.ds.bg))
            .child(div().text_color(rgb(self.ds.muted)).child(format!(
                "{} está en una ventana separada",
                self.active_instrument_name()
            )))
            .child(self.render_window_button(cx))
    }
}
