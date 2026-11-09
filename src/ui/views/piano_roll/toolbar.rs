//! Barras compactas: composición de los botones ya existentes, sin otro toolkit.
use crate::ui::{
    DawApp,
    components::button::pill_button,
    icons::ToolbarScope,
    piano_roll::{RollAction, RollTool},
};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};impl DawApp {
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
            .child(self.roll_button("◀", false, RollAction::PreviousClip, cx))
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
            .child(self.roll_button("▶", false, RollAction::NextClip, cx))
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
            .child(self.roll_button("Q", false, RollAction::Quantize, cx))
            .child(self.roll_button("H", false, RollAction::Humanize, cx))
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
                .map(|(icon, label)| {
                    let action = match label {
                        "selección rectangular" => RollAction::Tool(RollTool::Box),
                        "longitud" => RollAction::Tool(RollTool::Length),
                        "cortar" => RollAction::Cut,
                        "copiar" => RollAction::Copy,
                        "pegar" => RollAction::Paste,
                        _ => preview(label),
                    };
                    self.roll_button(
                        icon,
                        matches!(
                            (label, self.piano_roll.tool),
                            ("selección rectangular", RollTool::Box)
                                | ("longitud", RollTool::Length)
                        ),
                        action,
                        cx,
                    )
                }),
            )
            .child(separator(ds.border))
            .child(self.roll_button("Scale: C minor (Aeolian) ▾", false, preview("escala"), cx))
            .child(self.roll_button("Key: C ▾", false, preview("tónica"), cx))
            .child(separator(ds.border))
            .child(self.roll_button("Vel: 100", false, preview("velocity"), cx))
            .child(self.roll_button("Gate: 100%", false, preview("gate"), cx))
            .child(self.roll_humanize_control(cx))
            .child(separator(ds.border))
            .child(self.render_toolbar_bound_buttons(ToolbarScope::PianoRoll, true, cx))
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

    /// Control compacto de humanización: `−  Humanize: X%  +` con rueda de ratón.
    fn roll_humanize_control(&self, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let amount = self.piano_roll.humanize_amount;
        let amount_down = amount.saturating_sub(10);
        let amount_up = amount.saturating_add(10).min(100);
        div()
            .flex()
            .items_center()
            .gap(px(1.0))
            // Botón −
            .child(
                self.roll_button("−", false, RollAction::HumanizeAmount(amount_down), cx)
                    .w(px(14.0))
                    .px(px(2.0)),
            )
            // Etiqueta con rueda para ajustar en pasos de 5 %
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(23.0))
                    .px(px(5.0))
                    .rounded(px(2.0))
                    .cursor_pointer()
                    .text_size(px(10.5))
                    .text_color(rgb(ds.text))
                    .on_scroll(cx.listener(|this, event: &gpui::ScrollEvent, window, cx| {
                        let delta = -event.delta.pixel_delta(px(1.0)).y;
                        let step: i16 = if delta > px(0.0) { 5 } else { -5 };
                        let next =
                            (i16::from(this.piano_roll.humanize_amount) + step).clamp(0, 100) as u8;
                        this.apply_roll_action(RollAction::HumanizeAmount(next), window, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(format!("Humanize: {amount}%")),
            )
            // Botón +
            .child(
                self.roll_button("+", false, RollAction::HumanizeAmount(amount_up), cx)
                    .w(px(14.0))
                    .px(px(2.0)),
            )
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
