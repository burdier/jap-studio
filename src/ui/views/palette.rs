//! Overlay del buscador. Solo presenta el catálogo y delega la acción al controlador.
use super::super::DawApp;
use elevate::catalog::{CatalogAction, EntryKind};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, relative, rgb, rgba};

impl DawApp {
    pub(in crate::ui) fn render_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let selected = self.palette.selected();
        let results = self.palette.results();
        let count = results.len();
        let count_label = if count == 1 {
            "1 resultado".to_string()
        } else {
            format!("{count} resultados")
        };
        let list = if results.is_empty() {
            div()
                .p_4()
                .text_sm()
                .text_color(rgb(ds.muted))
                .child("Sin coincidencias. Prueba con «piano», «volume» o «patrón».")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(results.iter().enumerate().map(|(index, entry)| {
                    let entry = *entry;
                    let active = index == selected;
                    let kind_color = match entry.kind {
                        EntryKind::Instrument => ds.accent,
                        EntryKind::Primitive => ds.cyan,
                        EntryKind::Component => ds.orange,
                        EntryKind::Action => ds.success,
                    };
                    let unavailable = matches!(entry.action, CatalogAction::Unavailable(_));
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .bg(rgb(if active { ds.accent_dim } else { ds.panel }))
                        .border_b_1()
                        .border_color(rgb(ds.border))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.activate_catalog_entry(entry);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .w(px(100.0))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(rgb(kind_color))
                                .child(entry.kind.label()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().text_sm().text_color(rgb(ds.text)).child(entry.name))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(ds.muted))
                                        .child(entry.summary),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(if unavailable { ds.warning } else { ds.success }))
                                .child(if unavailable {
                                    "no disponible"
                                } else {
                                    "abrir"
                                }),
                        )
                }))
                .into_any_element()
        };
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(rgba(0x000000cc))
            .flex()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.palette.close();
                    this.status_message = "Buscador cerrado".into();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .mt(px(72.0))
                    .w(relative(0.72))
                    .max_w(px(760.0))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(rgb(ds.panel_dark))
                    .border_1()
                    .border_color(rgb(ds.accent))
                    .rounded(px(ds.radius_md))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(rgb(ds.border))
                            .child(div().text_lg().text_color(rgb(ds.accent)).child("❯"))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_lg()
                                    .text_color(rgb(if self.palette.query().is_empty() {
                                        ds.muted
                                    } else {
                                        ds.text
                                    }))
                                    .child(if self.palette.query().is_empty() {
                                        "Buscar instrumento, primitivo, componente o acción…"
                                            .to_string()
                                    } else {
                                        format!("{}▌", self.palette.query())
                                    }),
                            )
                            .child(div().text_xs().text_color(rgb(ds.muted)).child(":add")),
                    )
                    .child(list)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_2()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(count_label)
                            .child("↑↓ mover · Enter abrir · Esc cerrar"),
                    ),
            )
    }
}
