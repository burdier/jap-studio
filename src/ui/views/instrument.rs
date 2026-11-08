//! Teclado gráfico, selector de bancos FreePats/SoundFont y créditos del autor.
//! La vista solicita notas y cambios de banco a `instruments`; no contiene síntesis.
use super::super::{
    DawApp,
    components::keyboard::{NotePress, midi_keyboard},
};
use gpui::{AnyElement, Context, IntoElement, MouseButton, SharedString, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_instrument(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let name = self.instrument.active_name().to_string();
        let credits = self.instrument.active_credits().to_string();
        let assigned_track = self
            .tracks
            .iter()
            .position(|track| track.instrument == Some(self.instrument.id()));
        let heading = div()
            .flex_shrink_0()
            .p(px(ds.padding_lg))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(div().text_lg().text_color(rgb(ds.accent)).child(name))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.render_midi_button(cx))
                            .child(self.render_record_button(cx))
                            .child(self.render_capture_button(cx))
                            .child(self.render_window_button(cx)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child(format!("Créditos: {credits}")),
            )
            .child(self.render_bank_and_preset_bar(cx))
            .child(self.render_bank_pills(cx))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(if assigned_track.is_some() {
                                ds.success
                            } else {
                                ds.warning
                            }))
                            .child(match assigned_track {
                                Some(index) => format!("Pista: {}", self.tracks[index].name),
                                None => "Sin pista asignada · usa [+] en una cabecera".to_string(),
                            }),
                    )
                    .child(
                        div()
                            .text_color(rgb(if self.controller.enabled() {
                                ds.success
                            } else {
                                ds.warning
                            }))
                            .child(self.controller_status()),
                    )
                    .child(
                        div()
                            .text_color(rgb(ds.muted))
                            .child(elevate::musical::controller::HINT),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(ds.success))
                    .child(self.status_message.clone()),
            );
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
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .flex_col()
            .bg(rgb(ds.bg))
            .child(heading)
            .child(self.render_instrument_params(cx))
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

    fn render_bank_and_preset_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let bank_count = self.instrument.banks().len().max(1);
        let bank_pos = (self.instrument.bank_index() + 1).min(bank_count);
        let bank_label = format!(
            "Banco SoundFont ({bank_pos}/{bank_count}): {}",
            self.instrument.active_name()
        );
        let preset_count = self.instrument.presets().len();
        let preset_label = match self
            .instrument
            .presets()
            .get(self.instrument.preset_index())
        {
            Some(preset) => format!(
                "Preset ({}/{}): {:03}:{:03} {}",
                self.instrument.preset_index() + 1,
                preset_count,
                preset.bank,
                preset.patch,
                preset.name
            ),
            None => "Preset: predeterminado".to_string(),
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .text_xs()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.step_button("bank-prev", "◀", cx, |this| {
                        this.step_soundfont_bank(-1);
                    }))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(ds.panel))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .rounded(px(ds.radius_sm))
                            .text_color(rgb(ds.text))
                            .child(bank_label),
                    )
                    .child(self.step_button("bank-next", "▶", cx, |this| {
                        this.step_soundfont_bank(1);
                    }))
                    .child(
                        self.step_button("bank-refresh", "↻ Recargar bancos", cx, |this| {
                            this.instrument.refresh_banks();
                            this.status_message = format!(
                                "{} bancos SoundFont disponibles en assets/soundfonts/",
                                this.instrument.banks().len()
                            );
                        }),
                    ),
            )
            .when(preset_count > 1, |row| {
                row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(self.step_button("preset-prev", "◀", cx, |this| {
                            this.step_soundfont_preset(-1);
                        }))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .bg(rgb(ds.panel))
                                .border_1()
                                .border_color(rgb(ds.border))
                                .rounded(px(ds.radius_sm))
                                .text_color(rgb(ds.text))
                                .child(preset_label),
                        )
                        .child(self.step_button("preset-next", "▶", cx, |this| {
                            this.step_soundfont_preset(1);
                        })),
                )
            })
    }

    fn render_bank_pills(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let current = self.instrument.bank_index();
        let pills = self
            .instrument
            .banks()
            .iter()
            .enumerate()
            .map(|(index, bank)| {
                let selected = index == current;
                let title = SharedString::from(bank.name.clone());
                div()
                    .id(("sf2-bank", index))
                    .px_2()
                    .py(px(2.0))
                    .text_xs()
                    .rounded(px(ds.radius_sm))
                    .border_1()
                    .border_color(rgb(if selected { ds.accent } else { ds.border }))
                    .bg(rgb(if selected { ds.accent } else { ds.panel_dark }))
                    .text_color(rgb(if selected { ds.bg } else { ds.text }))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.select_soundfont_bank(index);
                            cx.notify();
                        }),
                    )
                    .child(title)
                    .into_any_element()
            });
        div()
            .id("soundfont-bank-list")
            .max_h(px(64.0))
            .overflow_y_scroll()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(pills.collect::<Vec<AnyElement>>())
    }

    fn step_button<F: Fn(&mut Self) + 'static>(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        action: F,
    ) -> impl IntoElement + use<F> {
        let ds = &self.ds;
        div()
            .id(id)
            .px_2()
            .py_1()
            .text_xs()
            .rounded(px(ds.radius_sm))
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.panel))
            .text_color(rgb(ds.accent))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    action(this);
                    cx.notify();
                }),
            )
            .child(label)
    }

    fn render_midi_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let on = self.controller.enabled();
        div()
            .id("controller-toggle")
            .px_2()
            .py_1()
            .text_xs()
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(if on { ds.accent } else { ds.panel }))
            .text_color(rgb(if on { ds.bg } else { ds.accent }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.toggle_controller();
                    cx.notify();
                }),
            )
            .child(if on {
                "Teclado MIDI: ON"
            } else {
                "Teclado MIDI: OFF"
            })
    }

    fn render_record_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let recording = self.midi_capture.is_recording();
        div()
            .id("midi-record-toggle")
            .px_2()
            .py_1()
            .text_xs()
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(if recording { ds.danger } else { ds.border }))
            .bg(rgb(if recording { ds.danger } else { ds.panel }))
            .text_color(rgb(if recording { ds.bg } else { ds.danger }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.toggle_midi_record();
                    cx.notify();
                }),
            )
            .child(if recording {
                "● Grabando MIDI (Detener)"
            } else {
                "● Grabar MIDI"
            })
    }

    fn render_capture_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let count = self.midi_capture.note_count();
        div()
            .id("midi-capture-insert")
            .px_2()
            .py_1()
            .text_xs()
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.panel))
            .text_color(rgb(if count > 0 { ds.cyan } else { ds.muted }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.capture_midi_to_track();
                    cx.notify();
                }),
            )
            .child(format!("↶ Recuperar MIDI a pista ({count})"))
    }

    fn render_window_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let label = if self.instrument_detached() {
            "Acoplar"
        } else {
            "Separar en ventana"
        };
        div()
            .id("instrument-window-toggle")
            .self_start()
            .px_2()
            .py_1()
            .text_xs()
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.panel))
            .text_color(rgb(ds.accent))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.toggle_instrument_window(cx);
                    cx.notify();
                }),
            )
            .child(label)
    }

    /// Marcador del centro mientras el instrumento vive en su propia ventana.
    pub(in crate::ui) fn render_instrument_placeholder(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .bg(rgb(ds.bg))
            .child(div().text_color(rgb(ds.muted)).child(format!(
                "{} está en una ventana separada",
                self.instrument.active_name()
            )))
            .child(self.render_window_button(cx))
    }
}
