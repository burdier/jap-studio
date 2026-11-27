//! Browser de dispositivos, plugins VST3 y acciones — estética de estudio Dieter Rams + Soft UI.
use super::super::{
    DawApp, DesignSystem,
    components::{
        button::{extruded_shadow, flat_control, led_dot, recessed_well},
        modal::{ModalClose, modal_backdrop},
    },
    design::blend_color,
    icons::render_svg_icon,
    palette::{Palette, PaletteCategory, PaletteEntry},
};
use elevate::catalog::{CatalogAction, EntryKind};
use gpui::{
    BoxShadow, Context, Div, FontWeight, IntoElement, MouseButton, div, prelude::*, px, rgb, rgba,
};
use std::rc::Rc;

const ICON_INSTRUMENT: &[u8] = include_bytes!("../../../assets/icons/toolbar/piano.svg");
const ICON_EFFECT: &[u8] = include_bytes!("../../../assets/icons/toolbar/sliders.svg");
const ICON_PRIMITIVE: &[u8] = include_bytes!("../../../assets/icons/toolbar/wave.svg");
const ICON_ACTION: &[u8] = include_bytes!("../../../assets/icons/toolbar/bolt.svg");
const ICON_LUA: &[u8] = include_bytes!("../../../assets/icons/toolbar/code.svg");
const ICON_SEARCH: &[u8] = include_bytes!("../../../assets/icons/toolbar/word-search.svg");

/// Fila limpia sin bordes duros, compartida con los menús desplegables de la barra.
pub(super) fn palette_card(ds: &DesignSystem, color: u32, active: bool) -> Div {
    let bg = if active {
        blend_color(ds.panel_hover, color, if ds.is_dark() { 14 } else { 10 })
    } else {
        ds.panel
    };
    let hover = blend_color(ds.panel_hover, color, if ds.is_dark() { 10 } else { 7 });
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded(px(ds.radius_md))
        .bg(if active { rgb(bg) } else { rgba(0x00000000) })
        .when(active, |row| row.shadow(extruded_shadow(ds)))
        .hover(move |style| style.bg(rgb(hover)))
        .cursor_pointer()
}

/// Emblema de icono esculpido (sin caja de 1 px).
pub(super) fn palette_icon(
    ds: &DesignSystem,
    icon: &'static [u8],
    color: u32,
    active: bool,
) -> Div {
    let surface = if active {
        blend_color(ds.panel_dark, color, if ds.is_dark() { 24 } else { 14 })
    } else {
        ds.panel_dark
    };
    div()
        .size(px(26.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(ds.radius_sm))
        .bg(rgb(surface))
        .child(render_svg_icon(
            icon,
            13.5,
            if active { color } else { ds.muted },
        ))
}

impl DawApp {
    pub(in crate::ui) fn render_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let selected = self.palette.selected();
        let active_category = self.palette.category();
        let results = self.palette.results();
        let target_track = self.palette.target_track().unwrap_or(self.cursor_row);
        let track_color = ds.track_color(target_track);
        let mut target_label = if self.fx.master_focus && self.palette.fx_target.is_some() {
            "Bus Master".to_string()
        } else {
            self.tracks
                .get(target_track)
                .map(|t| t.name.to_string())
                .unwrap_or_else(|| "Proyecto".into())
        };
        if let Some(target) = self.palette.fx_target
            && let Some(node) = self.fx_node(target)
        {
            target_label.push_str(" › ");
            target_label.push_str(&node.name);
        }
        let count = results.len();
        let count_label = if count == 1 {
            "1".to_string()
        } else {
            count.to_string()
        };
        let selected_entry = results.get(selected).cloned();

        let modal_shadow = if ds.is_dark() {
            vec![
                BoxShadow::new(px(0.0), px(20.0), rgba(0x000000cc).into()).blur_radius(px(44.0)),
                BoxShadow::new(px(0.0), px(4.0), rgba(0x00000088).into()).blur_radius(px(12.0)),
                BoxShadow::new(px(-1.0), px(-1.0), rgba(ds.shadow_light()).into())
                    .blur_radius(px(4.0)),
            ]
        } else {
            vec![
                BoxShadow::new(px(0.0), px(18.0), rgba(0x0f172a38).into()).blur_radius(px(38.0)),
                BoxShadow::new(px(0.0), px(4.0), rgba(0x0f172a1c).into()).blur_radius(px(10.0)),
                BoxShadow::new(px(-1.0), px(-1.0), rgba(0xffffffdd).into()).blur_radius(px(3.0)),
            ]
        };

        let list = if results.is_empty() {
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(ds.text))
                        .child("Sin coincidencias"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child("Prueba con otro término o cambia el filtro con Tab"),
                )
                .into_any_element()
        } else {
            div()
                .id("palette-results-scroll")
                .h(px(340.0))
                .w_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(6.0))
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .children(results.iter().enumerate().map(|(index, entry)| {
                            let choice = entry.clone();
                            let active = index == selected;
                            let is_fx_action = entry.id.starts_with("action.fx.")
                                || entry.id.starts_with("action.utility.");
                            let (icon_bytes, kind_color, short_tag) =
                                entry_visuals(ds, entry, is_fx_action);
                            let unavailable = matches!(entry.action, CatalogAction::Unavailable(_));

                            palette_card(ds, kind_color, active)
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
                                .child(palette_icon(ds, icon_bytes, kind_color, active))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(1.5))
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(if active {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .text_color(rgb(if unavailable {
                                                    ds.muted
                                                } else {
                                                    ds.text
                                                }))
                                                .truncate()
                                                .child(entry.name.clone()),
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
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(ds.radius_sm))
                                                .bg(rgb(ds.panel_dark))
                                                .font_family("monospace")
                                                .text_size(px(9.5))
                                                .text_color(rgb(if unavailable {
                                                    ds.warning
                                                } else if active {
                                                    kind_color
                                                } else {
                                                    ds.muted
                                                }))
                                                .child(if unavailable {
                                                    "PRONTO"
                                                } else {
                                                    short_tag
                                                }),
                                        )
                                        .when(active && !unavailable, |row| {
                                            row.child(led_dot(kind_color, 5.0, true))
                                        }),
                                )
                        })),
                )
                .into_any_element()
        };

        let on_close: ModalClose = Rc::new(cx.listener(|this, _, _, cx| {
            this.palette.close();
            this.status_message = "Buscador cerrado".into();
            cx.stop_propagation();
            cx.notify();
        }));

        let footer_action = selected_entry.clone().map(|entry| {
            let unavailable = matches!(entry.action, CatalogAction::Unavailable(_));
            let is_fx_action =
                entry.id.starts_with("action.fx.") || entry.id.starts_with("action.utility.");
            let (_, kind_color, _) = entry_visuals(ds, &entry, is_fx_action);
            let label = if unavailable {
                "No disponible"
            } else if entry.kind == EntryKind::Instrument {
                "↵ Cargar instrumento"
            } else if entry.kind == EntryKind::Component || is_fx_action {
                "↵ Insertar FX"
            } else if entry.has_parameters {
                "↵ Configurar"
            } else {
                "↵ Ejecutar"
            };
            (entry, unavailable, kind_color, label)
        });

        modal_backdrop(
            if ds.is_dark() { 0x05040ab0 } else { 0x0f172a55 },
            on_close,
            div()
                .w(px(660.0))
                .max_w(gpui::relative(0.94))
                .max_h(gpui::relative(0.88))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(14.0))
                .bg(rgb(ds.panel))
                .shadow(modal_shadow)
                // ── Cabecera de búsqueda limpia (estilo Spotlight / Bitwig Browser) ──
                .child(
                    div()
                        .px(px(16.0))
                        .pt(px(14.0))
                        .pb(px(12.0))
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(ds.muted))
                                                .child("NAVEGADOR DE DISPOSITIVOS"),
                                        )
                                        .child(div().text_color(rgb(ds.muted)).child("·"))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(5.0))
                                                .px(px(8.0))
                                                .py(px(2.0))
                                                .rounded(px(999.0))
                                                .bg(rgb(ds.panel_dark))
                                                .child(led_dot(track_color, 5.5, false))
                                                .child(
                                                    div()
                                                        .max_w(px(220.0))
                                                        .truncate()
                                                        .text_size(px(11.0))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(rgb(ds.text))
                                                        .child(target_label),
                                                ),
                                        ),
                                )
                                .child(
                                    flat_control(ds, false, ds.muted)
                                        .id("palette-close-btn")
                                        .w(px(24.0))
                                        .h(px(24.0))
                                        .rounded_full()
                                        .text_size(px(12.0))
                                        .text_color(rgb(ds.muted))
                                        .child("×")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.palette.close();
                                                this.status_message = "Buscador cerrado".into();
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        )
                        .child(
                            recessed_well(ds)
                                .h(px(40.0))
                                .px(px(12.0))
                                .rounded(px(10.0))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(render_svg_icon(ICON_SEARCH, 15.0, ds.accent))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_size(px(13.5))
                                                .text_color(rgb(
                                                    if self.palette.query().is_empty() {
                                                        ds.muted
                                                    } else {
                                                        ds.text
                                                    },
                                                ))
                                                .child(if self.palette.query().is_empty() {
                                                    "Buscar instrumento, VST3, efecto FX o acción…"
                                                        .to_string()
                                                } else {
                                                    format!("{}▌", self.palette.query())
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .font_family("monospace")
                                        .text_size(px(10.5))
                                        .text_color(rgb(ds.muted))
                                        .child(count_label),
                                ),
                        ),
                )
                // ── Cuerpo de 2 columnas: Categorías (izq) + Resultados (der) ──
                .child(
                    div()
                        .h(px(340.0))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .flex()
                        .border_t_1()
                        .border_b_1()
                        .border_color(rgba(if ds.is_dark() { 0xffffff0d } else { 0x0f172a12 }))
                        .child(
                            div()
                                .w(px(158.0))
                                .h_full()
                                .flex_shrink_0()
                                .p(px(10.0))
                                .bg(rgb(ds.bg))
                                .flex()
                                .flex_col()
                                .justify_between()
                                .child(div().flex().flex_col().gap(px(4.0)).children(
                                    Palette::CATEGORIES.into_iter().map(|cat| {
                                        self.render_palette_category_item(cat, active_category, cx)
                                    }),
                                ))
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .pt(px(8.0))
                                        .text_size(px(10.0))
                                        .text_color(rgb(ds.muted))
                                        .child("Tab · cambiar filtro"),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .min_h_0()
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(list),
                        ),
                )
                // ── Pie inspector compacto con botón de acción principal ──
                .child(
                    div()
                        .h(px(44.0))
                        .px(px(16.0))
                        .bg(rgb(ds.bg))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .font_family("monospace")
                                        .text_size(px(10.0))
                                        .text_color(rgb(ds.muted))
                                        .truncate()
                                        .child(
                                            selected_entry
                                                .as_ref()
                                                .map(|e| e.id.clone())
                                                .unwrap_or_else(|| {
                                                    "↑↓ navegar · Esc cerrar".into()
                                                }),
                                        ),
                                ),
                        )
                        .when_some(footer_action, |bar, (entry, unavailable, color, label)| {
                            bar.child(
                                flat_control(ds, !unavailable, color)
                                    .id("palette-primary-cta")
                                    .h(px(26.0))
                                    .px(px(12.0))
                                    .rounded(px(ds.radius_md))
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(if unavailable { ds.muted } else { color }))
                                    .child(label)
                                    .when(!unavailable, |btn| {
                                        btn.on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, window, cx| {
                                                this.activate_palette_entry(
                                                    entry.clone(),
                                                    window,
                                                    cx,
                                                );
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                    }),
                            )
                        }),
                ),
        )
    }

    fn render_palette_category_item(
        &self,
        cat: PaletteCategory,
        active_category: PaletteCategory,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let is_active = cat == active_category;
        let cat_color = category_color(ds, cat);
        let (label, icon) = match cat {
            PaletteCategory::All => ("Todo", None),
            PaletteCategory::Hey => ("Hey", Some(ICON_EFFECT)),
            PaletteCategory::Instrument => ("Instrumentos", Some(ICON_INSTRUMENT)),
            PaletteCategory::Component => ("Efectos FX", Some(ICON_EFFECT)),
            PaletteCategory::Primitive => ("Primitivos", Some(ICON_PRIMITIVE)),
            PaletteCategory::Action => ("Acciones", Some(ICON_ACTION)),
        };
        let bg = if is_active { ds.panel } else { ds.bg };
        let hover = ds.panel_hover;
        div()
            .h(px(30.0))
            .px(px(10.0))
            .rounded(px(ds.radius_md))
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(bg))
            .when(is_active, |el| el.shadow(extruded_shadow(ds)))
            .hover(move |el| el.bg(rgb(hover)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.palette.set_category(cat);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .when_some(icon, |row, bytes| {
                        row.child(render_svg_icon(
                            bytes,
                            12.5,
                            if is_active { cat_color } else { ds.muted },
                        ))
                    })
                    .when(icon.is_none(), |row| {
                        row.child(led_dot(
                            if is_active { cat_color } else { ds.muted },
                            5.0,
                            is_active,
                        ))
                    })
                    .child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(if is_active {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(rgb(if is_active { ds.text } else { ds.muted }))
                            .child(label),
                    ),
            )
            .when(is_active && icon.is_some(), |row| {
                row.child(led_dot(cat_color, 5.0, true))
            })
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
                "VST3"
            } else {
                "INST"
            };
            (ICON_INSTRUMENT, ds.accent, label)
        }
        EntryKind::Component => {
            let label = if entry.plugin.is_some() {
                "VST3 FX"
            } else if entry.component_id.is_some() {
                "LUA FX"
            } else {
                "FX"
            };
            (ICON_EFFECT, ds.orange, label)
        }
        EntryKind::Primitive => (ICON_PRIMITIVE, ds.cyan, "DSP"),
        EntryKind::Action => {
            if is_fx_action {
                (ICON_EFFECT, ds.orange, "FX")
            } else if entry.is_tool {
                (ICON_LUA, ds.success, "LUA")
            } else {
                (ICON_ACTION, ds.success, "CMD")
            }
        }
    }
}

fn category_color(ds: &DesignSystem, kind: PaletteCategory) -> u32 {
    match kind {
        PaletteCategory::All => ds.accent,
        PaletteCategory::Hey => ds.cyan,
        PaletteCategory::Instrument => ds.accent,
        PaletteCategory::Primitive => ds.cyan,
        PaletteCategory::Component => ds.orange,
        PaletteCategory::Action => ds.success,
    }
}
