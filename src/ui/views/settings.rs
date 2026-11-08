//! Panel de configuración y overlay translúcido con shortcuts y motions Vim-like.
use super::{
    super::{
        DawApp,
        components::{
            button::{brand_logo, chip_option},
            modal::{ModalClose, modal_backdrop},
        },
        cursor_fx::CursorFxMode,
        design::ThemePreset,
    },
    tracks::{TcpControlStyle, TcpLayoutPreset, TcpWidget},
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_settings_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let shortcuts = self.settings.shortcuts_open();
        let title = if shortcuts {
            "ATAJOS Y MOTIONS"
        } else {
            "CONFIGURACIÓN"
        };
        let on_close: ModalClose = Rc::new(cx.listener(|this, _, _, cx| {
            this.close_settings_overlay();
            cx.stop_propagation();
            cx.notify();
        }));
        modal_backdrop(
            0x05070bd9,
            on_close,
            div()
                .w(px(720.0))
                .max_w(gpui::relative(0.92))
                .max_h(gpui::relative(0.88))
                .flex()
                .flex_col()
                .overflow_hidden()
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.accent))
                .rounded(px(ds.radius_md))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_5()
                        .py_4()
                        .border_b_1()
                        .border_color(rgb(ds.border))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(brand_logo(20.0))
                                .child(
                                    div()
                                        .text_lg()
                                        .text_color(rgb(ds.accent))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(title),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(ds.muted))
                                .child("Esc · cerrar"),
                        ),
                )
                .child(if shortcuts {
                    self.render_shortcut_help()
                } else {
                    self.render_settings_panel(cx)
                }),
        )
    }

    fn render_settings_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let active_theme = ds.theme;
        let active_cursor = self.cursor_fx.mode();
        let active_preset = ds.track_header.preset();
        div()
            .id("settings-panel-scroll")
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.pink))
                                    .child("TEMAS DE COLOR · ESTUDIO"),
                            )
                            .child(div().text_xs().text_color(rgb(ds.muted)).child(format!(
                                "~/.jaybu.profile → THEME={}",
                                active_theme.token()
                            ))),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        ThemePreset::ALL.into_iter().map(|theme| {
                            chip_option(
                                ds,
                                format!("🎨 {}", theme.label()),
                                theme == active_theme,
                                theme.palette().accent,
                            )
                            .py_2()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.set_theme(theme);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.orange))
                                    .child("PUNTERO GPU · INMERSIÓN"),
                            )
                            .child(div().text_xs().text_color(rgb(ds.muted)).child(format!(
                                "~/.jaybu.profile → CURSOR_FX={}",
                                active_cursor.token()
                            ))),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        CursorFxMode::ALL.into_iter().map(|mode| {
                            chip_option(
                                ds,
                                format!("{} {}", mode.badge(), mode.label()),
                                mode == active_cursor,
                                ds.orange,
                            )
                            .py_2()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                    this.set_cursor_fx(mode);
                                    this.cursor_fx.on_mouse_down(event.position);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.cyan))
                                    .child("TRACK HEADERS · LAYOUT ESTILO REAPER (TCP)"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child("TRACK_HEADER_LAYOUT / TRACK_CONTROL_STYLE"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child("Controles Pan/Vol:"),
                            )
                            .children(TcpControlStyle::ALL.into_iter().map(|style| {
                                chip_option(
                                    ds,
                                    style.label(),
                                    ds.track_header.control_style() == style,
                                    ds.success,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.set_tcp_control_style(style);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                            })),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        TcpLayoutPreset::ALL.into_iter().map(|preset| {
                            chip_option(ds, preset.label(), active_preset == Some(preset), ds.cyan)
                                .py_2()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.set_tcp_preset(preset);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                        }),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child("Elementos visibles:"),
                            )
                            .children(TcpWidget::TOGGLEABLE.into_iter().map(|widget| {
                                chip_option(
                                    ds,
                                    widget.label(),
                                    ds.track_header.is_visible(widget),
                                    ds.accent,
                                )
                                .px_2()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.toggle_tcp_widget(widget);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_xs().text_color(rgb(ds.muted)).child("INTERFAZ"))
                    .child(
                        div()
                            .p_3()
                            .flex()
                            .items_center()
                            .gap_3()
                            .bg(rgb(ds.panel))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .rounded(px(ds.radius_sm))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.show_shortcuts();
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(div().text_lg().text_color(rgb(ds.success)).child("▣"))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(ds.text))
                                            .child("Mostrar shortcuts y motions Vim-like"),
                                    )
                                    .child(
                                        div().text_xs().text_color(rgb(ds.muted)).child(
                                            "Abre una guía translúcida sobre la vista actual",
                                        ),
                                    ),
                            )
                            .child(div().text_xs().text_color(rgb(ds.accent)).child("ABRIR →")),
                    ),
            )
            .into_any_element()
    }

    fn render_shortcut_help(&self) -> gpui::AnyElement {
        let ds = &self.ds;
        let groups: [(&str, &[(&str, &str)]); 4] = [
            (
                "VIM-LIKE / GENERAL",
                &[
                    ("h  j  k  l", "mover cursor / selección según la vista"),
                    ("i", "modo Insert"),
                    (":", "modo Command; escribe un comando y pulsa Enter"),
                    ("Esc", "salir del overlay o liberar notas"),
                    ("Tab", "cambiar a la vista siguiente"),
                    ("1  2  3  4", "Arrangement · Pattern · Mixer · Instrument"),
                ],
            ),
            (
                "ARRANGEMENT",
                &[
                    ("h / l", "mover el cursor en el tiempo"),
                    ("j / k", "seleccionar pista"),
                    ("+ / -", "acercar / alejar"),
                    ("Home", "volver al inicio"),
                    ("Space", "reproducir / detener"),
                ],
            ),
            (
                "COMANDOS",
                &[
                    (":add", "abrir buscador de instrumentos y acciones"),
                    (
                        ":play  :stop  :record  :capture",
                        "transporte y grabación MIDI",
                    ),
                    (":cursor fire|sparks|neon|stars|off", "cambiar puntero GPU"),
                    (":tcp standard|compact|mixer|minimal", "cambiar layout TCP"),
                    ("[  ]  b  R", "volumen -/+ · bypass · recargar diseño"),
                ],
            ),
            (
                "INSTRUMENT / PATTERN",
                &[
                    ("a w s e d f t g y h u j k", "tocar C4–C5 en Instrument"),
                    ("Click en las teclas", "tocar nota; soltar para Note Off"),
                    ("Space", "liberar notas / reproducir patrón"),
                    (
                        "Enter / rueda",
                        "alternar paso / cambiar velocity en Pattern",
                    ),
                ],
            ),
        ];
        div()
            .id("shortcut-help-scroll")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .overflow_y_scroll()
            .children(groups.iter().map(|(heading, shortcuts)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .mb_1()
                            .text_xs()
                            .text_color(rgb(ds.cyan))
                            .child(*heading),
                    )
                    .children(shortcuts.iter().map(|(key, description)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .py_1()
                            .border_b_1()
                            .border_color(rgb(ds.border))
                            .child(
                                div()
                                    .w(px(220.0))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(rgb(ds.accent))
                                    .child(*key),
                            )
                            .child(div().text_xs().text_color(rgb(ds.text)).child(*description))
                    }))
            }))
            .into_any_element()
    }
}
