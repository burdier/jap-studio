//! Navegación, herramientas, edición y botones personalizados del piano roll.
use crate::ui::{
    DawApp,
    components::button::pill_button,
    icons::ToolbarScope,
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
        let edited = self.piano_roll.opened_clip.as_ref().is_some_and(|before| {
            before.id == clip.id
                && (before.name != clip.name
                    || before.props != clip.props
                    || before.channel != clip.channel
                    || before.pattern != clip.pattern
                    || before.muted_notes != clip.muted_notes
                    || before.trim_start != clip.trim_start
                    || before.trim_len != clip.trim_len)
        });
        let first = div()
            .id("roll-clip-toolbar")
            .h(px(36.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px_1()
            .overflow_x_scroll()
            .child(self.roll_button("◀", false, RollAction::PreviousClip, cx))
            .child(
                self.roll_button(
                    format!("Clip: {} (pista {})", clip.name, clip.track + 1),
                    false,
                    RollAction::ChooseClip,
                    cx,
                )
                .w(px(218.0))
                .overflow_hidden(),
            )
            .child(self.roll_button("▶", false, RollAction::NextClip, cx))
            .child(separator(ds.border))
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
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(ds.warning))
                    .child(if edited { "Edited •" } else { "Sin cambios" }),
            )
            .child(separator(ds.border))
            .child(self.render_toolbar_bound_buttons(ToolbarScope::PianoRoll, true, cx))
            .child(self.roll_button("⚙", false, RollAction::OpenSettings, cx));
        let tools = [
            ("↖ Selección", RollTool::Select),
            ("▧ Región", RollTool::Box),
            ("✎ Dibujar", RollTool::Draw),
            ("↔ Longitud", RollTool::Length),
            ("▧ Borrar región", RollTool::EraseRegion),
            ("⌫ Borrar", RollTool::Erase),
            ("▨ Pintar", RollTool::Paint),
            ("M Silenciar", RollTool::Mute),
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
            .child(self.roll_button("✂ Cortar", false, RollAction::Cut, cx))
            .child(self.roll_button("▣ Copiar", false, RollAction::Copy, cx))
            .child(self.roll_button("▤ Pegar", false, RollAction::Paste, cx))
            .child(self.roll_button("⌫ Eliminar", false, RollAction::DeleteSelection, cx))
            .child(separator(ds.border))
            .child(self.roll_button("−", false, RollAction::GridCoarser, cx))
            .child(self.roll_button(
                format!("Grid 1/{}", self.piano_roll.resolution.denominator()),
                false,
                RollAction::Form("midi.grid"),
                cx,
            ))
            .child(self.roll_button("+", false, RollAction::GridFiner, cx))
            .child(self.roll_button(
                format!("Nota 1/{} ▾", self.piano_roll.draw_length.denominator()),
                false,
                RollAction::ChooseDrawLength,
                cx,
            ))
            .child(separator(ds.border))
            .child(self.roll_button("Subdividir", false, RollAction::Subdivide, cx))
            .child(self.roll_button("Velocidad…", false, RollAction::Velocity, cx))
            .child(self.roll_button("Duración…", false, RollAction::Gate, cx))
            .child(self.roll_button("Lua…", false, RollAction::Form("midi.script"), cx));
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
