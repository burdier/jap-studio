//! Buscador Add / FX con estética modular estilo Hay Filter e iconos por tipo (Instrumento / Efecto FX).
use super::super::{
    DawApp, DesignSystem,
    components::modal::{ModalClose, modal_backdrop},
    icons::render_svg_icon,
    palette::{Palette, PaletteEntry},
};
use elevate::catalog::{CatalogAction, EntryKind};
use gpui::{Context, Div, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

const ICON_INSTRUMENT: &[u8] = include_bytes!("../../../assets/icons/toolbar/piano.svg");
const ICON_EFFECT: &[u8] = include_bytes!("../../../assets/icons/toolbar/sliders.svg");
const ICON_PRIMITIVE: &[u8] = include_bytes!("../../../assets/icons/toolbar/wave.svg");
const ICON_ACTION: &[u8] = include_bytes!("../../../assets/icons/toolbar/bolt.svg");
const ICON_LUA: &[u8] = include_bytes!("../../../assets/icons/toolbar/code.svg");
const ICON_SEARCH: &[u8] = include_bytes!("../../../assets/icons/toolbar/word-search.svg");

impl DawApp {
    pub(in crate::ui) fn render_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let selected = self.palette.selected();
        let active_category = self.palette.category();
        let results = self.palette.results();
        let target_track = self.palette.target_track().unwrap_or(self.cursor_row);
        let track_color = ds.track_color(target_track);
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
        let selected_id = results
            .get(selected)
            .map(|entry| entry.id.clone())
            .unwrap_or_else(|| "sin selección".into());

        let list = if results.is_empty() {
            div()
                .p_4()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(ds.muted))
                .child("Sin coincidencias para esta búsqueda o filtro.")
                .into_any_element()
        } else {
            div()
                .id("palette-results-scroll")
                .max_h(px(296.0))
                .overflow_y_scroll()
                .p_1()
                .flex()
                .flex_col()
                .gap_1()
                .children(results.iter().enumerate().map(|(index, entry)| {
                    let choice = entry.clone();
                    let active = index == selected;
                    let is_fx_action = entry.id.starts_with("action.fx.")
                        || entry.id.starts_with("action.utility.");
                    let (icon_bytes, kind_color, badge_label) =
                        entry_visuals(ds, entry, is_fx_action);
                    let unavailable = matches!(entry.action, CatalogAction::Unavailable(_));
                    let cta_label = if unavailable {
                        "Pronto"
                    } else if entry.kind == EntryKind::Instrument {
                        "+ Instrumento"
                    } else if entry.kind == EntryKind::Component || is_fx_action {
                        "+ Efecto FX"
                    } else if entry.has_parameters {
                        "↵ Parámetros"
                    } else if entry.kind == EntryKind::Action {
                        "↵ Ejecutar"
                    } else {
                        "+ Añadir"
                    };

                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .border_1()
                        .border_color(rgb(if active { kind_color } else { ds.border }))
                        .bg(rgb(if active { ds.panel_hover } else { ds.panel }))
                        .cursor_pointer()
                        .on_mouse_move(cx.listener(move |this, _, _, cx| {
                            if this.palette.selected() != index {
                                this.palette.select(index);
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.activate_palette_entry(choice.clone(), window, cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .size(px(28.0))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .border_1()
                                .border_color(rgb(if active { kind_color } else { ds.border }))
                                .bg(rgb(ds.bg))
                                .child(render_svg_icon(icon_bytes, 15.0, kind_color)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(ds.text))
                                                .truncate()
                                                .child(entry.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .px(px(5.0))
                                                .py(px(1.0))
                                                .border_1()
                                                .border_color(rgb(kind_color))
                                                .bg(rgb(ds.bg))
                                                .text_size(px(9.0))
                                                .text_color(rgb(kind_color))
                                                .child(badge_label),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(rgb(ds.muted))
                                        .truncate()
                                        .child(entry.summary.clone()),
                                ),
                        )
                        .child(
                            div()
                                .px_2()
                                .py(px(3.0))
                                .border_1()
                                .border_color(rgb(if unavailable {
                                    ds.warning
                                } else if active {
                                    kind_color
                                } else {
                                    ds.frame_border()
                                }))
                                .bg(rgb(if unavailable {
                                    ds.bg
                                } else if active {
                                    kind_color
                                } else {
                                    ds.panel_dark
                                }))
                                .text_size(px(10.0))
                                .text_color(rgb(if unavailable {
                                    ds.warning
                                } else if active {
                                    ds.bg
                                } else {
                                    kind_color
                                }))
                                .child(cta_label),
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
                .w(px(560.0))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .rounded(px(ds.radius_sm))
                .border_1()
                .border_color(rgb(ds.frame_border()))
                .bg(rgb(ds.panel_dark))
                .child(
                    div()
                        .h(px(3.0))
                        .w_full()
                        .bg(rgb(track_color)),
                )
                .child(
                    div()
                        .pb_2()
                        .border_b_1()
                        .border_color(rgb(ds.border))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(ds.text))
                                        .child("Hay Browser · Add / FX"),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(rgb(ds.muted))
                                        .child("Instrumentos (🎹), Efectos FX (🎛), Primitivos y Acciones"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(3.0))
                                        .flex()
                                        .items_center()
                                        .gap(px(5.0))
                                        .border_1()
                                        .border_color(rgb(track_color))
                                        .bg(rgb(ds.bg))
                                        .text_size(px(10.5))
                                        .text_color(rgb(ds.text))
                                        .child(
                                            div()
                                                .size(px(7.0))
                                                .bg(rgb(track_color)),
                                        )
                                        .child(format!("Destino: {track_name}")),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(3.0))
                                        .border_1()
                                        .border_color(rgb(ds.frame_border()))
                                        .bg(rgb(ds.panel))
                                        .text_size(px(10.0))
                                        .text_color(rgb(ds.muted))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.palette.close();
                                                this.status_message = "Buscador cerrado".into();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                        .child("Esc · Cerrar"),
                                ),
                        ),
                )
                .child(
                    div()
                        .px_3()
                        .py(px(7.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .border_1()
                        .border_color(rgb(ds.border))
                        .bg(rgb(ds.bg))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(render_svg_icon(ICON_SEARCH, 14.0, ds.accent))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_xs()
                                        .text_color(rgb(if self.palette.query().is_empty() {
                                            ds.muted
                                        } else {
                                            ds.text
                                        }))
                                        .child(if self.palette.query().is_empty() {
                                            "Escribe para buscar instrumento, efecto FX, VST3 o acción…"
                                                .to_string()
                                        } else {
                                            format!("{}▌", self.palette.query())
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .border_1()
                                .border_color(rgb(ds.border))
                                .bg(rgb(ds.panel))
                                .text_size(px(10.0))
                                .text_color(rgb(ds.accent))
                                .child(count_label),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .w(px(38.0))
                                        .text_size(px(10.0))
                                        .text_color(rgb(ds.muted))
                                        .child("Filtro"),
                                )
                                .children(Palette::CATEGORIES.into_iter().map(|cat| {
                                    self.render_palette_category_pill(cat, active_category, cx)
                                })),
                        )
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(rgb(ds.muted))
                                .child("Tab · cambiar"),
                        ),
                )
                .child(
                    div()
                        .border_1()
                        .border_color(rgb(ds.border))
                        .bg(rgb(ds.bg))
                        .child(list),
                )
                .child(
                    div()
                        .pt_1()
                        .border_t_1()
                        .border_color(rgb(ds.border))
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(10.0))
                        .text_color(rgb(ds.muted))
                        .child(format!("ID: {selected_id}"))
                        .child("↑↓ elegir · Enter / Clic insertar · Esc cerrar"),
                ),
        )
    }

    fn render_palette_category_pill(
        &self,
        cat: Option<EntryKind>,
        active_category: Option<EntryKind>,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let is_active = cat == active_category;
        let cat_color = category_color(ds, cat);
        let (label, icon) = match cat {
            None => ("Todo", None),
            Some(EntryKind::Instrument) => ("Instrumentos", Some(ICON_INSTRUMENT)),
            Some(EntryKind::Component) => ("Efectos FX", Some(ICON_EFFECT)),
            Some(EntryKind::Primitive) => ("Primitivos", Some(ICON_PRIMITIVE)),
            Some(EntryKind::Action) => ("Acciones", Some(ICON_ACTION)),
        };
        let fg = if is_active { ds.bg } else { ds.text };
        let icon_fg = if is_active { ds.bg } else { cat_color };

        div()
            .px_2()
            .py(px(3.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .border_1()
            .border_color(rgb(if is_active {
                cat_color
            } else {
                ds.frame_border()
            }))
            .bg(rgb(if is_active { cat_color } else { ds.panel }))
            .text_size(px(10.0))
            .text_color(rgb(fg))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.palette.set_category(cat);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .when_some(icon, |pill, bytes| {
                pill.child(render_svg_icon(bytes, 12.0, icon_fg))
            })
            .child(label)
    }
}

fn entry_visuals(
    ds: &DesignSystem,
    entry: &PaletteEntry,
    is_fx_action: bool,
) -> (&'static [u8], u32, &'static str) {
    match entry.kind {
        EntryKind::Instrument => {
            let label = if entry.plugin.is_some() {
                "VST3 INST"
            } else {
                "INSTRUMENTO"
            };
            (ICON_INSTRUMENT, ds.accent, label)
        }
        EntryKind::Component => {
            let label = if entry.plugin.is_some() {
                "VST3 FX"
            } else if entry.component_id.is_some() {
                "LUA FX"
            } else {
                "EFECTO FX"
            };
            (ICON_EFFECT, ds.orange, label)
        }
        EntryKind::Primitive => (ICON_PRIMITIVE, ds.cyan, "PRIMITIVO DSP"),
        EntryKind::Action => {
            if is_fx_action {
                (ICON_EFFECT, ds.orange, "ACCIÓN FX")
            } else if entry.is_tool {
                (ICON_LUA, ds.success, "LUA MIDI")
            } else {
                (ICON_ACTION, ds.success, "ACCIÓN")
            }
        }
    }
}

fn category_color(ds: &DesignSystem, kind: Option<EntryKind>) -> u32 {
    match kind {
        None => ds.accent,
        Some(EntryKind::Instrument) => ds.accent,
        Some(EntryKind::Primitive) => ds.cyan,
        Some(EntryKind::Component) => ds.orange,
        Some(EntryKind::Action) => ds.success,
    }
}
