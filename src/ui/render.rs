//! Render raíz y ciclo visual. El refresco de GPUI nunca avanza el transporte musical.
use super::{CenterView, DawApp, VimMode};
use crate::audio::piano::PianoPlayer;
use gpui::{
    Context, IntoElement, KeyDownEvent, KeyUpEvent, MouseButton, Render, SharedString, Window, div,
    prelude::*, rgb,
};

impl Render for DawApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(player) = &self.player {
            self.timeline.playhead = Some(player.beat());
            if player.failed() || player.finished() {
                self.status_message = if player.failed() {
                    "La salida de audio falló; consulta la consola"
                } else {
                    "Fin del clip"
                }
                .into();
                self.stop_audio();
            }
        }
        if self.piano.as_ref().is_some_and(PianoPlayer::failed) {
            self.status_message = "La salida del piano falló; consulta la consola".into();
            self.stop_instrument();
        }
        if self
            .pattern_player
            .as_ref()
            .is_some_and(|player| player.failed())
        {
            self.status_message = "La salida de percusión falló; consulta la consola".into();
            self.stop_pattern();
        }
        let particles_alive = self
            .timeline
            .advance_particles(&self.ds, self.center_view == CenterView::Arrangement);
        if self.player.is_some() || self.pattern_player.is_some() || particles_alive {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
            window.request_animation_frame();
        }
        let ds = &self.ds;
        let mode_label = SharedString::from(match self.mode {
            VimMode::Normal => "NORMAL",
            VimMode::Insert => "INSERT",
            VimMode::Command => "COMMAND",
        });
        let mode_color = match self.mode {
            VimMode::Normal => ds.success,
            VimMode::Insert => ds.accent,
            VimMode::Command => ds.pink,
        };
        div()
            .id("hay")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(ds.bg))
            .text_color(rgb(ds.text))
            .font_family("monospace")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| window.focus(&this.focus_handle, cx)),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key(event);
                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                this.handle_key_up(event);
                cx.notify();
            }))
            .child(self.render_title_bar(mode_label.clone(), mode_color))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if !event.dragging() {
                    this.timeline.end_clip_drag();
                    return;
                }
                if this.center_view == CenterView::Arrangement
                    && let Some(track) =
                        this.timeline
                            .move_clip(event.position, this.ds.track_header_height, 8)
                {
                    this.cursor_row = track;
                    this.status_message = format!(
                        "Clip · pista {} · {}",
                        track + 1,
                        this.timeline.position_label()
                    );
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.release_mouse_note();
                    this.timeline.end_clip_drag();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.release_mouse_note();
                    this.timeline.end_clip_drag();
                }),
            )
            .child(self.render_transport(cx))
            .child(self.render_center(cx))
            .child(self.render_status_bar(mode_label, mode_color))
    }
}
