//! Barras de título y estado. No abrir dispositivos ni ejecutar DSP aquí.
use super::super::{
    CenterView, DawApp, VimMode,
    components::button::{MAMEY_LOGO_COLOR, brand_logo},
};
use gpui::{Context, IntoElement, MouseButton, SharedString, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_title_bar(
        &self,
        mode_label: SharedString,
        mode_color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let view_label = match self.center_view {
            CenterView::Arrangement => "ARRANGEMENT",
            CenterView::Session => "PATTERN",
            CenterView::Mixer => "MIXER",
        };
        let audio_label = if let Some(player) = &self.player {
            format!("{} Hz", player.sample_rate)
        } else if let Some(rate) = self.vst.sample_rate() {
            format!("{} · {rate} Hz", self.vst.name)
        } else if let Some(rate) = self.instrument.sample_rate() {
            format!("{} · {rate} Hz", self.instrument.active_name())
        } else if let Some(clip) = &self.timeline.clip {
            format!("{} Hz", clip.file.sample_rate)
        } else {
            "Sin audio".into()
        };
        let engine_label = if self.player.is_some() {
            if self.pattern_loop_beats.is_some() {
                "● MIX + DRUMS"
            } else {
                "● AUDIO"
            }
        } else if self.vst_any_ready() {
            "● VST3"
        } else if self.instrument.is_running() {
            "● INSTRUMENT"
        } else {
            "○ STOP"
        };
        div()
            .h(px(ds.title_bar_height))
            .mb(px(5.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .bg(rgb(ds.panel_dark))
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.frame_border()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.open_settings();
                                    window.focus(&this.focus_handle, cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .child(brand_logo(18.0))
                            .child(
                                div()
                                    .text_color(rgb(MAMEY_LOGO_COLOR))
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("Hay"),
                            ),
                    )
                    .child(div().text_color(rgb(ds.muted)).child("/"))
                    .child(div().text_color(rgb(ds.text)).child(format!(
                            "{}{}",
                            self.project
                                .path
                                .as_ref()
                                .and_then(|p| p.file_stem())
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Sin título".into()),
                            if self.project.dirty { " *" } else { "" }
                        )))
                    .child(
                        div()
                            .ml_2()
                            .px_2()
                            .py_1()
                            .bg(rgb(mode_color))
                            .rounded(px(ds.radius_sm))
                            .text_xs()
                            .text_color(rgb(ds.bg))
                            .child(mode_label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(view_label))
                    .child(div().text_xs().text_color(rgb(ds.muted)).child(audio_label))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.success))
                            .child(engine_label),
                    ),
            )
    }

    pub(in crate::ui) fn render_status_bar(
        &self,
        mode_label: SharedString,
        mode_color: u32,
    ) -> impl IntoElement {
        let ds = &self.ds;
        div()
            .h(px(ds.status_bar_height))
            .mt(px(5.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(rgb(ds.panel_dark))
            .rounded(px(ds.radius_lg))
            .border_1()
            .border_color(rgb(ds.frame_border()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(mode_color))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(mode_label),
                    )
                    .child(
                        div()
                            .text_color(rgb(if self.controller.enabled() {
                                ds.success
                            } else {
                                ds.muted
                            }))
                            .child(if self.controller.enabled() {
                                format!(
                                    "MIDI ON → {} · octava {} ({}) · vel {} · z/x:octava · m:off",
                                    self.midi_target_label(),
                                    self.controller.octave(),
                                    self.controller.base_label(),
                                    self.controller.velocity()
                                )
                            } else {
                                "MIDI off · m:on".to_string()
                            }),
                    )
                    .children(
                        if self.vim_enabled {
                            vec![
                                "h j k l",
                                "i:insert",
                                "::cmd",
                                "a:buscar",
                                "Tab:view",
                                "+/-:zoom",
                                "R:reload",
                                "1/2/3/4:jump",
                            ]
                        } else {
                            vec![
                                "Flechas: navegar",
                                "Delete: eliminar",
                                "F11: ampliar",
                                "[+]: acciones",
                            ]
                        }
                        .into_iter()
                        .map(|label| div().text_color(rgb(ds.muted)).child(label)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(if self.mode == VimMode::Command {
                        div()
                            .text_color(rgb(ds.pink))
                            .child(format!(":{}", self.command_buffer))
                    } else {
                        div()
                            .text_color(rgb(ds.success))
                            .child(self.status_message.clone())
                    })
                    .child(div().text_color(rgb(ds.grid)).child(self.last_key.clone())),
            )
    }
}
