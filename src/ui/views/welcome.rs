//! Modal de primer arranque: elige idioma y preset de atajos la primera vez que abre Jap Studio.
use crate::ui::{
    DawApp,
    components::{
        button::{brand_logo, chip_option},
        modal::{ModalClose, modal_backdrop},
    },
    i18n::{Language, UiText},
    shortcuts::ShortcutPreset,
};
use gpui::{Context, FontWeight, IntoElement, MouseButton, Window, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_welcome_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        let ds = &self.ds;
        let active_lang = self.settings.language;
        let active_preset = self.settings.shortcut_preset;
        let close: ModalClose = Rc::new(cx.listener(|this, _, _, cx| {
            this.complete_welcome();
            cx.stop_propagation();
            cx.notify();
        }));
        modal_backdrop(
            0x05070be0,
            close,
            div()
                .id("welcome-onboarding-modal")
                .w((viewport.width - px(32.0)).min(px(520.0)))
                .max_h(viewport.height - px(32.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_4()
                .p_5()
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.frame_border()))
                .rounded(px(ds.radius_lg))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(brand_logo(26.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(ds.text))
                                        .child(self.tr(UiText::WelcomeTitle)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(ds.muted))
                                        .child(self.tr(UiText::WelcomeSubtitle)),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(ds.cyan))
                                .child(self.tr(UiText::LanguageHeading)),
                        )
                        .child(div().flex().flex_wrap().gap_2().children(
                            Language::ALL.into_iter().map(|lang| {
                                chip_option(ds, lang.label(), lang == active_lang, ds.cyan)
                                    .py_2()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.settings.language = lang;
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
                                .text_color(rgb(ds.accent))
                                .child(self.tr(UiText::ShortcutPresetHeading)),
                        )
                        .child(div().flex().flex_wrap().gap_2().children(
                            ShortcutPreset::ALL.into_iter().map(|preset| {
                                chip_option(ds, preset.label(), preset == active_preset, ds.accent)
                                    .py_2()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.settings.shortcut_preset = preset;
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
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .pt_2()
                        .border_t_1()
                        .border_color(rgb(ds.border))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_xs()
                                .text_color(rgb(ds.muted))
                                .child(self.tr(UiText::WelcomeChangeLater)),
                        )
                        .child(
                            div()
                                .id("welcome-start-btn")
                                .px_4()
                                .py_2()
                                .rounded(px(ds.radius_sm))
                                .bg(rgb(ds.accent))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(ds.bg))
                                .cursor_pointer()
                                .child(self.tr(UiText::WelcomeStart))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.complete_welcome();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        ),
                ),
        )
    }
}
