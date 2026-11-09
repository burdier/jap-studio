//! Iconos asignados a las barras: ejecución y acceso al selector.
use crate::ui::{
    DawApp,
    icons::{ToolbarScope, render_svg_icon, toolbar_icon_button},
};
use gpui::{Context, Div, MouseButton, div, prelude::*, px, rgb};

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
        div()
            .flex_1()
            .min_w(px(48.0))
            .h_full()
            .flex()
            .items_center()
            .gap(px(5.0))
            .px_2()
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
                    .gap(px(5.0))
                    .overflow_x_scroll()
                    .children(list.iter().enumerate().map(|(idx, bound)| {
                        let icon = self.toolbar_icons.icon_by_id(&bound.icon_id);
                        let action = bound.action;
                        toolbar_icon_button(ds, icon.svg_bytes, action.short_tag(), accent, compact)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.execute_toolbar_action(action, window, cx);
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
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                    })),
            )
            .child(
                div()
                    .size(px(if compact { 24.0 } else { 28.0 }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(ds.panel_dark))
                    .border_1()
                    .border_color(rgb(ds.border))
                    .rounded(px(ds.radius_sm))
                    .child(render_svg_icon(
                        include_bytes!("../../../assets/icons/toolbar/plus.svg"),
                        16.0,
                        ds.text,
                    ))
                    .flex_shrink_0()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.toolbar_icons.open_picker(scope);
                            window.focus(&this.focus_handle, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }
}
