//! Configuración: categorías persistentes a la izquierda, contenido desplazable a la derecha.
use crate::ui::{
    DawApp,
    components::{
        button::chip_option,
        modal::{ModalClose, modal_backdrop},
    },
    settings::SettingsCategory,
};
use gpui::{Context, IntoElement, MouseButton, Pixels, Size, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_settings_overlay(
        &self,
        viewport: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                .border_color(rgb(ds.border))
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
                        .border_color(rgb(ds.border))
                        .child(
                            div()
                                .text_lg()
                                .text_color(rgb(ds.text))
                                .child("Configuración"),
                        )
                        .child(
                            chip_option(ds, "Cerrar · Esc", false, ds.muted).on_mouse_down(
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
                                .w(px(172.0))
                                .flex_shrink_0()
                                .p_3()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .border_r_1()
                                .border_color(rgb(ds.border))
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
                                        .child(item.label())
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, window, cx| {
                                                this.settings.category = item;
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
                                        .child(category.label()),
                                )
                                .child(match category {
                                    SettingsCategory::Appearance => {
                                        self.render_appearance_settings(cx)
                                    }
                                    SettingsCategory::Tracks => self.render_track_settings(cx),
                                    SettingsCategory::Export => self.render_export_settings(cx),
                                    SettingsCategory::Shortcuts => self.render_shortcut_help(),
                                }),
                        ),
                ),
        )
    }
}
