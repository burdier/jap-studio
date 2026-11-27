use crate::ui::{
    DawApp,
    components::button::chip_option,
    cursor_fx::CursorFxMode,
    design::ThemePreset,
    i18n::{Language, UiText},
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, rgb};
impl DawApp {
    pub(super) fn render_appearance_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let active_theme = ds.theme;
        let active_cursor = self.cursor_fx.mode();
        let active_lang = self.settings.language;
        div()
            .id("settings-panel-scroll")
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(ds.muted))
                    .child(self.tr(UiText::AppearanceAutoSave)),
            )
            .when_some(self.settings.appearance_save_error(), |panel, error| {
                panel.child(div().text_xs().text_color(rgb(ds.danger)).child(format!(
                    "{error}. Vuelve a seleccionar una opción para reintentar."
                )))
            })
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
                                    .child(self.tr(UiText::LanguageHeading)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child(active_lang.label()),
                            ),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
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
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.text))
                            .child(self.tr(UiText::SelectionContrastHeading)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(format!("Intensidad: {:.0}%", ds.selection_contrast * 100.0)),
                    )
                    .child(
                        div().flex().gap_2().children(
                            [("Suave", 0.12_f32), ("Medio", 0.18), ("Alto", 0.28)]
                                .into_iter()
                                .map(|(label, value)| {
                                    chip_option(
                                        ds,
                                        label,
                                        (ds.selection_contrast - value).abs() < 0.001,
                                        ds.accent,
                                    )
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
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(chip_option(ds, "Sin seleccionar", false, ds.accent))
                            .child(chip_option(ds, "Seleccionado", true, ds.accent)),
                    ),
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
                                    .text_color(rgb(ds.pink))
                                    .child(self.tr(UiText::ColorThemeHeading)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child(active_theme.label()),
                            ),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        ThemePreset::ALL.into_iter().map(|theme| {
                            chip_option(
                                ds,
                                theme.label(),
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
                                    .child("EFECTO DEL CURSOR"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child(active_cursor.label()),
                            ),
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
            .into_any_element()
    }
}
