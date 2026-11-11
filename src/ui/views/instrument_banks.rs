//! Fuente/preset compactos; el listado de bancos solo se construye al desplegarlo.
use crate::ui::{DawApp, components::button::chip_option};
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};

impl DawApp {
    pub(super) fn render_bank_and_preset_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        let bank_count = self.instrument.banks().len();
        let bank_label = if bank_count == 0 {
            "Descubriendo bancos…".into()
        } else {
            format!(
                "{} / {} · {}",
                self.instrument.bank_index() + 1,
                bank_count,
                self.instrument.active_name()
            )
        };
        let preset_label = self
            .instrument
            .presets()
            .get(self.instrument.preset_index())
            .map(|preset| format!("{:03}:{:03} · {}", preset.bank, preset.patch, preset.name))
            .unwrap_or_else(|| "Preset · pendiente de carga".into());
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(
                        self.piano_action("bank-prev", "◀", false, ds.accent, cx, |this| {
                            this.step_soundfont_bank(-1)
                        }),
                    )
                    .child(div().min_w_0().text_color(rgb(ds.text)).child(bank_label))
                    .child(
                        self.piano_action("bank-next", "▶", false, ds.accent, cx, |this| {
                            this.step_soundfont_bank(1)
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(
                        self.piano_action("preset-prev", "◀", false, ds.cyan, cx, |this| {
                            this.step_soundfont_preset(-1)
                        }),
                    )
                    .child(div().min_w_0().text_color(rgb(ds.cyan)).child(preset_label))
                    .child(
                        self.piano_action("preset-next", "▶", false, ds.cyan, cx, |this| {
                            this.step_soundfont_preset(1)
                        }),
                    ),
            )
    }

    pub(super) fn render_bank_pills(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .id("soundfont-bank-list")
            .max_h(px(100.))
            .overflow_y_scroll()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(
                self.instrument
                    .banks()
                    .iter()
                    .enumerate()
                    .map(|(index, bank)| {
                        chip_option(
                            ds,
                            bank.name.clone(),
                            index == self.instrument.bank_index(),
                            ds.accent,
                        )
                        .id(("sf2-bank", index))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.select_soundfont_bank(index);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                    }),
            )
    }
}
