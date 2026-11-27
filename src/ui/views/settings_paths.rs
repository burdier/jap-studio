//! Configuración de rutas multiplataforma (VST3, VST2, CLAP, SoundFonts, Presets, Samples, Kits).
use crate::ui::{DawApp, components::button::chip_option};
use elevate::paths::{AppPaths, PathGroup};
use gpui::{Context, FontWeight, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_paths_settings(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ds = &self.ds;
        let config_file = AppPaths::config_file()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "paths.profile".into());

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .pb_2()
            .border_b_1()
            .border_color(rgb(ds.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.muted))
                            .child("Todas las rutas de plugins, bancos y librerías se guardan aquí por sistema operativo (Linux / Windows / macOS)."),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family("monospace")
                            .text_color(rgb(ds.cyan))
                            .truncate()
                            .child(format!("Archivo: {config_file}")),
                    ),
            )
            .child(
                chip_option(ds, "Restablecer todo", false, ds.muted)
                    .id("paths-reset-all")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.reset_all_config_paths();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );

        let err_banner = self.settings.paths_save_error.clone().map(|err| {
            div()
                .px_3()
                .py_2()
                .rounded(px(ds.radius_sm))
                .bg(rgb(ds.panel))
                .border_1()
                .border_color(rgb(ds.accent))
                .text_xs()
                .text_color(rgb(ds.accent))
                .child(err)
        });

        let groups = PathGroup::ALL.into_iter().map(|group| {
            let paths = self.settings.paths.paths(group).to_vec();
            let count = paths.len();
            div()
                .p_3()
                .rounded(px(ds.radius_md))
                .bg(rgb(ds.panel))
                .border_1()
                .border_color(rgb(ds.border))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(ds.text))
                                        .child(format!("{} ({count})", group.label())),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(rgb(ds.muted))
                                        .child(group.hint()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    chip_option(ds, "+ Carpeta…", true, ds.cyan)
                                        .id(("paths-add", group as usize))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.choose_config_path_folder(group, cx);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    chip_option(ds, "Defecto", false, ds.muted)
                                        .id(("paths-reset", group as usize))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.reset_config_path_group(group);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        ),
                )
                .child(if paths.is_empty() {
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child("Sin carpetas configuradas")
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(paths.into_iter().enumerate().map(|(idx, path)| {
                            let exists = path.exists();
                            let display = path.display().to_string();
                            div()
                                .px_2()
                                .py(px(4.))
                                .rounded(px(ds.radius_sm))
                                .bg(rgb(ds.panel_dark))
                                .border_1()
                                .border_color(rgb(ds.border))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .w(px(7.))
                                                .h(px(7.))
                                                .flex_shrink_0()
                                                .rounded(px(4.))
                                                .bg(rgb(if exists {
                                                    ds.success
                                                } else {
                                                    ds.muted
                                                })),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .font_family("monospace")
                                                .text_size(px(10.5))
                                                .text_color(rgb(if exists {
                                                    ds.text
                                                } else {
                                                    ds.muted
                                                }))
                                                .truncate()
                                                .child(display),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(("paths-del", (group as usize) * 1000 + idx))
                                        .flex_shrink_0()
                                        .px(px(6.))
                                        .py(px(1.))
                                        .rounded(px(3.))
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(rgb(ds.muted))
                                        .child("✕")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.remove_config_path(group, idx);
                                                cx.stop_propagation();
                                                cx.notify();
                                            }),
                                        ),
                                )
                        }))
                        .into_any_element()
                })
        });

        div()
            .id("settings-paths-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .pb_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(header)
            .when_some(err_banner, |col, b| col.child(b))
            .children(groups)
            .into_any_element()
    }
}
