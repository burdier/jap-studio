use crate::ui::{
    DawApp,
    components::button::chip_option,
    views::tracks::{TcpControlStyle, TcpGroup, TcpLayoutPreset, TcpWidget},
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, rgb};
impl DawApp {
    pub(super) fn render_track_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let active_preset = ds.track_header.preset();
        div()
            .id("settings-tracks-scroll")
            .min_h_0()
            .p_4()
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
                                    .text_color(rgb(ds.cyan))
                                    .child("Cabeceras de pista"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.muted))
                                    .child("Distribución y controles"),
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
                            .mt_2()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("Elementos visibles por función"),
                    )
                    .children(TcpGroup::ALL.into_iter().map(|group| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .rounded(gpui::px(ds.radius_md))
                            .bg(rgb(ds.panel_dark))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(ds.text))
                                    .child(group.label()),
                            )
                            .child(
                                div().flex().flex_wrap().gap_2().children(
                                    TcpWidget::TOGGLEABLE
                                        .into_iter()
                                        .filter(move |widget| widget.group() == group)
                                        .map(|widget| {
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
                                        }),
                                ),
                            )
                    })),
            )
            .into_any_element()
    }
}
