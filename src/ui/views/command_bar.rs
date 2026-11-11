//! Minibuffer inline al pie de la ventana principal.
use crate::ui::DawApp;
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_command_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let completions = self.cmd_completions();
        let total = completions.len();
        let selected = self.cmd_selected.filter(|index| *index < total);
        const VISIBLE: usize = 8;
        let start = if total <= VISIBLE {
            0
        } else {
            selected
                .unwrap_or(0)
                .saturating_sub(VISIBLE / 2)
                .min(total - VISIBLE)
        };
        let end = (start + VISIBLE).min(total);

        div()
            .absolute()
            .bottom(px(5.0))
            .left(px(5.0))
            .right(px(5.0))
            .flex()
            .flex_col()
            .when(total > 0, |bar| {
                bar.child(
                    div()
                        .flex()
                        .flex_col()
                        .border_t_1()
                        .border_color(rgb(ds.pink))
                        .bg(rgb(ds.panel_dark))
                        .children(completions[start..end].iter().enumerate().map(|(i, (cmd, desc))| {
                            let absolute_index = start + i;
                            let active = selected == Some(absolute_index);
                            let command = cmd.clone();
                            let display_command = format!(":{command}");
                            div()
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .px(px(10.0))
                                .bg(rgb(if active { ds.selection_background() } else { ds.panel_dark }))
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                    this.command_buffer = command.to_string();
                                    this.cmd_selected = None;
                                    cx.stop_propagation();
                                    cx.notify();
                                }))
                                .child(div()
                                    .text_size(px(11.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(if active { ds.pink } else { ds.text }))
                                    .child(display_command))
                                .child(div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(10.5))
                                    .text_color(rgb(ds.muted))
                                    .child(desc.clone()))
                                .when(total > VISIBLE && i == 0 && start > 0, |row| {
                                    row.child(div().text_size(px(9.0)).text_color(rgb(ds.muted))
                                        .child(format!("▲ {start}")))
                                })
                                .when(total > VISIBLE && i + 1 == end - start && end < total, |row| {
                                    row.child(div().text_size(px(9.0)).text_color(rgb(ds.muted))
                                        .child(format!("▼ {}", total - end)))
                                })
                        })),
                )
            })
            .child(
                div()
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(10.0))
                    .bg(rgb(ds.panel_dark))
                    .border_t_1()
                    .border_color(rgb(ds.pink))
                    .child(div().text_size(px(13.0)).font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(ds.pink)).child(":"))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(13.0))
                        .text_color(rgb(ds.text)).child(format!("{}▌", self.command_buffer)))
                    .child(div().text_size(px(10.0)).text_color(rgb(ds.muted))
                        .child(if total == 0 {
                            "sin coincidencias · Enter ejecuta el texto".to_string()
                        } else {
                            format!("↑↓ elegir · Tab/clic completar · Enter ejecutar · Esc cancelar  [{}/{}]",
                                selected.map_or(0, |index| index + 1), total)
                        })),
            )
    }
}
