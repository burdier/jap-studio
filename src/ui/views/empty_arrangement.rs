//! Estado vacío del Arrangement; abre el mismo buscador que :add y la tecla a.
use crate::ui::{DawApp, icons::render_svg_icon};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_empty_arrangement(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .flex_1()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(rgb(ds.panel_dark))
            .border_1()
            .border_color(rgb(ds.frame_border()))
            .rounded(px(ds.radius_lg))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .p_5()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.open_palette();
                            window.focus(&this.focus_handle, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .p_4()
                            .border_1()
                            .border_color(rgb(ds.accent))
                            .rounded(px(ds.radius_lg))
                            .child(render_svg_icon(
                                include_bytes!("../../../assets/icons/toolbar/plus.svg"),
                                40.0,
                                ds.accent,
                            )),
                    )
                    .child(
                        div()
                            .text_lg()
                            .text_color(rgb(ds.text))
                            .child("Añadir una pista"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(ds.muted))
                            .child("Elige Insertar pista o un instrumento · A"),
                    ),
            )
    }
}
