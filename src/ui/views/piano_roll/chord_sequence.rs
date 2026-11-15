//! Cuadros de la progresión: edición y audición por clic, sin programar notas desde GPUI.
use crate::ui::{
    DawApp,
    piano_roll::{RollAction, chord::ChordPadState, chord_sequence::ChordUnit},
};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_chord_sequence(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let sequence = &self.piano_roll.chord_sequence;
        let scale_root = self.piano_roll.scale_root;
        let minor = self.piano_roll.scale_minor;
        let unit_label = match sequence.unit {
            ChordUnit::Bar => "compás",
            ChordUnit::Beat => "tiempo",
        };
        div()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.bg))
            .child(
                div().flex().items_center().justify_between().gap_1()
                    .child(div().text_size(px(10.)).text_color(rgb(ds.accent))
                        .child(format!("Secuencia · 1 cuadro / {unit_label}")))
                    .child(div().flex().gap_1()
                        .child(self.roll_button("Compás", sequence.unit == ChordUnit::Bar,
                            RollAction::ChordSequenceUnit(ChordUnit::Bar), cx))
                        .child(self.roll_button("Tiempo", sequence.unit == ChordUnit::Beat,
                            RollAction::ChordSequenceUnit(ChordUnit::Beat), cx))),
            )
            .child(
                div().id("chord-sequence-scroller").flex().gap_1().overflow_x_scroll().children(
                    sequence.slots.iter().enumerate().map(|(index, slot)| {
                        self.render_chord_slot(index, *slot, index == sequence.selected,
                            scale_root, minor, cx)
                    }).collect::<Vec<_>>()
                ),
            )
            .child(
                div().flex().items_center().justify_between().gap_1()
                    .child(self.roll_button("−", false, RollAction::ChordSequenceResize(-1), cx))
                    .child(self.roll_button("+ Cuadro", false, RollAction::ChordSequenceResize(1), cx))
                    .child(self.roll_button("Insertar secuencia", true,
                        RollAction::ChordSequenceInsert, cx)),
            )
            .child(div().text_size(px(9.)).text_color(rgb(ds.muted))
                .child("Desde el compás/tiempo del cursor · pulsa para escuchar · ▲/▼ invierten · ↺ reemplaza · × deja silencio."))
    }

    fn render_chord_slot(
        &self,
        index: usize,
        slot: Option<ChordPadState>,
        selected: bool,
        scale_root: u8,
        minor: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let name = slot.map_or_else(
            || "+ Acorde".to_string(),
            |pad| ChordPadState::degree_chord_name(scale_root, minor, pad.degree, pad.color),
        );
        let inv = slot.map_or(String::new(), |pad| format!("Inv {:+}", pad.inversion));
        div()
            .id(("chord-sequence-slot", index))
            .w(px(76.))
            .min_w(px(76.))
            .h(px(92.))
            .p(px(3.))
            .flex()
            .flex_col()
            .items_center()
            .justify_between()
            .border_1()
            .border_color(rgb(if selected { ds.accent } else { ds.border }))
            .bg(rgb(if selected { ds.panel_hover } else { ds.panel }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.apply_roll_action(RollAction::ChordSequenceSlot(index), window, cx);
                    this.audition_chord_slot(index);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.release_chord_audition();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.release_chord_audition();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_size(px(8.))
                    .text_color(rgb(ds.muted))
                    .child(format!("{}", index + 1)),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(ds.text))
                    .truncate()
                    .child(name),
            )
            .child(div().text_size(px(8.)).text_color(rgb(ds.cyan)).child(inv))
            .child(
                div().flex().gap(px(2.)).children(
                    [
                        ("▲", RollAction::ChordSequenceInvert(index, 1)),
                        ("▼", RollAction::ChordSequenceInvert(index, -1)),
                        ("↺", RollAction::ChordSequenceReplace(index)),
                        ("×", RollAction::ChordSequenceClear(index)),
                    ]
                    .map(|(label, action)| {
                        self.roll_button(label, false, action, cx)
                            .w(px(16.))
                            .h(px(17.))
                            .px_0()
                            .justify_center()
                    }),
                ),
            )
    }
}
