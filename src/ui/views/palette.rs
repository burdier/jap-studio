//! Overlay del buscador compacto, centrado, con categorías y efecto liquid.
use super::super::{
    DawApp,
    components::{
        button::{badge_tag, chip_option},
        modal::{ModalClose, modal_backdrop},
    },
    palette::Palette,
};
use elevate::catalog::{CatalogAction, EntryKind};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb, rgba};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let selected = self.palette.selected();
        let active_category = self.palette.category();
        let results = self.palette.results();
        let target_track = self.palette.target_track().unwrap_or(self.cursor_row);
        let liquid_color = ds.track_color(target_track);
        let mut track_name = self
            .tracks
            .get(target_track)
            .map(|t| t.name.to_string())
            .unwrap_or_else(|| "Proyecto".into());

        if let Some(target) = self.palette.fx_target {
            if let Some(node) = self.fx_node(target) {
                track_name.push_str(" / ");
                track_name.push_str(&node.name);
            }
        }
        let count = results.len();
        let count_label = if count == 1 {
            "1 resultado".to_string()
        } else {
            format!("{count} resultados")
        };

        let list = if results.is_empty() {
            div()
                .p_4()
                .text_xs()
                .text_color(rgb(ds.muted))
                .child("Sin coincidencias en esta categoría.")
                .into_any_element()
        } else {
            div()
                .id("palette-results-scroll")
                .max_h(px(250.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(results.iter().enumerate().map(|(index, entry)| {
                    let choice = entry.clone();
                    let active = index == selected;
                    let kind_color = category_color(ds, Some(entry.kind));
                    let unavailable = matches!(entry.action, CatalogAction::Unavailable(_));
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .bg(rgb(if active {
                            ds.selection_background()
                        } else {
                            ds.panel
                        }))
                        .border_b_1()
                        .border_color(rgb(ds.border))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.activate_palette_entry(choice.clone(), window, cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .child(
                            badge_tag(ds, entry.kind.label(), kind_color)
                                .px_2()
                                .py(px(2.0))
                                .border_1()
                                .border_color(rgb(if active { kind_color } else { ds.border })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(ds.text))
                                        .child(entry.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(ds.muted))
                                        .child(entry.summary.clone()),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(if unavailable {
                                    ds.warning
                                } else {
                                    liquid_color
                                }))
                                .child(if unavailable {
                                    "pronto"
                                } else if entry.bundle.is_some() {
                                    "+ cargar"
                                } else if entry.has_parameters {
                                    "↵ parámetros"
                                } else if entry.kind == EntryKind::Action {
                                    "↵ ejecutar"
                                } else {
                                    "+ añadir"
                                }),
                        )
                }))
                .into_any_element()
        };

        let on_close: ModalClose = Rc::new(cx.listener(|this, _, _, cx| {
            this.palette.close();
            this.status_message = "Buscador cerrado".into();
            cx.stop_propagation();
            cx.notify();
        }));

        modal_backdrop(
            0x05070bb8,
            on_close,
            div()
                .p(px(3.0))
                .rounded(px(14.0))
                .bg(rgba((liquid_color << 8) | 0x38))
                .child(
                    div()
                        .w(px(500.0))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .bg(rgb(ds.panel_dark))
                        .border_1()
                        .border_color(rgb(liquid_color))
                        .rounded(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .bg(rgb(ds.panel))
                                .border_b_1()
                                .border_color(rgb(ds.border))
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(2.0))
                                        .rounded(px(10.0))
                                        .bg(rgb(liquid_color))
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(ds.bg))
                                        .child(format!("+ {track_name}")),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .text_color(rgb(if self.palette.query().is_empty() {
                                            ds.muted
                                        } else {
                                            ds.text
                                        }))
                                        .child(if self.palette.query().is_empty() {
                                            "Buscar en el catálogo general…".to_string()
                                        } else {
                                            format!("{}▌", self.palette.query())
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(ds.muted))
                                        .child("Tab · categoría"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_1()
                                .px_3()
                                .py_2()
                                .bg(rgb(ds.panel_dark))
                                .border_b_1()
                                .border_color(rgb(ds.border))
                                .children(Palette::CATEGORIES.into_iter().map(|cat| {
                                    let is_active = cat == active_category;
                                    let cat_color = category_color(ds, cat);
                                    let label = match cat {
                                        None => "Todo",
                                        Some(EntryKind::Instrument) => "Instrumentos",
                                        Some(EntryKind::Primitive) => "Primitivos",
                                        Some(EntryKind::Component) => "Componentes",
                                        Some(EntryKind::Action) => "Acciones",
                                    };
                                    chip_option(ds, label, is_active, cat_color)
                                        .px_2()
                                        .py(px(2.0))
                                        .rounded(px(8.0))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.palette.set_category(cat);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                })),
                        )
                        .child(list)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_3()
                                .py(px(6.0))
                                .bg(rgb(ds.panel))
                                .border_t_1()
                                .border_color(rgb(ds.border))
                                .text_xs()
                                .text_color(rgb(ds.muted))
                                .child(count_label)
                                .child("↑↓ elegir · Tab categoría · Enter añadir · Esc"),
                        ),
                ),
        )
    }
}

fn category_color(ds: &super::super::DesignSystem, kind: Option<EntryKind>) -> u32 {
    match kind {
        None => ds.text,
        Some(EntryKind::Instrument) => ds.accent,
        Some(EntryKind::Primitive) => ds.cyan,
        Some(EntryKind::Component) => ds.orange,
        Some(EntryKind::Action) => ds.success,
    }
}
