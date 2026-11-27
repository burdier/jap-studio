//! Selector visual de Iconos SVG (`assets/icons/toolbar/`) y acciones vinculables
//! estilo REAPER para las áreas libres del Transporte y del Piano Roll.
use crate::ui::{
    DawApp,
    components::{
        button::{chip_option, pill_button},
        modal::modal_backdrop,
    },
    icons::{ToolbarAction, ToolbarScope},
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_icon_picker_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let scope = self.toolbar_icons.picker_scope;
        let selected_action = self.toolbar_icons.selected_action;
        let total_icons = self.toolbar_icons.icons.len();
        let close = Rc::new(cx.listener(|this, _, _, cx| {
            this.toolbar_icons.picker_open = false;
            cx.notify();
        }));
        let actions = ToolbarAction::available(scope, &self.toolbar_icons.action_query);

        modal_backdrop(
            0x00000088,
            close,
            div()
                .w(px(960.0))
                .h(px(680.0))
                .max_w(gpui::relative(0.94))
                .max_h(gpui::relative(0.92))
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.accent))
                .rounded(px(ds.radius_lg))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_base()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(ds.accent))
                                .child(format!(
                                    "BIBLIOTECA DE ICONOS SVG ({total_icons} ICONOS) · VINCULAR A ACCIÓN"
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    chip_option(ds, "↻ Recargar SVGs", false, ds.cyan)
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                let count =
                                                    this.toolbar_icons.reload_custom_icons();
                                                this.status_message = format!(
                                                    "Catálogo de iconos recargado ({} iconos en total)",
                                                    count + 8
                                                );
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    chip_option(ds, "Cerrar · Esc", false, ds.muted).on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.toolbar_icons.picker_open = false;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    chip_option(
                                        ds,
                                        "Barra de Transporte (Acciones Globales)",
                                        scope == ToolbarScope::Transport,
                                        ds.orange,
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.toolbar_icons.open_picker(ToolbarScope::Transport);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .child(
                                    chip_option(
                                        ds,
                                        "Barra del Piano Roll (Acciones Piano Roll)",
                                        scope == ToolbarScope::PianoRoll,
                                        ds.cyan,
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.toolbar_icons.open_picker(ToolbarScope::PianoRoll);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                                ),
                        )
                        .child(self.render_icon_search(false, cx)),
                )
                .child(self.render_icon_browser(cx))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(ds.text))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(format!("2. Acción · {} resultados", actions.len()))
                        .child(self.render_icon_search(true, cx)),
                )
                .child(
                    div()
                        .id("icon-picker-actions-scroll")
                        .h(px(160.0))
                        .flex_shrink_0()
                        .overflow_y_scroll()
                        .when(actions.is_empty(), |el| el.child(
                            div().text_xs().text_color(rgb(ds.muted))
                                .child("No hay acciones con ese filtro.")
                        ))
                        .child(div().flex().flex_wrap().gap_1()
                            .children(actions.into_iter().map(|act| {
                                chip_option(ds, act.label(), act == selected_action, ds.accent)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                        this.toolbar_icons.selected_action = act;
                                        cx.stop_propagation();
                                        cx.notify();
                                    }))
                            }))),
                )
                .child(
                    div()
                        .pt_2()
                        .flex_shrink_0()
                        .gap_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_t_1()
                        .border_color(rgb(ds.border))
                        .child(
                            div().flex_1().min_w_0().text_xs()
                                .text_color(rgb(ds.muted))
                                .child(div().truncate().text_color(rgb(ds.text)).child(format!(
                                    "{} → {}",
                                    self.toolbar_icons.icons.get(self.toolbar_icons.selected_icon)
                                        .map_or("Elige un icono", |icon| icon.label.as_str()),
                                    selected_action.label()
                                )))
                                .child("Tab cambia de búsqueda · Enter asigna · Clic derecho quita el icono"),
                        )
                        .child(
                            pill_button(
                                ds,
                                "＋ Asignar Icono a la Barra",
                                true,
                                ds.success,
                            )
                            .flex_shrink_0()
                            .when(!self.toolbar_icons.can_bind_selected(), |el| el.opacity(0.4))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.assign_toolbar_icon();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                        ),
                ),
        )
    }
}
