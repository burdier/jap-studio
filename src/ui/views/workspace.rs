//! Arrangement superior y panel inferior del editor; el mixer ocupa el hueco al separarlo.
use crate::ui::{
    DawApp,
    components::splitter::{SplitDrag, horizontal_splitter, vertical_splitter},
    piano_roll::RollAction,
};
use gpui::{
    Context, IntoElement, MouseButton, MouseDownEvent, StyleRefinement, div, prelude::*, px,
    relative, rgb,
};

impl DawApp {
    pub(super) fn render_arrangement_workspace(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let resizing = matches!(self.split_drag, Some(SplitDrag::Explorer { .. }));
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .gap(px(3.0))
            .overflow_hidden()
            .when(self.explorer.width > 0.0, |workspace| {
                workspace.child(
                    div()
                        .w(px(self.explorer.width))
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(
                            self.explorer
                                .view
                                .clone()
                                .cached(StyleRefinement::default().size_full()),
                        ),
                )
            })
            .when(self.explorer.width > 0.0 || resizing, |workspace| {
                workspace.child(vertical_splitter(&self.ds, resizing).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        this.split_drag = Some(SplitDrag::Explorer {
                            start_x: f32::from(event.position.x),
                            start_width: this.explorer.width,
                        });
                        window.focus(&this.focus_handle, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ))
            })
            .when(self.fx.width > 0.0, |workspace| {
                workspace.child(
                    div()
                        .w(px(self.fx.width))
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(
                            self.fx
                                .view
                                .clone()
                                .cached(StyleRefinement::default().size_full()),
                        ),
                )
            })
            .when(
                self.fx.width > 0.0 || matches!(self.split_drag, Some(SplitDrag::Fx { .. })),
                |workspace| {
                    workspace.child(
                        vertical_splitter(
                            &self.ds,
                            matches!(self.split_drag, Some(SplitDrag::Fx { .. })),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                this.split_drag = Some(SplitDrag::Fx {
                                    start_x: f32::from(event.position.x),
                                    start_width: this.fx.width,
                                });
                                window.focus(&this.focus_handle, cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                    )
                },
            )
            .child(self.render_arrangement_editors(cx))
    }

    fn render_arrangement_editors(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
