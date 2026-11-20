//! Vista ampliada: presenta la misma entidad/editor y conserva el layout subyacente.
use crate::ui::{CenterView, DawApp, workspace_focus::Panel};
use gpui::{
    AnyElement, Context, Div, IntoElement, MouseButton, StyleRefinement, div, prelude::*, px, rgb,
};
impl DawApp {
    pub(in crate::ui) fn workspace_zoom_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .px_2()
            .cursor_pointer()
            .text_size(px(10.))
            .text_color(rgb(self.ds.accent))
            .child(if self.workspace_focus.expanded.is_some() {
                "Restaurar"
            } else if self.vim_enabled {
                "Ampliar · f"
            } else {
                "Ampliar · F11"
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.execute_command_with_context("view.maximize", window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }
    pub(super) fn render_expanded_workspace(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.workspace_focus.expanded?;
        let content = match panel {
            Panel::PianoRoll => self
                .render_piano_roll(&self.piano_roll_focus, cx)
                .into_any_element(),
            Panel::Instrument => self.render_instrument(cx).into_any_element(),
            Panel::Fx => self.render_fx_column(cx).into_any_element(),
            Panel::Explorer => self
                .explorer
                .view
                .clone()
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
            Panel::Arrangement => self.render_arrangement(cx).into_any_element(),
            Panel::Center => match self.center_view {
                CenterView::Session => self.render_pattern(cx).into_any_element(),
                CenterView::Mixer => self.render_mixer(cx).into_any_element(),
                CenterView::Arrangement => self.render_arrangement(cx).into_any_element(),
            },
        };
        Some(
            div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(
                    div()
                        .h(px(22.))
                        .flex_shrink_0()
                        .flex()
                        .justify_end()
                        .items_center()
                        .bg(rgb(self.ds.panel))
                        .child(self.workspace_zoom_button(cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(content),
                )
                .into_any_element(),
        )
    }
}
