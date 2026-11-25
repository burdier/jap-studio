//! Superficie contextual plana compartida con clips e inserción del Arrangement.
use crate::ui::{
    DawApp,
    components::{
        color_picker::color_swatch,
        context_menu::{context_menu, menu_panel, menu_row, menu_separator},
    },
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_track_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.track_menu.as_ref().expect("menú abierto");
        let ds = &self.ds;
        let index = self.cursor_row;
        let close = Rc::new(cx.listener(|this, _, _, cx| {
            this.track_menu = None;
            cx.notify();
        }));
        let blank = menu.blank_space;
        let title = if blank {
            "Pistas".to_string()
        } else {
            self.track_title(index)
        };
        let mut panel = menu_panel(ds, 270.0).child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(rgb(ds.muted))
                .child(title),
        );
        for (i, (label, id)) in self.track_menu_items().iter().enumerate() {
            if !blank
                && matches!(
                    *id,
                    "action.track.group"
                        | "action.track.height.compact"
                        | "action.track.mute"
                        | "action.track.delete"
                )
            {
                panel = panel.child(menu_separator(ds));
            }
            let checked = if blank {
                false
            } else {
                match *id {
                    "action.track.mute" => self.tracks[index].muted,
                    "action.track.solo" => self.tracks[index].solo,
                    _ => false,
                }
            };
            panel = panel.child(
                menu_row(ds, *label, menu.selected == i, checked)
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(menu) = &mut this.track_menu {
                            menu.selected = i;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.run_track_menu(i, window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            );
        }
        let content = if blank {
            panel
        } else {
            let mut colors = div().p_2().flex().gap_1().flex_wrap();
            for color in std::iter::once(None).chain(ds.track_palette().into_iter().map(Some)) {
                colors = colors.child(
                    color_swatch(
                        ds,
                        color.unwrap_or(ds.track_palette()[index % 7]),
                        ds.track_color_override.get(index).copied().flatten() == color,
                        false,
                        color.is_none(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.ds.track_color_override[index] = color;
                            this.track_menu = None;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
                );
            }
            panel.child(menu_separator(ds)).child(colors)
        };
        context_menu(
            menu.position,
            close,
            content
                .id("track-context-scroll")
                .max_h(menu.max_height)
                .overflow_y_scroll(),
        )
    }
}
