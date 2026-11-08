//! Arrangement superior y panel inferior del editor; el mixer ocupa el hueco al separarlo.
use crate::ui::{
    DawApp,
    components::splitter::{SplitDrag, horizontal_splitter},
    piano_roll::RollAction,
};
use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, div, prelude::*, px, relative, rgb};

impl DawApp {
    pub(super) fn render_arrangement_workspace(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor_open = self.piano_roll.open && self.timeline.midi_clip.is_some();
        let dragging_split = matches!(self.split_drag, Some(SplitDrag::WorkspaceVertical { .. }));
        let ratio = self.workspace_split_ratio;
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .when(editor_open, |top| {
                        top.flex_basis(relative(ratio))
                            .flex_grow_0()
                            .flex_shrink_0()
                    })
                    .child(self.render_arrangement(cx)),
            )
            .when(editor_open, |workspace| {
                let bottom = if self.piano_roll_window.is_some() {
                    self.render_mixer_dock(cx).into_any_element()
                } else {
                    self.render_piano_roll(&self.piano_roll_focus, cx)
                        .into_any_element()
                };
                workspace
                    .child(
                        horizontal_splitter(&self.ds, dragging_split)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                    this.split_drag = Some(SplitDrag::WorkspaceVertical {
                                        start_y: f32::from(event.position.y),
                                        start_ratio: this.workspace_split_ratio,
                                    });
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, _, _, cx| {
                                    this.workspace_split_ratio = 0.38;
                                    this.status_message =
                                        "División Arrangement / Editor restablecida".into();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .rounded(px(self.ds.radius_lg))
                            .border_1()
                            .border_color(rgb(self.ds.accent))
                            .child(bottom),
                    )
            })
    }

    fn render_mixer_dock(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .bg(rgb(self.ds.panel_dark))
                    .text_size(px(11.0))
                    .child("MIXER / MOCK · Piano roll en ventana separada")
                    .child(self.roll_button(
                        "Acoplar piano roll",
                        false,
                        RollAction::ToggleWindow,
                        cx,
                    )),
            )
            .child(self.render_mixer())
    }
}
