//! Render raíz y ciclo visual. El refresco de GPUI nunca avanza el transporte musical.
use super::{CenterView, DawApp, VimMode};
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
        } else if let Some(midi_clip) = &self.timeline.midi_clip {
            if self.instrument.clip_playing() {
                self.timeline.playhead = self
                    .instrument
                    .clip_beat(midi_clip.start_beat, elevate::musical::time::DEFAULT_BPM);
            } else if self.instrument.clip_finished() {
                self.status_message = "Fin del clip MIDI".into();
                self.stop_audio();
            }
        }
        if self.instrument.failed() {
            self.status_message = format!(
                "La salida de {} falló; consulta la consola",
                self.instrument.active_name()
            );
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
        let cursor_fx_alive = self.cursor_fx.advance();
        if self.player.is_some()
            || self.instrument.clip_playing()
            || self.pattern_player.is_some()
            || particles_alive
            || cursor_fx_alive
        {
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
        let palette = self
            .palette
            .is_open()
            .then(|| self.render_palette(cx).into_any_element());
        let settings_overlay = self
            .settings
            .is_visible()
            .then(|| self.render_settings_overlay(cx).into_any_element());
        let cursor_overlay = self.cursor_fx.render().map(|el| el.into_any_element());
        div()
            .id("hay")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(ds.bg))
            .text_color(rgb(ds.text))
            .font_family("monospace")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle, cx);
                    this.sync_midi_focus();
                    this.cursor_fx.on_mouse_down(event.position);
                    if f32::from(event.position.x) <= 88.0 && f32::from(event.position.y) <= 36.0 {
                        this.open_settings();
                    }
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.handle_key(event);
                this.sync_windows(cx);
                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                this.handle_key_up(event);
                cx.notify();
            }))
            .child(self.render_title_bar(mode_label.clone(), mode_color, cx))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.cursor_fx.on_mouse_move(event.position);
                if !event.dragging() {
                    this.timeline.end_clip_drag();
                    if this.cursor_fx.mode() != super::cursor_fx::CursorFxMode::Off {
                        cx.notify();
                    }
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
                }
                cx.notify();
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
            .when_some(palette, |root, palette| root.child(palette))
            .when_some(settings_overlay, |root, overlay| root.child(overlay))
            .when_some(cursor_overlay, |root, cursor| root.child(cursor))
    }
}
