//! Campos de búsqueda y rejilla SVG del selector de iconos.
use crate::ui::{DawApp, icons::render_svg_icon};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_icon_search(&self, actions: bool, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        let query = if actions {
            &self.toolbar_icons.action_query
        } else {
            &self.toolbar_icons.search_query
        };
        let active = self.toolbar_icons.searching_actions == actions;
        div()
            .w(px(260.0))
            .min_w_0()
            .px_3()
            .py_1()
            .rounded(px(ds.radius_sm))
            .bg(rgb(ds.panel))
            .border_1()
            .border_color(rgb(if active { ds.accent } else { ds.border }))
            .text_xs()
            .text_color(rgb(if query.is_empty() { ds.muted } else { ds.text }))
            .truncate()
            .cursor_text()
            .child(if query.is_empty() {
                if actions {
                    "Buscar acciones…".to_string()
                } else {
                    "Buscar iconos…".to_string()
                }
            } else {
                query.clone()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.toolbar_icons.searching_actions = actions;
                    window.focus(&this.focus_handle, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }

    pub(super) fn render_icon_browser(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let query = self.toolbar_icons.search_query.trim().to_lowercase();
        let filtered: Vec<_> = self
            .toolbar_icons
            .icons
            .iter()
            .enumerate()
            .filter(|(_, icon)| {
                query.is_empty()
                    || icon.id.to_lowercase().contains(&query)
                    || icon.label.to_lowercase().contains(&query)
            })
            .collect();
        div()
            .id("icon-picker-grid-scroll")
            .flex_1()
            .min_h_0()
            .p_2()
            .rounded(px(ds.radius_md))
            .bg(rgb(ds.bg))
            .border_1()
            .border_color(rgb(ds.border))
            .overflow_y_scroll()
            .when(filtered.is_empty(), |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child("No hay iconos con ese filtro."),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(filtered.into_iter().map(|(idx, icon)| {
                        let active = idx == self.toolbar_icons.selected_icon;
                        let col = if active { ds.orange } else { ds.text };
                        div()
                            .w(px(168.0))
                            .h(px(38.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded(px(ds.radius_md))
                            .bg(rgb(if active { ds.panel_hover } else { ds.panel }))
                            .border_1()
                            .border_color(rgb(if active { ds.orange } else { ds.border }))
                            .cursor_pointer()
                            .overflow_hidden()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.toolbar_icons.selected_icon = idx;
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(render_svg_icon(icon.svg_bytes, 22.0, col))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(rgb(col))
                                    .child(icon.label.clone()),
                            )
                    })),
            )
    }
}
