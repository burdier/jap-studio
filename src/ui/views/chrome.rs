//! Barras de título y estado. No abrir dispositivos ni ejecutar DSP aquí.
use super::super::{
    CenterView, DawApp,
    components::button::{MAMEY_LOGO_COLOR, brand_logo, flat_control, recessed_well},
    i18n::UiText,
};
use super::menu_bar::MenuGroup;
use gpui::{
    Context, CursorStyle, Decorations, IntoElement, MouseButton, ResizeEdge, Stateful, Window,
    component::tooltip::Tooltip, div, prelude::*, px, rgb, rgba,
};

impl DawApp {
    pub(in crate::ui) fn render_title_bar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = &self.ds;
        let viewport_w = f32::from(window.viewport_size().width);
        let narrow = viewport_w < 1080.0;
        let compact = viewport_w < 860.0;
        let client = matches!(window.window_decorations(), Decorations::Client { .. });
        let view_label = match self.center_view {
            CenterView::Arrangement => "Arrangement",
            CenterView::Session => "Drums",
            CenterView::Mixer => "Mixer",
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
            self.tr(UiText::NoAudio).into()
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
            .when(client, |bar| {
                bar.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, event: &gpui::MouseDownEvent, window, _| {
                        if event.click_count == 2 {
                            window.zoom_window();
                        } else {
                            window.start_window_move();
                        }
                    }),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap(px(if compact { 6.0 } else { 12.0 }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.bar_menu = None;
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
                                    .child("Jap Studio"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .h_full()
                            .items_center()
                            .child(self.render_bar_button(MenuGroup::File, cx))
                            .child(self.render_bar_button(MenuGroup::View, cx)),
                    )
                    .child(div().text_color(rgb(ds.muted)).child("/"))
                    .child(
                        div()
                            .min_w_0()
                            .when(narrow, |el| el.max_w(px(150.0)))
                            .truncate()
                            .text_color(rgb(ds.text))
                            .child(format!(
                                "{}{}",
                                self.project
                                    .path
                                    .as_ref()
                                    .and_then(|p| p.file_stem())
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| self.tr(UiText::UntitledProject).into()),
                                if self.project.dirty { " *" } else { "" }
                            )),
                    )
                    .child({
                        let can_undo = self.history.can_undo();
                        let can_redo = self.history.can_redo();
                        let hist_open = self.history.open;
                        let hist_count = self.history.len();
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.))
                            .ml_1()
                            .child(
                                div()
                                    .id("title-undo")
                                    .px(px(5.))
                                    .py(px(1.))
                                    .rounded(px(3.))
                                    .bg(rgba((ds.text << 8) | if can_undo { 0x12 } else { 0x05 }))
                                    .hover(|b| b.bg(rgba((ds.accent << 8) | 0x26)))
                                    .text_size(px(11.))
                                    .text_color(rgb(if can_undo { ds.text } else { ds.muted }))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Deshacer (Ctrl+Z)").build(window, cx)
                                    })
                                    .child("↶")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.undo_project();
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("title-redo")
                                    .px(px(5.))
                                    .py(px(1.))
                                    .rounded(px(3.))
                                    .bg(rgba((ds.text << 8) | if can_redo { 0x12 } else { 0x05 }))
                                    .hover(|b| b.bg(rgba((ds.accent << 8) | 0x26)))
                                    .text_size(px(11.))
                                    .text_color(rgb(if can_redo { ds.text } else { ds.muted }))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Rehacer (Ctrl+Y)").build(window, cx)
                                    })
                                    .child("↷")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.redo_project();
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("title-history")
                                    .px(px(6.))
                                    .py(px(1.))
                                    .rounded(px(3.))
                                    .border_1()
                                    .border_color(if hist_open {
                                        rgb(ds.cyan)
                                    } else {
                                        rgba((ds.text << 8) | 0x18)
                                    })
                                    .bg(if hist_open {
                                        rgba((ds.cyan << 8) | 0x26)
                                    } else {
                                        rgba((ds.text << 8) | 0x0a)
                                    })
                                    .hover(|b| b.bg(rgba((ds.cyan << 8) | 0x22)))
                                    .text_size(px(10.))
                                    .text_color(rgb(if hist_open { ds.cyan } else { ds.text }))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Ver historial de cambios (Ctrl+H)")
                                            .build(window, cx)
                                    })
                                    .child(format!("🕒 {hist_count}"))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.history.open = !this.history.open;
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .when_some(
                                (!narrow)
                                    .then(|| self.project.autosave_label.clone())
                                    .flatten(),
                                |row, stamp| {
                                    row.child(
                                        div()
                                            .id("title-autosave")
                                            .ml(px(4.))
                                            .text_size(px(9.5))
                                            .text_color(rgb(ds.success))
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Autoguardado activo cada 25 s")
                                                    .build(window, cx)
                                            })
                                            .child(format!("✓ {stamp}")),
                                    )
                                },
                            )
                    })
                    .when(self.command_open, |row| {
                        row.child(
                            div()
                                .ml_2()
                                .px_2()
                                .py(px(2.0))
                                .bg(rgb(ds.pink))
                                .rounded(px(ds.radius_sm))
                                .text_xs()
                                .text_color(rgb(ds.bg))
                                .child("COMANDO"),
                        )
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(if compact { 6.0 } else { 10.0 }))
                    .children({
                        #[cfg(feature = "p2p")]
                        let shared = self.shared_project_label();
                        #[cfg(not(feature = "p2p"))]
                        let shared: Option<&str> = None;
                        shared.map(|label| div().text_xs().text_color(rgb(ds.cyan)).child(label))
                    })
                    .children(self.vst_effects.tuner.as_ref().map(|tuner| {
                        let track = self.tracks.iter().find(|t| t.id == tuner.track);
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .text_xs()
                            .child(
                                div()
                                    .max_w(px(120.))
                                    .truncate()
                                    .text_color(rgb(ds.muted))
                                    .child(track.map(|t| t.name.to_string()).unwrap_or_default()),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_color(rgb(ds.cyan))
                                    .child(tuner.reading()),
                            )
                    }))
                    .child(
                        flat_control(ds, self.tuner_enabled(), ds.cyan)
                            .id("track-tuner-toggle")
                            .px_2()
                            .text_xs()
                            .text_color(rgb(if self.tuner_enabled() {
                                ds.cyan
                            } else {
                                ds.muted
                            }))
                            .child("Afinador")
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_track_tuner(cx))),
                    )
                    .when(!compact, |bar| {
                        bar.child(div().text_xs().text_color(rgb(ds.muted)).child(view_label))
                            .child(div().text_xs().text_color(rgb(ds.muted)).child(audio_label))
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ds.success))
                            .child(engine_label),
                    )
                    .when(client, |group| {
                        group.child(
                            div()
                                .flex()
                                .items_center()
                                .child(self.window_button("–", false, cx, |_, window, _| {
                                    window.minimize_window()
                                }))
                                .child(self.window_button("□", false, cx, |_, window, _| {
                                    window.zoom_window()
                                }))
                                .child(self.window_button("×", true, cx, |this, window, cx| {
                                    if this.guard_project_close(window, cx) {
                                        window.remove_window();
                                        cx.quit();
                                    }
                                })),
                        )
                    }),
            )
    }

    fn window_button(
        &self,
        label: &'static str,
        danger: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Stateful<gpui::Div> {
        let ds = &self.ds;
        let (hover_bg, hover_text) = if danger {
            (ds.danger, ds.bg)
        } else {
            (ds.selection_background(), ds.text)
        };
        div()
            .id(label)
            .w(px(30.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(ds.radius_sm))
            .text_color(rgb(ds.muted))
            .cursor_pointer()
            .hover(move |button| button.bg(rgb(hover_bg)).text_color(rgb(hover_text)))
            .child(label)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
    }

    /// Bordes y esquinas invisibles para redimensionar con decoraciones propias.
    pub(in crate::ui) fn window_resize_edges() -> Vec<Stateful<gpui::Div>> {
        const EDGE: f32 = 4.0;
        const CORNER: f32 = 10.0;
        let handle = |index: usize, edge: ResizeEdge, cursor: CursorStyle| {
            let base = div().id(("window-resize", index)).absolute().cursor(cursor);
            let base = match edge {
                ResizeEdge::Top => base.top_0().left(px(CORNER)).right(px(CORNER)).h(px(EDGE)),
                ResizeEdge::Bottom => base
                    .bottom_0()
                    .left(px(CORNER))
                    .right(px(CORNER))
                    .h(px(EDGE)),
                ResizeEdge::Left => base.left_0().top(px(CORNER)).bottom(px(CORNER)).w(px(EDGE)),
                ResizeEdge::Right => base
                    .right_0()
                    .top(px(CORNER))
                    .bottom(px(CORNER))
                    .w(px(EDGE)),
                ResizeEdge::TopLeft => base.top_0().left_0().size(px(CORNER)),
                ResizeEdge::TopRight => base.top_0().right_0().size(px(CORNER)),
                ResizeEdge::BottomLeft => base.bottom_0().left_0().size(px(CORNER)),
                ResizeEdge::BottomRight => base.bottom_0().right_0().size(px(CORNER)),
            };
            base.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.start_window_resize(edge);
                cx.stop_propagation();
            })
        };
        vec![
            handle(0, ResizeEdge::Top, CursorStyle::ResizeUpDown),
            handle(1, ResizeEdge::Bottom, CursorStyle::ResizeUpDown),
            handle(2, ResizeEdge::Left, CursorStyle::ResizeLeftRight),
            handle(3, ResizeEdge::Right, CursorStyle::ResizeLeftRight),
            handle(4, ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight),
            handle(
                5,
                ResizeEdge::BottomRight,
                CursorStyle::ResizeUpLeftDownRight,
            ),
            handle(6, ResizeEdge::TopRight, CursorStyle::ResizeUpRightDownLeft),
            handle(
                7,
                ResizeEdge::BottomLeft,
                CursorStyle::ResizeUpRightDownLeft,
            ),
        ]
    }

    pub(in crate::ui) fn render_status_bar(&self) -> impl IntoElement {
        let ds = &self.ds;
        let active_panel = self.workspace_focus.active.label(self.center_view);
        let badge_label = if self.command_open {
            "COMANDO".to_string()
        } else {
            format!("● {active_panel}")
        };
        let badge_color = if self.command_open {
            ds.pink
        } else {
            ds.accent
        };
        recessed_well(ds)
            .h(px(ds.status_bar_height))
            .mt(px(5.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .rounded(px(ds.radius_lg))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(badge_color))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(badge_label),
                    )
                    .child(
                        div()
                            .text_color(rgb(if self.midi_hw.has_ports() || self.controller.enabled() {
                                ds.success
                            } else {
                                ds.muted
                            }))
                            .child({
                                let hw_ports = self.midi_hw.port_names();
                                let pitch_tag = self
                                    .midi_live_pitch_badge()
                                    .map(|b| format!(" · 🎹 {b}"))
                                    .unwrap_or_default();
                                if !hw_ports.is_empty() {
                                    let dev = hw_ports[0];
                                    let short_dev: String = dev.chars().take(22).collect();
                                    format!(
                                        "MIDI HW: {short_dev} → {}{pitch_tag}",
                                        self.midi_target_label(),
                                    )
                                } else if self.controller.enabled() {
                                    format!(
                                        "MIDI ON → {} · octava {} ({}) · vel {}{pitch_tag} · z/x:octava · m:off",
                                        self.midi_target_label(),
                                        self.controller.octave(),
                                        self.controller.base_label(),
                                        self.controller.velocity(),
                                    )
                                } else {
                                    format!("MIDI off · m:on{pitch_tag}")
                                }
                            }),
                    )
                    .children(
                        [
                            "Espacio: Play",
                            "Flechas: navegar",
                            "Ctrl+P / :: comandos",
                            "Ctrl+E: explorador",
                            "F11: ampliar",
                            "Delete: borrar",
                        ]
                        .into_iter()
                        .map(|label| div().text_color(rgb(ds.muted)).child(label)),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(if self.command_open {
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
