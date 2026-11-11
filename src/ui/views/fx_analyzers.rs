//! Inspectores en Track FX: varias instancias visibles para comparar puntos de la cadena.
use crate::ui::{
    DawApp,
    components::spectrum::spectrum_panel,
    fx::{FxTarget, analyzers::nodes},
};
use gpui::{Context, Div, MouseButton, div, prelude::*, px, rgb};
impl DawApp {
    pub(in crate::ui) fn render_fx_analyzers(&self, cx: &mut Context<Self>) -> Div {
        let mut panel = div().flex().flex_col().gap_1().p_1();
        let Some(track) = self.tracks.get(self.cursor_row) else {
            return panel;
        };
        for (index, (node, enabled)) in nodes(&track.chain).into_iter().enumerate() {
            let Some(view) = self.fx.analyzers.views.get(&node.id) else {
                continue;
            };
            let target = FxTarget {
                track: track.id,
                node: Some(node.id),
            };
            let selected = self.fx.current.is_some_and(|t| t.node == Some(node.id));
            let title = div()
                .flex()
                .justify_between()
                .items_center()
                .text_size(px(10.))
                .text_color(rgb(if selected {
                    self.ds.text
                } else {
                    self.ds.muted
                }))
                .child(format!(
                    "{} · {}{}",
                    index + 1,
                    node.name,
                    if enabled { "" } else { " · Bypass" }
                ))
                .child(div().cursor_pointer().child("↺").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        let _ = this.execute_fx_at(target, "fx.spectrum.reset");
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ));
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(title)
                    .child(spectrum_panel(
                        crate::ui::spectrum::data(Some(&view.analyzer), view.mode, &self.ds),
                        view.mode,
                        cx.listener(move |this, _, _, cx| {
                            let _ = this.execute_fx_at(target, "fx.spectrum.previous");
                            cx.stop_propagation();
                            cx.notify();
                        }),
                        cx.listener(move |this, _, _, cx| {
                            let _ = this.execute_fx_at(target, "fx.spectrum.next");
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ))
                    .child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(self.ds.muted))
                            .child(if enabled {
                                view.signal_status()
                            } else {
                                "Captura en bypass · audio intacto"
                            }),
                    ),
            );
        }
        panel
    }
}
