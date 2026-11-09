//! Presentación común a clips MIDI/audio y al menú de inserción del carril.
use crate::ui::{
    DawApp,
    clip_menu::{ClipAction, MENU_HEIGHT, MENU_WIDTH, MenuPage},
    components::{
        color_picker::color_swatch,
        context_menu::{context_menu, menu_panel, menu_row, menu_separator},
    },
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_clip_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.clip_menu.as_ref().expect("menú visible");
        let ds = &self.ds;
        let close = Rc::new(cx.listener(|this, _, _, cx| {
            this.clip_menu = None;
            cx.notify();
        }));
        let title = match menu.page {
            MenuPage::Main => String::new(),
            MenuPage::Gain => format!(
                "Ganancia · {:+.1} dB",
                menu.clip_kind
                    .and_then(|kind| self.active_clip_props(kind))
                    .unwrap_or_default()
                    .gain_db
            ),
            MenuPage::Rename => "Renombrar clip".into(),
        };
        let mut panel = menu_panel(ds, MENU_WIDTH)
            .id("arrangement-context-menu")
            .max_w((menu.viewport.width - px(10.0)).max(px(0.0)))
            .max_h(
                (menu.viewport.height - menu.position.y - px(10.0))
                    .max(px(0.0))
                    .min(px(MENU_HEIGHT)),
            )
            .overflow_y_scroll()
            .when(menu.page != MenuPage::Main, |panel| {
                panel.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(title),
                )
            });
        if menu.page == MenuPage::Rename {
            panel = panel.child(
                div()
                    .mx_1()
                    .px_2()
                    .py_1()
                    .border_1()
                    .border_color(rgb(ds.accent))
                    .rounded(px(ds.radius_sm))
                    .text_xs()
                    .text_color(rgb(ds.text))
                    .bg(rgb(if menu.select_name {
                        ds.panel_hover
                    } else {
                        ds.panel_dark
                    }))
                    .overflow_hidden()
                    .child(format!("{}│", menu.rename)),
            );
        }
        let mut picker = div().flex().gap_1().px_2().py_1();
        let mut has_colors = false;
        for (index, item) in self.clip_menu_items().into_iter().enumerate() {
            if item.separator {
                panel = panel.child(menu_separator(ds));
            }
            let selected = index == menu.selected && item.enabled;
            let is_color = matches!(item.action, ClipAction::Color(_));
            let base = if let ClipAction::Color(color_index) = item.action {
                has_colors = true;
                let color = color_index.map_or_else(
                    || ds.track_color(menu.track),
                    |index| ds.clip_palette()[usize::from(index)],
                );
                color_swatch(ds, color, item.checked, selected, color_index.is_none())
            } else {
                menu_row(ds, item.label, selected, item.checked)
            };
            let mut row = base.id(("clip-menu-item", index));
            if item.enabled {
                let action = item.action;
                row = row
                    .hover(move |style| {
                        if is_color {
                            style.border_color(rgb(ds.text))
                        } else {
                            style.bg(rgb(ds.panel_hover))
                        }
                    })
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(menu) = &mut this.clip_menu {
                            if menu.selected != index {
                                menu.selected = index;
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.run_clip_menu_action(action, window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    );
            } else {
                row = row.opacity(0.4);
            }
            if is_color {
                picker = picker.child(row);
            } else {
                panel = panel.child(row);
            }
        }
        if has_colors {
            panel = panel.child(picker).child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.0))
                    .text_color(rgb(ds.muted))
                    .child("A · color de la pista"),
            );
        }
        context_menu(menu.position, close, panel)
    }
}
