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
                        .child(self.render_fx_column(cx)),
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

    pub(in crate::ui) fn render_fx_column(&self, cx: &mut Context<Self>) -> gpui::Div {
        let lua_component = self.selected_lua_component();
        let analyzers = self
            .tracks
            .get(self.cursor_row)
            .is_some_and(|t| !crate::ui::fx::analyzers::nodes(&t.chain).is_empty());
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                this.workspace_focus.active = crate::ui::workspace_focus::Panel::Fx;
                if !this.fx.view.read(cx).focus.is_focused(window) {
                    window.focus(&this.focus_handle, cx);
                }
                this.sync_midi_focus();
            }))
            .child(
                div().flex_1().min_h_0().child(
                    self.fx
                        .view
                        .clone()
                        .cached(StyleRefinement::default().size_full()),
                ),
            )
            .when(analyzers, |column| {
                column.child(
                    div()
                        .id("fx-analyzers")
                        .max_h(relative(0.65))
                        .min_h(px(100.))
                        .flex_shrink_0()
                        .overflow_y_scroll()
                        .child(self.render_fx_analyzers(cx)),
                )
            })
            .when_some(lua_component, |column, target| {
                column.child(self.render_lua_component_controls(target, cx))
            })
    }

    fn render_arrangement_editors(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let instrument_open = self.fx.editor_track.is_some_and(|id| {
            self.tracks
                .get(self.cursor_row)
                .is_some_and(|track| track.id == id && track.instrument.is_some())
        });
        let editor_open =
            instrument_open || self.piano_roll.open && self.timeline.midi_clip.is_some();
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
                    .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                        this.workspace_focus.active =
                            crate::ui::workspace_focus::Panel::Arrangement;
                        window.focus(&this.focus_handle, cx);
                        this.sync_midi_focus();
                    }))
                    .child(self.render_arrangement(cx)),
            )
            .when(editor_open, |workspace| {
                let bottom = if instrument_open && self.instrument_detached() {
                    self.render_instrument_placeholder(cx).into_any_element()
                } else if instrument_open {
                    self.render_instrument(cx).into_any_element()
                } else if self.piano_roll_window.is_some() {
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
                            .border_color(rgb(self.ds.frame_border()))
                            .when(instrument_open, |panel| {
                                panel.child(
                                    div()
                                        .h(px(24.0))
                                        .flex_shrink_0()
                                        .flex()
                                        .justify_end()
                                        .px_2()
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .text_color(rgb(self.ds.muted))
                                                .child("Cerrar instrumento ×")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.fx.editor_track = None;
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    }),
                                                ),
                                        ),
                                )
                            })
                            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                                this.workspace_focus.active = if instrument_open {
                                    crate::ui::workspace_focus::Panel::Instrument
                                } else {
                                    crate::ui::workspace_focus::Panel::PianoRoll
                                };
                                if instrument_open {
                                    window.focus(&this.focus_handle, cx);
                                    this.sync_midi_focus();
                                }
                            }))
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
                    .child("Mixer · Piano Roll en ventana separada")
                    .child(self.roll_button(
                        "Acoplar piano roll",
                        false,
                        RollAction::ToggleWindow,
                        cx,
                    )),
            )
            .child(self.render_mixer(cx))
    }
}
