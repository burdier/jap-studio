use crate::ui::{DawApp, components::button::chip_option, i18n::UiText, shortcuts::ShortcutPreset};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, relative, rgb};

impl DawApp {
    pub(super) fn render_shortcut_help(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let active_preset = self.settings.shortcut_preset;
        let lang = self.settings.language;
        let groups = active_preset.help_groups(lang);
        div()
            .id("shortcut-help-scroll")
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pb_3()
                    .border_b_1()
                    .border_color(rgb(ds.border))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.cyan))
                                    .child(self.tr(UiText::ShortcutPresetHeading)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child(active_preset.label()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child(self.tr(UiText::ShortcutPresetNote)),
                    )
                    .child(div().flex().flex_wrap().gap_2().children(
                        ShortcutPreset::ALL.into_iter().map(|preset| {
                            chip_option(ds, preset.label(), preset == active_preset, ds.accent)
                                .py_1()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.set_shortcut_preset(preset);
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                )
                        }),
                    )),
            )
            .children(groups.into_iter().map(|(heading, shortcuts)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .mb_1()
                            .text_xs()
                            .text_color(rgb(ds.cyan))
                            .child(heading),
                    )
                    .children(shortcuts.into_iter().map(|(key, description)| {
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_x(px(12.0))
                            .gap_y(px(2.0))
                            .py_1()
                            .border_b_1()
                            .border_color(rgb(ds.border))
                            .child(
                                div()
                                    .w(px(176.0))
                                    .max_w(relative(0.45))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(rgb(ds.accent))
                                    .child(key),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(rgb(ds.text))
                                    .child(description),
                            )
                    }))
            }))
            .into_any_element()
    }
}
