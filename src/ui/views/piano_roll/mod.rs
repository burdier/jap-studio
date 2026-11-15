//! Editor reutilizado en el panel inferior del Arrangement y en una ventana propia.
mod choices;
mod chord;
mod chord_sequence;
mod control_lane;
mod grid;
mod grid_background;
mod inspector;
mod note_frame;
mod toolbar;

use crate::ui::{DawApp, VimMode, piano_roll::RollAction};
use gpui::{
    Context, FocusHandle, IntoElement, KeyDownEvent, MouseButton, div, prelude::*, px, rgb,
};

impl DawApp {
    pub(in crate::ui) fn render_piano_roll(
        &self,
        focus: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let ds = &self.ds;
        let clip = self.timeline.midi_clip.as_ref().expect("clip del editor");
        let position = self.timeline.meter.position_label(self.piano_roll.cursor);
        let focus = focus.clone();
        div()
            .id("piano-roll-editor")
            .track_focus(&focus)
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(ds.bg))
            .text_color(rgb(ds.text))
            .font_family("monospace")
            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                this.workspace_focus.active = crate::ui::workspace_focus::Panel::PianoRoll;
                window.focus(&focus, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    if this.piano_roll.drag.is_some()
                        || this.piano_roll.control_drag
                        || this.piano_roll.paint_stroke.is_some()
                    {
                        this.finish_roll_drag(event.position);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    if this.piano_roll.drag.is_some()
                        || this.piano_roll.control_drag
                        || this.piano_roll.paint_stroke.is_some()
                    {
                        this.finish_roll_drag(event.position);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                // Continúa el rectángulo/arrastre al cruzar la regla o el inspector.
                if event.dragging() && this.piano_roll.drag.is_some() {
                    this.drag_roll_to(event.position);
                    cx.notify();
                }
                // El separador del explorador sigue funcionando al cruzar sobre el editor.
                if this.split_drag.is_none() && this.track_reorder_drag.is_none() {
                    cx.stop_propagation();
                }
                if this.hover_roll_at(event.position) {
                    cx.notify();
                }
            }))
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .on_pinch(|_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.workspace_key(event, window, cx) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if this.action_form.is_some() {
                    return;
                }
                if this.toolbar_icons.picker_open || this.settings.is_visible() {
                    this.handle_key(event, window, cx);
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if this.handle_roll_grid_key(event) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if this.handle_roll_selection_key(event, window, cx) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if this.piano_roll.choices.is_none()
                    && this.piano_roll.lane_menu.is_none()
                    && this.handle_transport_key(event)
                {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                let key = event
                    .keystroke
                    .key_char
                    .as_deref()
                    .unwrap_or(&event.keystroke.key);
                let modifiers = event.keystroke.modifiers;
                if this.mode == VimMode::Command
                    || (this.vim_enabled
                        && matches!(key, "e" | ":")
                        && this.mode == VimMode::Normal
                        && !modifiers.control
                        && !modifiers.alt
                        && !modifiers.platform)
                {
                    this.handle_key(event, window, cx);
                } else if !modifiers.alt && !modifiers.control && !modifiers.platform {
                    this.handle_roll_key(key, window, cx);
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(rgb(ds.panel_dark))
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(ds.accent))
                            .child("PIANO ROLL EDITOR"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(ds.muted))
                                    .child("MIDI"),
                            )
                            .child(self.workspace_zoom_button(cx))
                            .child(self.roll_button(
                                "Ajustar vista",
                                false,
                                RollAction::ResetView,
                                cx,
                            ))
                            .child(self.roll_button(
                                if self.piano_roll_window.is_some() {
                                    "Acoplar · Esc"
                                } else {
                                    "Separar en ventana"
                                },
                                false,
                                RollAction::ToggleWindow,
                                cx,
                            ))
                            .when(self.piano_roll_window.is_none(), |header| {
                                header.child(self.roll_button(
                                    "Cerrar · Esc",
                                    false,
                                    RollAction::Close,
                                    cx,
                                ))
                            }),
                    ),
            )
            .child(self.render_roll_toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.render_roll_inspector(cx))
                    .child(self.render_roll_grid(cx)),
            )
            .child(
                div()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_4()
                    .bg(rgb(ds.panel_dark))
                    .border_t_1()
                    .border_color(rgb(ds.border))
                    .text_size(px(10.0))
                    .child(div().text_color(rgb(ds.success)).child(self.mode_label()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(ds.muted))
                            .child(self.piano_roll.hint.clone()),
                    )
                    .child(div().text_color(rgb(ds.warning)).child(format!(
                        "Pista {} · Clip: {}  │  {position}  │  {} notas  │  {} {}",
                        clip.track + 1,
                        clip.name,
                        clip.notes().len(),
                        [
                            "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"
                        ][usize::from(self.piano_roll.scale_root)],
                        if self.piano_roll.scale_minor {
                            "minor"
                        } else {
                            "major"
                        }
                    ))),
            )
            .when(self.piano_roll.choices.is_some(), |root| {
                root.child(self.render_roll_choices(cx))
            })
            .children(
                self.piano_roll
                    .lane_menu
                    .as_ref()
                    .map(|menu| self.render_roll_lane_menu(menu, cx)),
            )
    }

    fn render_roll_lane_menu(
        &self,
        menu: &crate::ui::piano_roll::RollLaneMenu,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let active = self.piano_roll.active_lane;
        let mut panel = crate::ui::components::context_menu::menu_panel(ds, 224.0)
            .id("roll-lane-menu-panel")
            .max_h(px(320.0))
            .overflow_y_scroll();
        for (index, item) in crate::ui::piano_roll::lane::LANE_MENU_ITEMS
            .iter()
            .enumerate()
        {
            if let Some(section) = item.section {
                panel = panel.child(
                    div()
                        .px_2()
                        .pt(px(4.0))
                        .pb(px(2.0))
                        .text_size(px(9.5))
                        .text_color(rgb(ds.accent))
                        .child(section),
                );
            }
            let is_active = item.lane == active;
            let is_selected = menu.selected == index;
            panel = panel.child(
                crate::ui::components::context_menu::menu_row(
                    ds,
                    item.lane.label(),
                    is_selected,
                    is_active,
                )
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if let Some(menu) = &mut this.piano_roll.lane_menu {
                        menu.selected = index;
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.apply_roll_action(RollAction::SetLane(item.lane), window, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            );
            if item.separator_after {
                panel = panel.child(crate::ui::components::context_menu::menu_separator(ds));
            }
        }
        let close = std::rc::Rc::new(cx.listener(|this, _, _, cx| {
            this.piano_roll.lane_menu = None;
            cx.notify();
        }));
        let pos = gpui::point(
            menu.position.x.max(px(10.0)),
            (menu.position.y - px(260.0)).max(px(32.0)),
        );
        crate::ui::components::context_menu::context_menu(pos, close, panel)
    }
}
