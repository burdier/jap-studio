//! Acciones de interpretación compartidas por la vista compacta y la ventana separada.
use crate::ui::{DawApp, components::button::pill_button};
use gpui::{Context, IntoElement, MouseButton, SharedString, prelude::*};

impl DawApp {
    pub(super) fn piano_action<F: Fn(&mut Self) + 'static, L: Into<SharedString>>(
        &self,
        id: &'static str,
        label: L,
        selected: bool,
        color: u32,
        cx: &mut Context<Self>,
        action: F,
    ) -> impl IntoElement + use<F, L> {
        pill_button(&self.ds, label, selected, color)
            .id(id)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    action(this);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }

    pub(in crate::ui) fn render_midi_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let on = self.controller.enabled();
        self.piano_action(
            "controller-toggle",
            if on {
                "Teclado MIDI · ON"
            } else {
                "Teclado MIDI"
            },
            on,
            self.ds.accent,
            cx,
            |this| this.toggle_controller(),
        )
    }

    pub(super) fn render_midi_learn_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        self.piano_action(
            "midi-learn-toggle",
            "MIDI Learn",
            self.midi_mapper.learn_mode(),
            self.ds.warning,
            cx,
            |this| this.toggle_midi_learn(),
        )
    }

    pub(in crate::ui) fn render_record_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let recording = self.midi_capture.is_recording();
        self.piano_action(
            "midi-record-toggle",
            if recording {
                "■ Detener REC"
            } else {
                "● Grabar MIDI"
            },
            recording,
            self.ds.danger,
            cx,
            |this| this.toggle_midi_record(),
        )
    }

    pub(in crate::ui) fn render_capture_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let count = self.midi_capture.note_count();
        let tip = self.tr("transport.midi_capture_tooltip");
        pill_button(
            &self.ds,
            format!("↶ Recuperar MIDI ({count})"),
            false,
            if count > 0 {
                self.ds.cyan
            } else {
                self.ds.muted
            },
        )
        .id("midi-capture-insert")
        .tooltip(move |window, cx| gpui::component::tooltip::Tooltip::new(tip).build(window, cx))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.capture_midi_to_track();
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _, _, cx| {
                this.clear_midi_capture_buffer();
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }

    pub(super) fn render_window_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        pill_button(
            &self.ds,
            if self.instrument_detached() {
                "Acoplar"
            } else {
                "Separar"
            },
            false,
            self.ds.accent,
        )
        .id("instrument-window-toggle")
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.toggle_instrument_window(cx);
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }
}
