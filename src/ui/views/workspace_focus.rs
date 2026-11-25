//! Vista ampliada: presenta la misma entidad/editor y conserva el layout subyacente.
use crate::ui::{
    CenterView, DawApp,
    components::button::{flat_control, led_dot},
    workspace_focus::Panel,
};
use gpui::{
    AnyElement, Context, Div, FontWeight, IntoElement, MouseButton, StyleRefinement, div,
    prelude::*, px, rgb,
};
impl DawApp {
    pub(in crate::ui) fn workspace_zoom_button(&self, cx: &mut Context<Self>) -> Div {
        let expanded = self.workspace_focus.expanded.is_some();
        flat_control(&self.ds, expanded, self.ds.accent)
            .h(px(24.0))
            .px(px(9.0))
            .gap(px(5.0))
            .rounded(px(self.ds.radius_md))
            .text_size(px(10.5))
            .text_color(rgb(if expanded {
                self.ds.accent
            } else {
                self.ds.muted
            }))
            .child(if expanded {
                "⤡ Restaurar (F11 / Esc)"
            } else {
                "⤢ F11"
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
        let panel_name = panel.label(self.center_view);
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
                .gap(px(6.0))
                .overflow_hidden()
                .child(
                    div()
                        .h(px(32.))
                        .flex_shrink_0()
                        .flex()
                        .justify_between()
                        .items_center()
                        .px_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.0))
                                .text_xs()
                                .child(led_dot(self.ds.accent, 6.0, true))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(self.ds.text))
                                        .child(format!("PANEL AMPLIADO · {panel_name}")),
                                ),
                        )
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
