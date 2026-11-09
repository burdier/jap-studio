//! Render raíz y ciclo visual. El refresco de GPUI nunca avanza el transporte musical.
use super::{
    CenterView, DawApp, VimMode,
    components::splitter::{SplitDrag, horizontal_splitter},
};
use gpui::{
    Context, IntoElement, KeyDownEvent, KeyUpEvent, MouseButton, Render, SharedString, Window, div,
    prelude::*, px, rgb,
};

impl Render for DawApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        super::kit::sync_theme(&self.ds, cx);
        self.sync_explorer(window, cx);
        self.sync_fx(window, cx);
        self.refresh_track_layout();
        if !cx.has_active_drag() {
            self.timeline.import_preview = None;
        }
        let hw_midi = self.midi_hw.drain();
        self.poll_export();
        if !hw_midi.is_empty() {
            self.apply_midi(hw_midi);
        }
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
            if self.sampler.clip_playing() {
                self.timeline.playhead = self
                    .sampler
                    .beat(midi_clip.start_beat, elevate::musical::time::DEFAULT_BPM);
            } else if self.sampler.clip_finished() {
                self.status_message = "Fin del clip MIDI · Hay Sampler".into();
                self.stop_audio();
            } else if self.instrument.clip_playing() {
                self.timeline.playhead = self
                    .instrument
                    .clip_beat(midi_clip.start_beat, elevate::musical::time::DEFAULT_BPM);
            } else if self.instrument.clip_finished() {
                self.status_message = "Fin del clip MIDI".into();
                self.stop_audio();
            }
        }
        if self.sampler.failed() {
            self.sampler.stop();
            self.status_message = "La salida del sampler falló; vuelve a cargar el sample".into();
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
            || self.sampler.clip_playing()
            || self.pattern_player.is_some()
            || self.midi_hw.has_ports()
            || particles_alive
            || cursor_fx_alive
            || self.export_job.is_some()
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
        let settings_overlay = self.settings.is_visible().then(|| {
            self.render_settings_overlay(window.viewport_size(), cx)
                .into_any_element()
        });
        let icon_picker_overlay = self
            .toolbar_icons
            .picker_open
            .then(|| self.render_icon_picker_overlay(cx).into_any_element());
        let cursor_overlay = self.cursor_fx.render().map(|el| el.into_any_element());
        let action_form = self
            .action_form
            .as_ref()
            .filter(|form| form.window_id == window.window_handle().window_id())
            .map(|_| {
                self.render_action_form(window.viewport_size(), cx)
                    .into_any_element()
            });
        let fx_menu = self
            .fx
            .menu
            .as_ref()
            .map(|_| self.render_fx_menu(window, cx).into_any_element());
        let clip_menu = self
            .clip_menu
            .is_some()
            .then(|| self.render_clip_menu(cx).into_any_element());
        let track_menu = self
            .track_menu
            .as_ref()
            .map(|_| self.render_track_menu(cx).into_any_element());
        let track_color_menu = self
            .track_color_menu
            .is_some()
            .then(|| self.render_track_color_menu(cx).into_any_element());
        div()
            .id("hay")
            .key_context(if self.action_form.is_some() {
                "HayModal"
            } else {
                "HayWorkspace"
            })
            .on_action(
                cx.listener(|this, _: &super::kit::WorkspaceTab, window, cx| {
                    this.handle_workspace_tab(false, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &super::kit::WorkspaceBackTab, window, cx| {
                    this.handle_workspace_tab(true, window, cx)
                }),
            )
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .p(px(5.0))
            .flex()
            .flex_col()
            .bg(rgb(ds.bg))
            .text_color(rgb(ds.text))
            .font_family("monospace")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    this.track_delete_pending = false;
                    window.focus(&this.focus_handle, cx);
                    this.sync_midi_focus();
                    this.cursor_fx.on_mouse_down(event.position);
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key(event, window, cx);
                this.sync_windows(cx);
                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                this.handle_key_up(event);
                cx.notify();
            }))
            .child(self.render_title_bar(mode_label.clone(), mode_color, cx))
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    this.update_track_control_drag(event.position, event.dragging());
                    this.cursor_fx.on_mouse_move(event.position);
                    if let Some(drag) = this.split_drag {
                        if !event.dragging() {
                            this.split_drag = None;
                            cx.notify();
                            return;
                        }
                        match drag {
                            SplitDrag::Fx {
                                start_x,
                                start_width,
                            } => {
                                this.resize_fx(start_width + f32::from(event.position.x) - start_x);
                            }
                            SplitDrag::Explorer {
                                start_x,
                                start_width,
                            } => {
                                let dx = f32::from(event.position.x) - start_x;
                                this.resize_explorer(
                                    start_width + dx,
                                    f32::from(window.viewport_size().width),
                                );
                            }
                            SplitDrag::TrackHeaders {
                                start_x,
                                start_width,
                            } => {
                                let dx = f32::from(event.position.x) - start_x;
                                this.ds.track_header_width = (start_width + dx).clamp(140.0, 380.0);
                            }
                            SplitDrag::WorkspaceVertical {
                                start_y,
                                start_ratio,
                            } => {
                                let vh = f32::from(window.viewport_size().height).max(300.0);
                                let dy = f32::from(event.position.y) - start_y;
                                this.workspace_split_ratio =
                                    (start_ratio + dy / vh).clamp(0.22, 0.75);
                            }
                            SplitDrag::TransportHeight {
                                start_y,
                                start_height,
                            } => {
                                let dy = f32::from(event.position.y) - start_y;
                                this.ds.transport_height = (start_height + dy).clamp(36.0, 68.0);
                            }
                        }
                        cx.notify();
                        return;
                    }
                    let hover_changed = this.center_view == CenterView::Arrangement
                        && this.timeline.update_hover(
                            event.position,
                            event.dragging(),
                            this.ds.track_header_height,
                            this.tracks.len(),
                        );
                    if !event.dragging() {
                        this.timeline.end_clip_drag();
                        if hover_changed
                            || this.cursor_fx.mode() != super::cursor_fx::CursorFxMode::Off
                        {
                            cx.notify();
                        }
                        return;
                    }
                    if this.center_view == CenterView::Arrangement
                        && let Some(track) = this.timeline.move_clip_in_tracks(
                            event.position,
                            this.ds.track_header_height,
                            &this.tracks,
                        )
                    {
                        this.cursor_row = track;
                        this.status_message = format!(
                            "Clip · pista {} · {}",
                            track + 1,
                            this.timeline.position_label()
                        );
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_track_control_drag();
                    this.release_mouse_note();
                    this.timeline.end_clip_drag();
                    if this.split_drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_track_control_drag();
                    this.release_mouse_note();
                    this.timeline.end_clip_drag();
                    if this.split_drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(self.render_transport(cx))
            .child(
                horizontal_splitter(
                    &self.ds,
                    matches!(self.split_drag, Some(SplitDrag::TransportHeight { .. })),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                        this.split_drag = Some(SplitDrag::TransportHeight {
                            start_y: f32::from(event.position.y),
                            start_height: this.ds.transport_height,
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.ds.transport_height = 44.0;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            )
            .when(self.utility_open, |root| {
                root.child(self.render_utility(cx))
            })
            .child(self.render_center(cx))
            .child(self.render_status_bar(mode_label, mode_color))
            .when_some(clip_menu, |root, menu| root.child(menu))
            .when_some(track_menu, |root, menu| root.child(menu))
            .when_some(track_color_menu, |root, menu| root.child(menu))
            .when_some(fx_menu, |root, menu| root.child(menu))
            .when_some(palette, |root, palette| root.child(palette))
            .when_some(settings_overlay, |root, overlay| root.child(overlay))
            .when_some(icon_picker_overlay, |root, overlay| root.child(overlay))
            .when_some(action_form, |root, overlay| root.child(overlay))
            .when_some(cursor_overlay, |root, cursor| root.child(cursor))
    }
}
