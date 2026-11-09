use super::FxTarget;
use crate::ui::{
    DawApp,
    components::context_menu::{context_menu, menu_panel, menu_row, menu_separator},
};
use gpui::{
    Context, IntoElement, KeyDownEvent, MouseButton, Pixels, Point, Window, prelude::*, px,
};
use std::rc::Rc;
const ITEMS: &[(&str, &str)] = &[
    ("Add Effect…", "fx.add"),
    ("Add Serial Chain", "fx.serial"),
    ("Add Parallel Chain", "fx.parallel"),
    ("Parámetros…", "fx.parameters"),
    ("Rename…", "fx.rename"),
    ("Duplicate", "fx.duplicate"),
    ("Bypass / Enable", "fx.bypass"),
    ("Remove", "fx.remove"),
];
pub(in crate::ui) struct FxMenu {
    pub target: FxTarget,
    pub position: Point<Pixels>,
    pub selected: usize,
}
impl DawApp {
    pub(in crate::ui) fn render_fx_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let menu = self.fx.menu.as_ref().unwrap();
        let count = if menu.target.node.is_some() {
            ITEMS.len()
        } else {
            3
        };
        let mut panel = menu_panel(&self.ds, 240.0);
        for (index, (label, command)) in ITEMS.iter().take(count).enumerate() {
            if index == 3 || index == 7 {
                panel = panel.child(menu_separator(&self.ds));
            }
            let bypass = *command == "fx.bypass";
            let enabled = self.fx_node(menu.target).is_some_and(|node| node.enabled);
            let label = if bypass {
                if enabled { "Bypass" } else { "Enable" }
            } else {
                *label
            };
            panel = panel.child(
                menu_row(&self.ds, label, menu.selected == index, bypass && !enabled)
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(menu) = &mut this.fx.menu {
                            menu.selected = index;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.run_fx_menu(index);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        }
        let position = gpui::point(
            menu.position
                .x
                .min((window.viewport_size().width - px(248.0)).max(px(0.0))),
            menu.position
                .y
                .min((window.viewport_size().height - px(count as f32 * 30.0 + 24.0)).max(px(0.0))),
        );
        context_menu(
            position,
            Rc::new(cx.listener(|this, _, _, cx| {
                this.fx.menu = None;
                cx.notify();
            })),
            panel,
        )
    }
    fn run_fx_menu(&mut self, index: usize) {
        if let Some(menu) = self.fx.menu.take() {
            if let Some((_, command)) = ITEMS.get(index) {
                if let Err(error) = self.execute_fx_at(menu.target, command) {
                    self.status_message = error;
                }
            }
        }
    }
    pub(in crate::ui) fn handle_fx_menu_key(&mut self, event: &KeyDownEvent) {
        let Some(menu) = &mut self.fx.menu else {
            return;
        };
        let count = if menu.target.node.is_some() {
            ITEMS.len()
        } else {
            3
        };
        match event.keystroke.key.as_str() {
            "escape" => self.fx.menu = None,
            "up" => menu.selected = (menu.selected + count - 1) % count,
            "down" => menu.selected = (menu.selected + 1) % count,
            "enter" => {
                let index = menu.selected;
                self.run_fx_menu(index);
            }
            _ => {}
        }
    }
}
