//! Vista Configuración → Apariencia organizada en tarjetas claras:
//! 1. Tema de color (con muestras de paleta en vivo)
//! 2. Tamaño de fuente (presets + ajuste fino + vista previa)
//! 3. Idioma de la interfaz
//! 4. Lectura e interacción (contraste de selección + efecto del cursor)
use crate::ui::{
    DawApp, DesignSystem,
    components::button::{chip_option, led_dot, panel_shadow},
    cursor_fx::CursorFxMode,
    design::ThemePreset,
    i18n::Language,
};
use gpui::{Context, Div, FontWeight, IntoElement, MouseButton, div, prelude::*, px, rgb, rgba};

fn appearance_card(
    ds: &DesignSystem,
    title: &'static str,
    badge: impl Into<gpui::SharedString>,
    accent: u32,
) -> Div {
    let dark = ds.is_dark();
    div()
        .w_full()
        .min_w_0()
        .p(px(14.0))
        .rounded(px(ds.radius_lg))
        .bg(rgb(ds.panel))
        .border_1()
        .border_color(rgba(if dark { 0xffffff10 } else { 0x0f172a16 }))
        .shadow(panel_shadow(ds))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .pb(px(6.0))
                .border_b_1()
                .border_color(rgba(if dark { 0xffffff0d } else { 0x0f172a12 }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(led_dot(accent, 5.5, false))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(ds.text))
                                .child(title),
                        ),
                )
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .bg(rgb(ds.panel_dark))
                        .font_family("monospace")
                        .text_size(px(10.5))
                        .text_color(rgb(ds.muted))
                        .child(badge.into()),
                ),
        )
}

fn theme_swatch(color: u32) -> Div {
    div()
        .w(px(14.0))
        .h(px(14.0))
        .rounded(px(3.0))
        .bg(rgb(color))
        .border_1()
        .border_color(rgba(0x00000033))
}

impl DawApp {
    pub(super) fn render_appearance_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let dark = ds.is_dark();
        let active_theme = ds.theme;
        let active_cursor = self.cursor_fx.mode();
        let active_lang = self.settings.language;
        let font_px = ds.font_size_xs;
        let font_pct = ds.font_scale_percent();

        let font_presets: [(&'static str, f32); 5] = [
            (self.tr("settings.appearance.font_compact"), 10.5),
            (self.tr("settings.appearance.font_normal"), 12.0),
            (self.tr("settings.appearance.font_large"), 13.5),
            (self.tr("settings.appearance.font_xlarge"), 15.0),
            (self.tr("settings.appearance.font_xxlarge"), 17.0),
        ];

        // 1. Tarjeta de Tema de color con previsualización de paletas
        let theme_card = appearance_card(
            ds,
            self.tr("settings.appearance.color_theme"),
            active_theme.label(),
            ds.accent,
        )
        .child(div().flex().flex_wrap().gap(px(8.0)).children(
            ThemePreset::ALL.into_iter().map(|theme| {
                let pal = theme.palette();
                let active = theme == active_theme;
                let is_dark_theme = {
                    let r = ((pal.bg >> 16) & 0xff) as f32;
                    let g = ((pal.bg >> 8) & 0xff) as f32;
                    let b = (pal.bg & 0xff) as f32;
                    (0.299 * r + 0.587 * g + 0.114 * b) < 128.0
                };
                let polarity = if is_dark_theme {
                    self.tr("settings.appearance.theme_dark")
                } else {
                    self.tr("settings.appearance.theme_light")
                };
                div()
                    .id(("theme-card", theme as usize))
                    .min_w(px(176.0))
                    .flex_1()
                    .p(px(10.0))
                    .rounded(px(ds.radius_md))
                    .bg(rgb(if active {
                        ds.selection_background()
                    } else {
                        ds.panel_dark
                    }))
                    .border_1()
                    .border_color(rgb(if active { pal.accent } else { ds.border }))
                    .cursor_pointer()
                    .hover(move |s| s.border_color(rgb(pal.accent)))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_theme(theme);
                            cx.stop_propagation();
                            cx.notify();
                        }),
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
                                    .gap(px(6.0))
                                    .when(active, |el| el.child(led_dot(pal.accent, 5.0, true)))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(if active {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(rgb(ds.text))
                                            .child(theme.label()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(9.5))
                                    .text_color(rgb(ds.muted))
                                    .child(polarity),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(theme_swatch(pal.bg))
                            .child(theme_swatch(pal.panel))
                            .child(theme_swatch(pal.accent))
                            .child(theme_swatch(pal.cyan))
                            .child(theme_swatch(pal.text)),
                    )
            }),
        ));

        // 2. Tarjeta de Tamaño de fuente (escala global + ajuste fino + vista previa)
        let font_card = appearance_card(
            ds,
            self.tr("settings.appearance.font_size"),
            format!("{font_px:.1} px · {font_pct}%"),
            ds.cyan,
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.0))
                .children(font_presets.into_iter().map(|(label, size)| {
                    let active = (font_px - size).abs() < 0.25;
                    chip_option(ds, label, active, ds.cyan)
                        .py_1()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.set_font_size(size);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                })),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            chip_option(ds, "A−", false, ds.cyan)
                                .px_3()
                                .py_1()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.step_font_size(-0.5);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .min_w(px(64.0))
                                .text_center()
                                .font_family("monospace")
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(ds.text))
                                .child(format!("{font_px:.1} px")),
                        )
                        .child(
                            chip_option(ds, "A+", false, ds.cyan)
                                .px_3()
                                .py_1()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.step_font_size(0.5);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        ),
                )
                .when((font_px - 12.0).abs() >= 0.1, |row| {
                    row.child(
                        chip_option(
                            ds,
                            self.tr("settings.appearance.font_reset"),
                            false,
                            ds.muted,
                        )
                        .py_1()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.set_font_size(12.0);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                    )
                }),
        )
        .child(
            div()
                .w_full()
                .px(px(10.0))
                .py(px(7.0))
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgba(if dark { 0xffffff0c } else { 0x0f172a12 }))
                .text_xs()
                .text_color(rgb(ds.text))
                .child(self.tr("settings.appearance.font_preview")),
        );

        // 3. Tarjeta de Idioma
        let language_card = appearance_card(
            ds,
            self.tr("settings.appearance.language"),
            active_lang.label(),
            ds.cyan,
        )
        .child(div().flex().flex_wrap().gap(px(6.0)).children(
            Language::ALL.into_iter().map(|lang| {
                chip_option(ds, lang.label(), lang == active_lang, ds.cyan)
                    .py_1()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_language(lang);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
            }),
        ));

        // 4. Tarjeta de Lectura e interacción (Contraste de selección + Efecto de cursor)
        let interaction_card = appearance_card(
            ds,
            self.tr("settings.appearance.interaction"),
            format!(
                "{:.0}% · {}",
                ds.selection_contrast * 100.0,
                active_cursor.label()
            ),
            ds.orange,
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(ds.muted))
                                .child(self.tr("settings.appearance.selection_contrast")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(chip_option(
                                    ds,
                                    self.tr("settings.appearance.unselected"),
                                    false,
                                    ds.accent,
                                ))
                                .child(chip_option(
                                    ds,
                                    self.tr("settings.appearance.selected"),
                                    true,
                                    ds.accent,
                                )),
                        ),
                )
                .child(
                    div().flex().flex_wrap().gap(px(6.0)).children(
                        [
                            (self.tr("settings.appearance.contrast_soft"), 0.12_f32),
                            (self.tr("settings.appearance.contrast_medium"), 0.18),
                            (self.tr("settings.appearance.contrast_high"), 0.28),
                        ]
                        .into_iter()
                        .map(|(label, value)| {
                            chip_option(
                                ds,
                                label,
                                (ds.selection_contrast - value).abs() < 0.001,
                                ds.accent,
                            )
                            .py_1()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.set_selection_contrast(value);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                        }),
                    ),
                ),
        )
        .child(div().w_full().h(px(1.0)).my(px(2.0)).bg(rgba(if dark {
            0xffffff0d
        } else {
            0x0f172a12
        })))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(ds.muted))
                        .child(self.tr("settings.appearance.cursor_fx")),
                )
                .child(div().flex().flex_wrap().gap(px(6.0)).children(
                    CursorFxMode::ALL.into_iter().map(|mode| {
                        chip_option(
                            ds,
                            format!("{} {}", mode.badge(), mode.label()),
                            mode == active_cursor,
                            ds.orange,
                        )
                        .py_1()
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
        );

        div()
            .id("settings-panel-scroll")
            .p_4()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child(self.tr("settings.appearance.autosave")),
            )
            .when_some(self.settings.appearance_save_error(), |panel, error| {
                panel.child(div().text_xs().text_color(rgb(ds.danger)).child(format!(
                    "{error}. Vuelve a seleccionar una opción para reintentar."
                )))
            })
            .child(theme_card)
            .child(font_card)
            .child(language_card)
            .child(interaction_card)
            .into_any_element()
    }
}
