//! Vista estilo Hay Filter para Hay Chord (Smart Pad) sincronizada con la escala del DAW.
use crate::ui::{
    DawApp,
    components::slider::{FractionSet, Reset, param_slider},
    piano_roll::{RollAction, chord::ChordPadState},
};
use gpui::{
    Context, Div, IntoElement, MouseButton, MouseDownEvent, Stateful, div, prelude::*, px, rgb,
};
use std::rc::Rc;

const ROOT_LABELS: [&str; 12] = [
    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
];
const COLOR_CHOICES: [(&str, u8); 5] =
    [("Básico", 0), ("7ª", 1), ("9ª", 2), ("Sus4", 3), ("5ª", 4)];
const VOICING_CHOICES: [(&str, u8); 3] = [("Cerrado", 0), ("Abierto", 1), ("Drop 2", 2)];

impl DawApp {
    pub(super) fn render_roll_chord_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let pad = &self.piano_roll.chord_pad;
        let scale_root = self.piano_roll.scale_root;
        let scale_minor = self.piano_roll.scale_minor;
        let root_label = ROOT_LABELS[usize::from(scale_root.min(11))];
        let scale_label = if scale_minor { "minor" } else { "major" };

        let inv_fraction = (f32::from(pad.inversion + 2) / 5.0).clamp(0.0, 1.0);
        let inv_label = match pad.inversion {
            0 => "0 · Fundamental".to_string(),
            1 => "+1 · 1ª inv".to_string(),
            2 => "+2 · 2ª inv".to_string(),
            3 => "+3 · 3ª inv".to_string(),
            v => format!("{v} · Grave"),
        };
        let inv_set: FractionSet = Rc::new(cx.listener(|this, fraction: &f32, window, cx| {
            let inv = (fraction * 5.0).round() as i8 - 2;
            this.apply_roll_action(RollAction::ChordInversion(inv), window, cx);
            cx.stop_propagation();
            cx.notify();
        }));
        let inv_reset: Reset = Rc::new(cx.listener(|this, _, window, cx| {
            this.apply_roll_action(RollAction::ChordInversion(0), window, cx);
            cx.stop_propagation();
            cx.notify();
        }));

        let strum_fraction = (f32::from(pad.strum_ticks) / 120.0).clamp(0.0, 1.0);
        let strum_set: FractionSet = Rc::new(cx.listener(|this, fraction: &f32, window, cx| {
            let ticks = ((fraction * 24.0).round() as i16) * 5;
            this.apply_roll_action(RollAction::ChordStrum(ticks), window, cx);
            cx.stop_propagation();
            cx.notify();
        }));
        let strum_reset: Reset = Rc::new(cx.listener(|this, _, window, cx| {
            this.apply_roll_action(RollAction::ChordStrum(0), window, cx);
            cx.stop_propagation();
            cx.notify();
        }));

        div()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(ds.radius_md))
            .border_1()
            .border_color(rgb(ds.frame_border()))
            .bg(rgb(ds.panel_dark))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .pb_1()
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(rgb(ds.text))
                                    .child("Hay Chord"),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(rgb(ds.muted))
                                    .child("Progresiones y voicing en escala"),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                self.roll_button(
                                    format!("{root_label} ▾"),
                                    false,
                                    RollAction::ChooseRoot,
                                    cx,
                                )
                                .h(px(20.0))
                                .px(px(5.0))
                                .text_color(rgb(ds.accent)),
                            )
                            .child(
                                self.roll_button(
                                    format!("{scale_label} ▾"),
                                    false,
                                    RollAction::ChooseScale,
                                    cx,
                                )
                                .h(px(20.0))
                                .px(px(5.0))
                                .text_color(rgb(ds.cyan)),
                            ),
                    ),
            )
            .child({
                let mut color_pills: Vec<Stateful<Div>> = COLOR_CHOICES
                    .into_iter()
                    .map(|(label, idx)| {
                        self.chord_choice_pill(
                            label,
                            pad.color == idx,
                            RollAction::ChordColor(idx),
                            cx,
                        )
                    })
                    .collect();
                color_pills.push(self.chord_choice_pill(
                    "=Todos",
                    false,
                    RollAction::ChordColorAll,
                    cx,
                ));
                self.render_chord_choice_row("Color", color_pills)
            })
            .child({
                let mut voicing_pills: Vec<Stateful<Div>> = VOICING_CHOICES
                    .into_iter()
                    .map(|(label, idx)| {
                        self.chord_choice_pill(
                            label,
                            pad.voicing == idx,
                            RollAction::ChordVoicing(idx),
                            cx,
                        )
                    })
                    .collect();
                voicing_pills.push(self.chord_choice_pill(
                    "+Bajo",
                    pad.bass_octave,
                    RollAction::ChordToggleBass,
                    cx,
                ));
                self.render_chord_choice_row("Voces", voicing_pills)
            })
            .child(param_slider(
                ds,
                "Inversión".into(),
                inv_label,
                inv_fraction,
                inv_set,
                inv_reset,
            ))
            .child(param_slider(
                ds,
                "Rasgueo (Strum)".into(),
                format!("{} ticks", pad.strum_ticks),
                strum_fraction,
                strum_set,
                strum_reset,
            ))
            .child(
                div()
                    .id("hay-chord-midi-mode")
                    .w_full()
                    .px_2()
                    .py(px(5.0))
                    .rounded(px(ds.radius_sm))
                    .border_1()
                    .border_color(rgb(if pad.midi_chord_mode {
                        ds.cyan
                    } else {
                        ds.border
                    }))
                    .bg(rgb(if pad.midi_chord_mode {
                        ds.panel_hover
                    } else {
                        ds.panel
                    }))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.apply_roll_action(RollAction::ChordToggleMidiMode, window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(if pad.midi_chord_mode {
                                        ds.cyan
                                    } else {
                                        ds.text
                                    }))
                                    .child(if pad.midi_chord_mode {
                                        "☑ Acordes con teclado MIDI"
                                    } else {
                                        "☐ Acordes con teclado MIDI"
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(8.5))
                                    .text_color(rgb(ds.muted))
                                    .child("Toca notas MIDI en la escala para probar sus acordes"),
                            ),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(ds.radius_sm))
                            .bg(rgb(if pad.midi_chord_mode {
                                ds.cyan
                            } else {
                                ds.panel_dark
                            }))
                            .text_size(px(9.0))
                            .text_color(rgb(if pad.midi_chord_mode { ds.bg } else { ds.muted }))
                            .child(if pad.midi_chord_mode { "ON" } else { "OFF" }),
                    ),
            )
            .child(self.render_chord_sequence(cx))
            .child(self.render_chord_response_box(cx))
    }

    fn render_chord_choice_row(
        &self,
        label: &'static str,
        pills: impl IntoIterator<Item = Stateful<Div>>,
    ) -> Div {
        let ds = &self.ds;
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .w(px(38.0))
                    .flex_shrink_0()
                    .text_size(px(10.0))
                    .text_color(rgb(ds.muted))
                    .child(label),
            )
            .child(div().flex().flex_wrap().gap(px(3.0)).children(pills))
    }

    fn chord_choice_pill(
        &self,
        label: &'static str,
        active: bool,
        action: RollAction,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        div()
            .id(gpui::SharedString::from(format!("hay-chord-pill-{label}")))
            .px(px(5.0))
            .py(px(2.0))
            .border_1()
            .border_color(rgb(if active { ds.accent } else { ds.frame_border() }))
            .bg(rgb(if active { ds.accent } else { ds.panel }))
            .text_size(px(9.5))
            .text_color(rgb(if active { ds.bg } else { ds.text }))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.apply_roll_action(action, window, cx);
                    this.audition_current_chord_pad();
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
            .child(label)
    }

    fn render_chord_response_box(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let pad = &self.piano_roll.chord_pad;
        let scale_root = self.piano_roll.scale_root;
        let scale_minor = self.piano_roll.scale_minor;
        let (root_pitch, voices) = pad.preview_voices(scale_root, scale_minor);
        let active_roman = ChordPadState::degree_roman(scale_minor, pad.degree);
        let active_name =
            ChordPadState::degree_chord_name(scale_root, scale_minor, pad.degree, pad.color);
        let active_mood = ChordPadState::degree_mood(scale_minor, pad.degree);
        let notes_text = voices
            .iter()
            .map(|&p| ChordPadState::format_note_name(p))
            .collect::<Vec<_>>()
            .join(" · ");

        let degree_pads = (0..7u8).map(|deg| {
            let selected = pad.degree == deg;
            let roman = ChordPadState::degree_roman(scale_minor, deg);
            let chord_name = ChordPadState::degree_chord_name(
                scale_root,
                scale_minor,
                deg,
                pad.color_for(deg),
            );
            let mood = ChordPadState::degree_mood(scale_minor, deg);
            div()
                .id(("hay-chord-degree", usize::from(deg)))
                .w(px(66.0))
                .flex_grow(1.0)
                .min_w(px(56.0))
                .py(px(4.0))
                .px(px(4.0))
                .rounded(px(ds.radius_sm))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(1.0))
                .border_1()
                .border_color(rgb(if selected { ds.accent } else { ds.border }))
                .bg(rgb(if selected { ds.panel_hover } else { ds.panel }))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.apply_roll_action(RollAction::ChordDegree(deg), window, cx);
                        this.audition_current_chord_pad();
                        if event.click_count >= 2 {
                            this.apply_roll_action(RollAction::ChordInsertAtCursor, window, cx);
                        }
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
                        .text_size(px(8.5))
                        .text_color(rgb(if selected { ds.accent } else { ds.muted }))
                        .child(roman),
                )
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .text_size(px(9.5))
                        .text_color(rgb(ds.text))
                        .truncate()
                        .child(chord_name),
                )
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .text_size(px(7.5))
                        .text_color(rgb(ds.muted))
                        .truncate()
                        .child(mood),
                )
        });

        let bass_pitch = pad.bass_octave.then(|| root_pitch.saturating_sub(12));
        let mini_keys = (48u8..72u8).map(|key_pitch| {
            let pc = key_pitch % 12;
            let is_black = matches!(pc, 1 | 3 | 6 | 8 | 10);
            let is_bass = bass_pitch == Some(key_pitch);
            let is_voice = voices.iter().any(|&v| {
                v == key_pitch
                    || (v >= 72 && v - 12 == key_pitch)
                    || (v < 48 && v + 12 == key_pitch)
            });
            let is_root = is_voice && (pc == root_pitch % 12) && !is_bass;
            let key_bg = if is_bass {
                ds.success
            } else if is_root {
                ds.cyan
            } else if is_voice {
                ds.accent
            } else if is_black {
                ds.bg
            } else {
                ds.panel_dark
            };
            div()
                .flex_1()
                .h(px(if is_black { 20.0 } else { 28.0 }))
                .border_1()
                .border_color(rgb(if is_bass || is_root || is_voice {
                    key_bg
                } else {
                    ds.border
                }))
                .bg(rgb(key_bg))
        });

        div()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .p_2()
            .rounded(px(ds.radius_sm))
            .flex()
            .flex_col()
            .gap_2()
            .border_1()
            .border_color(rgb(ds.border))
            .bg(rgb(ds.bg))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .text_size(px(9.5))
                    .child(
                        div()
                            .min_w_0()
                            .text_color(rgb(ds.accent))
                            .truncate()
                            .child(format!("{active_roman} · {active_name} · {active_mood}")),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_color(rgb(ds.cyan))
                            .truncate()
                            .child(notes_text),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .gap(px(3.0))
                    .children(degree_pads),
            )
            .child(
                div()
                    .w_full()
                    .h(px(30.0))
                    .p(px(2.0))
                    .flex()
                    .items_start()
                    .gap(px(1.0))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .bg(rgb(ds.panel))
                    .children(mini_keys),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        self.roll_button("Aplicar", false, RollAction::ChordApplySelection, cx)
                            .flex_1()
                            .min_w(px(80.0))
                            .justify_center(),
                    )
                    .child(
                        self.roll_button("+ Insertar", true, RollAction::ChordInsertAtCursor, cx)
                            .flex_1()
                            .min_w(px(90.0))
                            .justify_center(),
                    ),
            )
            .child({
                let captured_count = self.midi_capture.note_count();
                let has_captured = captured_count > 0;
                let recover_label = if has_captured {
                    format!("↶ Recuperar MIDI ({captured_count})")
                } else {
                    "↶ Recuperar MIDI".into()
                };
                self.roll_button(
                    recover_label,
                    has_captured,
                    RollAction::ChordRecoverMidi,
                    cx,
                )
                .w_full()
                .justify_center()
            })
    }
}
