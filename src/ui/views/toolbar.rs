//! Iconos asignados a las barras: ejecución y acceso al selector.
use crate::ui::{
    DawApp,
    components::button::flat_control,
    icons::{ToolbarAction, ToolbarScope, render_svg_icon, toolbar_icon_button},
};
use gpui::{Context, Div, MouseButton, component::tooltip::Tooltip, div, prelude::*, px};

impl DawApp {
    pub(in crate::ui) fn render_toolbar_bound_buttons(
        &self,
        scope: ToolbarScope,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let list = match scope {
            ToolbarScope::Transport => &self.toolbar_icons.transport_buttons,
            ToolbarScope::PianoRoll => &self.toolbar_icons.piano_roll_buttons,
        };
        let accent = if scope == ToolbarScope::PianoRoll {
            ds.cyan
        } else {
            ds.orange
        };
        let add_size = if compact { 24.0 } else { ds.transport_btn_size };
        div()
            .flex_1()
            .min_w(px(48.0))
            .h_full()
            .flex()
            .items_center()
            .px_1()
            .when(scope == ToolbarScope::Transport, |strip| {
                strip.h(px(ds.transport_btn_size + 4.0))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, window, cx| {
                    this.toolbar_icons.open_picker(scope);
                    window.focus(&this.focus_handle, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id(match scope {
                        ToolbarScope::Transport => "transport-custom-buttons",
                        ToolbarScope::PianoRoll => "roll-custom-buttons",
                    })
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .overflow_x_scroll()
                    .children(list.iter().enumerate().map(|(idx, bound)| {
                        let icon = self.toolbar_icons.icon_by_id(&bound.icon_id);
                        let action = bound.action;
                        let command = bound.custom_command.clone();
                        let controller = command.as_deref().map_or(
                            action == ToolbarAction::ToggleController,
                            |command| {
                                matches!(command.trim().trim_start_matches(':'), "midi" | "kbd")
                            },
                        );
                        let tuner = command.as_deref().map_or(
                            action == ToolbarAction::Catalog("action.track.tuner"),
                            |command| command.trim().trim_start_matches(':') == "track.tuner",
                        );
                        let active = (tuner && self.tuner_enabled())
                            || (controller && self.controller.enabled());
                        let tag = if controller {
                            if self.controller.enabled() {
                                "MIDI ON"
                            } else {
                                "MIDI OFF"
                            }
                        } else {
                            bound
                                .custom_command
                                .as_deref()
                                .unwrap_or(action.short_tag())
                        };
                        let hint = command.clone().unwrap_or_else(|| action.label().into());
                        toolbar_icon_button(
                            ds,
                            icon.svg_bytes,
                            tag,
                            if controller { ds.success } else { accent },
                            compact,
                            active,
                        )
                        .id(("bound-toolbar-action", idx))
                        .tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                if let Some(command) = &command {
                                    this.submit_command_line(command, window, cx);
                                } else {
                                    this.execute_toolbar_action(action, window, cx);
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, _, _, cx| {
                                this.toolbar_icons.remove_at(scope, idx);
                                this.status_message = format!(
                                    "Icono desvinculado de {} (clic en + para añadir)",
                                    scope.label()
                                );
                                if let Err(error) = this.toolbar_icons.save_bindings() {
                                    this.status_message
                                        .push_str(&format!(" · no se pudo guardar: {error}"));
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    }))
                    .child(
                        flat_control(ds, false, ds.accent)
                            .size(px(add_size))
                            .id("add-toolbar-action")
                            .tooltip(|window, cx| {
                                Tooltip::new("Añadir herramienta").build(window, cx)
                            })
                            .child(render_svg_icon(
                                include_bytes!("../../../assets/icons/toolbar/plus.svg"),
                                if compact { 13.0 } else { 14.0 },
                                ds.muted,
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.toolbar_icons.open_picker(scope);
                                    window.focus(&this.focus_handle, cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }
}
