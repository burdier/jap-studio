//! Menú plano compartido con clips y explorador, alimentado por comandos MIDI.
use crate::ui::{
    DawApp,
    components::context_menu::{context_menu, menu_panel, menu_row},
};
use gpui::{Context, IntoElement, MouseButton, div, point, prelude::*, px};

impl DawApp {
    pub(super) fn render_roll_choices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.piano_roll.choices.as_ref().expect("menu visible");
        let rows =
            div()
                .flex()
                .when(menu.colors, |el| el.flex_row().flex_wrap().gap_1())
                .when(!menu.colors, |el| el.flex_col())
                .children(menu.rows.iter().enumerate().map(
                    |(index, (label, command, checked))| {
                        let command = command.clone();
                        let row = if menu.colors {
                            let color = if index == 0 {
                                self.timeline
                                    .midi_clip
                                    .as_ref()
                                    .map_or(self.ds.accent, |clip| self.ds.track_color(clip.track))
                            } else {
                                self.ds.clip_palette()[index - 1]
                            };
                            crate::ui::components::color_picker::color_swatch(
                                &self.ds,
                                color,
                                *checked,
                                menu.selected == index,
                                index == 0,
                            )
                        } else {
                            menu_row(&self.ds, label.clone(), menu.selected == index, *checked)
                        };
                        row.on_mouse_move(cx.listener(move |this, _, _, cx| {
                            if let Some(menu) = &mut this.piano_roll.choices {
                                menu.selected = index;
                            }
                            cx.notify();
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.piano_roll.choices = None;
                                this.execute_command_with_context(&command, window, cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    },
                ));
        let panel = menu_panel(&self.ds, 310.0)
            .id("roll-choice-list")
            .max_h(px(240.0))
            .overflow_y_scroll()
            .child(rows);
        let close = std::rc::Rc::new(cx.listener(|this, _, _, cx| {
            this.piano_roll.choices = None;
            cx.notify();
        }));
        context_menu(point(px(10.0), px(64.0)), close, panel)
    }
}
