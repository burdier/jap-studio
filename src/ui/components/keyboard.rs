//! Teclado MIDI clicable con barra de octavas (C3, C4…). Emite la nota pulsada;
//! quien lo usa decide qué hacer con ella (piano, sampler, entrada de pasos…).
use crate::ui::DesignSystem;
use elevate::musical::{keyboard::KeyboardLayout, pitch::note_name};
use gpui::{App, IntoElement, MouseButton, Window, div, prelude::*, px, relative, rgb};
use std::rc::Rc;

/// Callback de pulsación: recibe la nota MIDI.
pub(in crate::ui) type NotePress = Rc<dyn Fn(&u8, &mut Window, &mut App)>;

/// Dibuja el teclado y, debajo, la barra de octavas. `held` solo se consulta al construir.
pub(in crate::ui) fn midi_keyboard(
    layout: &KeyboardLayout,
    ds: &DesignSystem,
    held: &dyn Fn(u8) -> bool,
    on_press: NotePress,
) -> impl IntoElement + use<> {
    let keys = layout.keys().iter().map(|key| {
        let pressed = held(key.note);
        let press = on_press.clone();
        let note = key.note;
        let white = div()
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
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                press(&note, window, cx)
            })
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
            .child(white)
            .when_some(key.black, |slot, black| {
                let press = on_press.clone();
                slot.child(
                    div()
                        .absolute()
                        .top_0()
                        .left(relative(0.38))
                        .w(relative(0.62))
                        .h(relative(0.62))
                        .bg(rgb(if held(black) {
                            ds.accent
                        } else {
                            ds.panel_dark
                        }))
                        .border_1()
                        .border_color(rgb(ds.border))
                        .rounded_b(px(ds.radius_md))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            press(&black, window, cx)
                        }),
                )
            })
    });
    let octaves = layout.keys().iter().map(|key| {
        let label = key.octave_label();
        div()
            .flex_1()
            .h_full()
            .pl(px(4.0))
            .when(label.is_some(), |cell| {
                cell.border_l_1().border_color(rgb(ds.accent))
            })
            .text_xs()
            .text_color(rgb(ds.accent))
            .children(label)
    });
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(div().flex_1().flex().gap(px(2.0)).children(keys))
        .child(
            div()
                .flex_shrink_0()
                .h(px(20.0))
                .mt_1()
                .flex()
                .gap(px(2.0))
                .children(octaves),
        )
}
