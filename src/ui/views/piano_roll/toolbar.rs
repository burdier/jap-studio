//! Barras compactas: composición de los botones ya existentes, sin otro toolkit.
use crate::ui::{
    DawApp,
    components::button::pill_button,
    piano_roll::{RollAction, RollTool},
};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui::views) fn roll_button(
        &self,
        label: impl Into<gpui::SharedString>,
        active: bool,
        action: RollAction,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        pill_button(ds, label, active, ds.accent)
            .h(px(23.0))
            .py_0()
            .px(px(7.0))
            .rounded(px(2.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .text_size(px(10.5))
            .text_color(rgb(if active { ds.panel_dark } else { ds.text }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.apply_roll_action(action, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }

    pub(super) fn render_roll_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let clip = self.timeline.midi_clip.as_ref().expect("clip visible");
        let preview = RollAction::Preview;
        let first = div()
            .id("roll-clip-toolbar")
            .h(px(36.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px_1()
            .overflow_x_scroll()
            .child(self.roll_button("◀", false, preview("clip anterior"), cx))
            .child(
                self.roll_button(
                    format!("Clip: {} (pista {})", clip.name, clip.track + 1),
                    false,
                    preview("selector de clip"),
                    cx,
                )
                .w(px(218.0))
                .overflow_hidden(),
            )
            .child(self.roll_button("▶", false, preview("clip siguiente"), cx))
            .child(self.roll_button(
                format!("Len: {} bars", (clip.duration_beats() / 4.0).ceil() as u64),
                false,
                preview("longitud"),
                cx,
            ))
            .child(self.roll_button("Ch: 1", false, preview("canal MIDI"), cx))
            .child(self.roll_button("Snap: 1/16 ▾", false, preview("snap"), cx))
            .child(self.roll_button("Quantize: 1/16 ▾", false, preview("cuantización"), cx))
            .child(self.roll_button("Swing: 0%", false, preview("swing"), cx))
            .child(separator(ds.border))
            .child(self.roll_button("Q", true, preview("cuantización"), cx))
            .child(self.roll_button("H", false, preview("humanizar"), cx))
            .child(self.roll_button("V: 1/8 ▾", false, preview("duración de nota"), cx))
            .child(separator(ds.border))
            .child(
                self.roll_button(
                    if self.piano_roll.ghost_notes {
                        "Ghost notes ✓"
                    } else {
                        "Ghost notes"
                    },
                    false,
                    RollAction::GhostNotes,
                    cx,
                )
                .text_color(rgb(ds.accent)),
            )
            .child(
                self.roll_button(
                    if self.piano_roll.follow {
                        "Follow ✓"
                    } else {
                        "Follow"
                    },
                    false,
                    RollAction::Follow,
                    cx,
                )
                .text_color(rgb(ds.accent)),
            )
            .child(
                self.roll_button("Loop ⟳ 4 bars", false, preview("loop"), cx)
                    .text_color(rgb(ds.accent)),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(ds.warning))
                    .child("Edited (•)"),
            );
        let tools = [
            ("Select (V)", RollTool::Select),
            ("Draw (B)", RollTool::Draw),
            ("Erase (E)", RollTool::Erase),
            ("Paint (P)", RollTool::Paint),
            ("Mute (M)", RollTool::Mute),
        ];
        let second = div()
            .id("roll-tools-toolbar")
            .h(px(36.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(6.0))
            .px_1()
            .overflow_x_scroll()
            .children(tools.map(|(label, tool)| {
                self.roll_button(
                    label,
                    self.piano_roll.tool == tool,
                    RollAction::Tool(tool),
                    cx,
                )
            }))
            .child(separator(ds.border))
            .children(
                [
                    ("▧", "selección rectangular"),
                    ("✎", "lápiz"),
                    ("↔", "longitud"),
                    ("✂", "cortar"),
                    ("▣", "copiar"),
                    ("▤", "pegar"),
                ]
                .map(|(icon, label)| self.roll_button(icon, false, preview(label), cx)),
            )
            .child(separator(ds.border))
            .child(self.roll_button("Scale: C minor (Aeolian) ▾", false, preview("escala"), cx))
            .child(self.roll_button("Key: C ▾", false, preview("tónica"), cx))
            .child(separator(ds.border))
            .child(self.roll_button("Vel: 100", false, preview("velocity"), cx))
            .child(self.roll_button("Gate: 100%", false, preview("gate"), cx))
            .child(self.roll_button("Humanize: 0%", false, preview("humanizar"), cx))
            .child(div().flex_1())
            .child(self.roll_button("⚙", false, preview("preferencias del editor"), cx));
        div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(first)
            .child(second)
    }
}

fn separator(color: u32) -> Div {
    div()
        .w(px(1.0))
        .h(px(18.0))
        .mx_1()
        .flex_shrink_0()
        .bg(rgb(color))
}
