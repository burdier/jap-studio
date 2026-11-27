//! Configuración: categorías persistentes a la izquierda, contenido desplazable a la derecha.
use crate::ui::{
    DawApp,
    components::{
        button::chip_option,
        modal::{ModalClose, modal_backdrop},
    },
    settings::SettingsCategory,
};
use gpui::{Context, IntoElement, MouseButton, Window, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_settings_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        let ds = &self.ds;
        let category = self.settings.category;
        let close: ModalClose = Rc::new(cx.listener(|this, _, _, cx| {
            this.close_settings_overlay();
            cx.stop_propagation();
            cx.notify();
        }));
        modal_backdrop(
            0x05070bd9,
            close,
            div()
                .w((viewport.width - px(32.0)).min(px(980.0)))
                .h((viewport.height - px(32.0)).min(px(680.0)))
                .flex()
                .flex_col()
                .overflow_hidden()
                .bg(rgb(ds.panel_dark))
                .border_1()
                .border_color(rgb(ds.frame_border()))
                .rounded(px(ds.radius_lg))
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_5()
                        .py_3()
                        .border_b_1()
                        .border_color(rgb(ds.frame_border()))
                        .child(
                            div()
                                .text_lg()
                                .text_color(rgb(ds.text))
                                .child(self.tr("settings.title")),
                        )
                        .child(
                            chip_option(ds, self.tr("settings.close_esc"), false, ds.muted)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.close_settings_overlay();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(
                            div()
                                .id("settings-sidebar")
                                .w(if viewport.width < px(720.0) {
                                    px(124.0)
                                } else {
                                    px(172.0)
                                })
                                .flex_shrink_0()
                                .overflow_y_scroll()
                                .p_3()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .border_r_1()
                                .border_color(rgb(ds.frame_border()))
                                .bg(rgb(ds.panel))
                                .children(SettingsCategory::ALL.into_iter().map(|item| {
                                    div()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(ds.radius_sm))
                                        .cursor_pointer()
                                        .bg(rgb(if category == item {
                                            ds.selection_background()
                                        } else {
                                            ds.panel
                                        }))
                                        .text_sm()
                                        .text_color(rgb(if category == item {
                                            ds.text
                                        } else {
                                            ds.muted
                                        }))
                                        .child(item.label_in(self.settings.language))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, window, cx| {
                                                this.settings.audio.probe = None;
                                                this.settings.audio.focus_request = None;
                                                this.settings.audio.selectors.clear();
                                                this.settings.category = item;
                                                if item == SettingsCategory::Audio {
                                                    this.refresh_audio_devices(cx);
                                                }
                                                this.settings.export.editing = None;
                                                window.focus(&this.focus_handle, cx);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        )
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .px_4()
                                        .pt_4()
                                        .pb_2()
                                        .flex_shrink_0()
                                        .text_lg()
                                        .text_color(rgb(ds.text))
                                        .child(category.label_in(self.settings.language)),
                                )
                                .child(match category {
                                    SettingsCategory::Appearance => {
                                        self.render_appearance_settings(cx)
                                    }
                                    SettingsCategory::Audio => {
                                        self.render_audio_settings(window, cx)
                                    }
                                    SettingsCategory::Paths => self.render_paths_settings(cx),
                                    SettingsCategory::Tracks => self.render_track_settings(cx),
                                    SettingsCategory::Export => self.render_export_settings(cx),
                                    SettingsCategory::MidiMap => self.render_midi_settings(cx),
                                    SettingsCategory::Shortcuts => self.render_shortcut_help(cx),
                                }),
                        ),
                ),
        )
    }
}
