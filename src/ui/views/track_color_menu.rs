//! Menú contextual de color de pista (clic derecho sobre la cabecera).
use super::super::{
    DawApp,
    components::{
        color_picker::color_swatch,
        context_menu::{context_menu, menu_panel},
    },
};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, rgb};
use std::rc::Rc;

impl DawApp {
    pub(in crate::ui) fn render_track_color_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (position, track) = self.track_color_menu.expect("menú visible");
        let ds = &self.ds;
        let close = Rc::new(cx.listener(|this, _, _, cx| {
            this.track_color_menu = None;
            cx.notify();
        }));
        let current = ds.track_color_override[track];
        let swatch = |color: Option<u32>, cx: &mut Context<Self>| {
            let selected = current == color;
            color_swatch(
                ds,
                color.unwrap_or(ds.track_palette()[track % 7]),
                selected,
                false,
                color.is_none(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.ds.track_color_override[track] = color;
                    this.track_color_menu = None;
                    this.status_message = format!("{}: color actualizado", this.tracks[track].name);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
        };
        let mut row = div().flex().flex_wrap().gap_1();
        row = row.child(swatch(None, cx));
        for color in ds.track_palette() {
            row = row.child(swatch(Some(color), cx));
        }
        context_menu(
            position,
            close,
            menu_panel(ds, 248.0)
                .p_2()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(ds.muted))
                        .child(format!("Color · {}", self.tracks[track].name)),
                )
                .child(row),
        )
    }
}
