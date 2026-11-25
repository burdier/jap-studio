//! Arrangement superior y panel inferior del editor; el mixer ocupa el hueco al separarlo.
use crate::ui::{
    CenterView, DawApp,
    components::{
        button::{flat_control, led_dot, panel_shadow},
        splitter::{SplitDrag, horizontal_splitter, vertical_splitter},
    },
    piano_roll::RollAction,
    toolbar::ToolbarAction,
    workspace_focus::Panel,
};
use gpui::{
    Context, FontWeight, IntoElement, MouseButton, MouseDownEvent, StyleRefinement, div,
    prelude::*, px, relative, rgb, rgba,
};

use std::{cell::Cell, rc::Rc};

const TEJER_MAX_FRACTION: f32 = 0.65;

impl DawApp {
    pub(super) fn workspace_with_explorer(&self, cx: &mut Context<Self>) -> gpui::Div {
        let resizing = matches!(self.split_drag, Some(SplitDrag::Explorer { .. }));
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .gap(px(3.0))
            .overflow_hidden()
            .when(self.explorer.width > 0.0, |workspace| {
                let focused = self.workspace_focus.active == Panel::Explorer;
                workspace.child(
                    div()
                        .w(px(self.explorer.width))
                        .h_full()
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .bg(rgb(self.ds.panel))
                        .rounded(px(self.ds.radius_lg))
                        .shadow(panel_shadow(&self.ds))
                        .when(focused, |el| {
                            el.border_t_2().border_color(rgb(self.ds.accent))
                        })
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
    }

    pub(super) fn render_arrangement_workspace(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .gap(px(3.0))
            .overflow_hidden()
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
        let vst3_fx = self.selected_vst3_fx();
        let analyzers = self
            .tracks
            .get(self.cursor_row)
            .is_some_and(|t| !crate::ui::fx::analyzers::nodes(&t.chain).is_empty());
        let focused = self.workspace_focus.active == Panel::Fx;
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(self.ds.panel))
            .rounded(px(self.ds.radius_lg))
            .shadow(panel_shadow(&self.ds))
            .when(focused, |el| {
                el.border_t_2().border_color(rgb(self.ds.accent))
            })
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                this.workspace_focus.active = Panel::Fx;
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
            .children(self.cabinet_panel(cx))
            .when_some(lua_component, |column, target| {
                column.child(
                    div()
                        .id("fx-lua-controls")
                        .max_h(relative(0.72))
                        .flex_shrink_0()
                        .overflow_y_scroll()
                        .child(self.render_lua_component_controls(target, cx)),
                )
            })
            .when_some(vst3_fx, |column, target| {
                column.child(self.render_vst3_fx_controls(target, cx))
            })
    }

    fn render_arrangement_editors(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let instrument_open = self.fx.editor_track.is_some_and(|id| {
            self.tracks
                .get(self.cursor_row)
                .is_some_and(|track| track.id == id && track.instrument.is_some())
        });
        let tejer_open = self.fx.tejer.is_some_and(|target| {
            self.tracks
                .get(self.cursor_row)
                .is_some_and(|track| track.id == target.track)
        });
        let clip_panel = if tejer_open {
            Some(
                self.render_tejer_selector(self.cursor_row, cx)
                    .into_any_element(),
            )
        } else if instrument_open {
            None
        } else {
            self.shown_sample()
                .map(|clip| self.render_sample_editor(clip, cx).into_any_element())
        };
        let mixer_open = self.center_view == crate::ui::CenterView::Mixer;
        let editor_open = mixer_open
            || instrument_open
            || (self.piano_roll.open && self.timeline.midi_clip.is_some());
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
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .when(editor_open, |top| {
                        top.flex_basis(relative(ratio))
                            .flex_grow_0()
                            .flex_shrink_0()
                    })
                    .when_some(clip_panel, |row, panel| {
                        row.child(
                            div()
                                .w(relative(0.44))
                                .when_some(
                                    self.fx.tejer_width.filter(|_| tejer_open),
                                    |el, width| el.w(px(width)).max_w(relative(TEJER_MAX_FRACTION)),
                                )
                                .min_w_0()
                                .h_full()
                                .flex_shrink_0()
                                .overflow_hidden()
                                .child(panel),
                        )
                    })
                    .when(tejer_open, |row| {
                        let sizes = Rc::new(Cell::new((0.0, 0.0)));
                        let measured = sizes.clone();
                        let active = matches!(self.split_drag, Some(SplitDrag::Tejer { .. }));
                        row.on_children_prepainted(move |bounds, _, _| {
                            if let (Some(editor), Some(arrangement)) =
                                (bounds.first(), bounds.last())
                            {
                                let total = f32::from(arrangement.right() - editor.left());
                                measured.set((
                                    f32::from(editor.size.width),
                                    total * TEJER_MAX_FRACTION,
                                ));
                            }
                        })
                        .child(
                            vertical_splitter(&self.ds, active).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    if event.click_count == 2 {
                                        this.fx.tejer_width = None;
                                        this.split_drag = None;
                                    } else {
                                        let (start_width, max_width) = sizes.get();
                                        if max_width > 0.0 {
                                            this.split_drag = Some(SplitDrag::Tejer {
                                                start_x: f32::from(event.position.x),
                                                start_width,
                                                max_width,
                                            });
                                        }
                                    }
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .bg(rgb(self.ds.panel))
                            .rounded(px(self.ds.radius_lg))
                            .shadow(panel_shadow(&self.ds))
                            .when(self.workspace_focus.active == Panel::Arrangement, |el| {
                                el.border_t_2().border_color(rgb(self.ds.accent))
                            })
                            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                                this.workspace_focus.active = Panel::Arrangement;
                                window.focus(&this.focus_handle, cx);
                                this.sync_midi_focus();
                            }))
                            .child(self.render_arrangement(cx)),
                    ),
            )
            .when(editor_open, |workspace| {
                let ds = &self.ds;
                let has_instrument = self
                    .tracks
                    .get(self.cursor_row)
                    .is_some_and(|t| t.instrument.is_some());
                let active_dock_panel = if mixer_open {
                    Panel::Center
                } else if instrument_open {
                    Panel::Instrument
                } else {
                    Panel::PianoRoll
                };
                let dock_focused = self.workspace_focus.active == active_dock_panel;
                let roll_active = !mixer_open && !instrument_open;
                let inst_active = instrument_open && !mixer_open;
                let dock_tab = |id: &'static str, label: &'static str, active: bool| {
                    flat_control(ds, active, ds.accent)
                        .id(id)
                        .h(px(22.0))
                        .px(px(10.0))
                        .gap(px(5.0))
                        .rounded(px(ds.radius_md))
                        .text_size(px(10.5))
                        .font_weight(if active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(rgb(if active { ds.text } else { ds.muted }))
                        .when(active, |el| el.child(led_dot(ds.accent, 4.5, true)))
                        .child(label)
                };
                let dock_header = div()
                    .h(px(32.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(8.0))
                    .bg(rgb(ds.bg))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                dock_tab("dock-tab-roll", "Piano Roll", roll_active).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.set_center_view(CenterView::Arrangement);
                                        this.fx.editor_track = None;
                                        this.workspace_focus.active = Panel::PianoRoll;
                                        if !this.piano_roll.open {
                                            this.execute_toolbar_action(
                                                ToolbarAction::OpenPianoRoll,
                                                window,
                                                cx,
                                            );
                                        }
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .when(has_instrument, |tabs| {
                                tabs.child(
                                    dock_tab("dock-tab-instrument", "Instrumento", inst_active)
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, window, cx| {
                                                this.set_center_view(CenterView::Arrangement);
                                                if let Some(track) =
                                                    this.tracks.get(this.cursor_row)
                                                {
                                                    this.fx.editor_track = Some(track.id);
                                                }
                                                this.workspace_focus.active = Panel::Instrument;
                                                window.focus(&this.focus_handle, cx);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                            })
                            .child(
                                dock_tab("dock-tab-mixer", "Mixer", mixer_open).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.set_center_view(CenterView::Mixer);
                                        this.workspace_focus.active = Panel::Center;
                                        window.focus(&this.focus_handle, cx);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .id("dock-expand")
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(ds.radius_sm))
                                    .cursor_pointer()
                                    .hover(|el| el.bg(rgba((ds.accent << 8) | 0x20)))
                                    .text_size(px(10.0))
                                    .text_color(rgb(ds.muted))
                                    .child("⤢ Ampliar")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            this.workspace_focus.active = active_dock_panel;
                                            this.toggle_workspace_focus(window, cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("dock-close")
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(ds.radius_sm))
                                    .cursor_pointer()
                                    .hover(|el| el.bg(rgba((ds.danger << 8) | 0x26)))
                                    .text_size(px(10.5))
                                    .text_color(rgb(ds.muted))
                                    .child("×")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if mixer_open {
                                                this.set_center_view(CenterView::Arrangement);
                                            } else if instrument_open {
                                                this.fx.editor_track = None;
                                            } else {
                                                this.piano_roll.open = false;
                                            }
                                            this.workspace_focus.active = Panel::Arrangement;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    );
                let bottom = if mixer_open {
                    self.render_mixer(cx).into_any_element()
                } else if instrument_open && self.instrument_detached() {
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
                            .bg(rgb(self.ds.panel))
                            .rounded(px(self.ds.radius_lg))
                            .shadow(panel_shadow(&self.ds))
                            .when(dock_focused, |el| {
                                el.border_t_2().border_color(rgb(self.ds.accent))
                            })
                            .child(dock_header)
                            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                                this.workspace_focus.active = active_dock_panel;
                                if mixer_open || instrument_open {
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
