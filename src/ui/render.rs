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
        self.validate_workspace_focus();
        self.sync_explorer(window, cx);
        self.sync_fx(window, cx);
        self.sync_vst_effects();
        self.poll_fx_analyzers();
        self.refresh_track_layout();
        if !cx.has_active_drag() {
            self.timeline.import_preview = None;
        }
        self.poll_live_midi();
        if self.vst_task.is_none()
            && let Some((track, bundle, class)) = self.vst_reloads.pop_front()
        {
            let pending = self
                .tracks
                .iter()
                .find(|item| item.id == track)
                .and_then(|item| item.instrument)
                .and_then(|id| {
                    if id == self.vst.id {
                        Some(&self.vst)
                    } else {
                        self.vst_instances.get(&id)
                    }
                })
                .is_some_and(|vst| {
                    !vst.ready() && vst.bundle.as_ref() == Some(&bundle) && vst.class == class
                });
            if pending {
                self.load_vst_class(bundle, Some(track), class, cx);
            }
        }
        if let Some(message) = self.vst.editor_message() {
            self.status_message = message;
        }
        for vst in self.vst_instances.values() {
            if let Some(message) = vst.editor_message() {
                self.status_message = message;
            }
        }
        self.poll_export();
        self.poll_audio_probe();
        self.poll_file_playback();
        self.scan_palette_plugins(cx);
        if (self.fx.editor_track.is_some_and(|id| {
            self.center_view == CenterView::Arrangement
                && self.tracks.get(self.cursor_row).is_some_and(|track| {
                    track.id == id && track.instrument == Some(self.instrument.id())
                })
        }) || self.instrument_detached())
            && !self.sampler_selected()
        {
            self.instrument.request_resources();
        }
        self.poll_piano_resources(cx);
        let piano_editor = self.fx.editor_track.is_some_and(|id| {
            self.center_view == CenterView::Arrangement
                && self.tracks.get(self.cursor_row).is_some_and(|track| {
                    track.id == id && track.instrument == Some(self.instrument.id())
                })
        });
        self.instrument.set_spectrum_enabled(piano_editor);
        if piano_editor {
            if let Some(rate) = self.instrument.sample_rate() {
                if self.piano_ui.spectrum_rate != rate || self.piano_ui.spectrum.is_none() {
                    self.piano_ui.spectrum =
                        Some(crate::audio::analyzer::SpectrumAnalyzer::new(rate));
                    self.piano_ui.spectrum_rate = rate;
                }
                if let Some(analyzer) = &mut self.piano_ui.spectrum {
                    self.instrument.drain_spectrum(analyzer);
                }
            }
        }
        if let Some(player) = &self.player {
            self.timeline.playhead = Some(player.beat());
            if player.failed() || player.finished() {
                self.status_message = if player.failed() {
                    "La salida de audio falló; consulta la consola"
                } else {
                    "Fin de la mezcla de audio"
                }
                .into();
                self.stop_audio();
            }
        } else if let Some(midi_clip) = &self.timeline.midi_clip {
            let vst = self
                .tracks
                .get(midi_clip.track)
                .and_then(|track| track.instrument)
                .and_then(|id| {
                    if id == self.vst.id {
                        Some(&self.vst)
                    } else {
                        self.vst_instances.get(&id)
                    }
                });
            if vst.is_some_and(|vst| vst.clip_playing()) {
                self.timeline.playhead =
                    vst.and_then(|vst| vst.beat(midi_clip.start_beat, self.timeline.tempo.bpm()));
            } else if vst.is_some_and(|vst| vst.clip_finished()) {
                self.status_message = "Fin del clip MIDI · VST3".into();
                self.stop_audio();
            } else if self.sampler.clip_playing() {
                self.timeline.playhead = self
                    .sampler
                    .beat(midi_clip.start_beat, self.timeline.tempo.bpm());
            } else if self.sampler.clip_finished() {
                self.status_message = "Fin del clip MIDI · Hay Sampler".into();
                self.stop_audio();
            } else if self.instrument.clip_playing() {
                self.timeline.playhead = self
                    .instrument
                    .clip_beat(midi_clip.start_beat, self.timeline.tempo.bpm());
            } else if self.instrument.clip_finished() {
                self.status_message = "Fin del clip MIDI".into();
                self.stop_audio();
            }
        }
        let cursor_animating = self.timeline.advance_visual_transport();
        if self.vst.failed() {
            self.vst.unload();
            self.status_message =
                "El instrumento VST3 o su salida falló; vuelve a cargarlo desde [+]".into();
        }
        for vst in self.vst_instances.values_mut() {
            if vst.failed() {
                vst.unload();
                self.status_message = format!(
                    "{}: salida VST3 fallida; vuelve a cargarlo desde [+]",
                    vst.name
                );
            }
        }
        self.follow_roll_playhead();
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
        let particles_alive = self
            .timeline
            .advance_particles(&self.ds, self.center_view == CenterView::Arrangement);
        let cursor_fx_alive = self.cursor_fx.advance();
        if self.player.is_some()
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.vst_any_clip_playing()
            || self.vst_any_ready()
            || self.vst_task.is_some()
            || !self.vst_reloads.is_empty()
            || !self.vst_effects.instances.is_empty()
            || (self.fx.width > 0. && !self.fx.analyzers.views.is_empty())
            || self.file_playback.is_some()
            || self.midi_hw.has_ports()
            || cursor_animating
            || particles_alive
            || cursor_fx_alive
            || self.export_job.is_some()
            || self.settings.audio.probe.is_some()
            || self.instrument.loading()
            || (piano_editor && self.instrument.is_running())
        {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
            window.request_animation_frame();
        }
        self.sync_audio_selectors(window, cx);
        let ds = &self.ds;
        let mode_label = SharedString::from(self.mode_label());
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
            .then(|| self.render_settings_overlay(window, cx).into_any_element());
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
        let general_note = self
            .general_note
            .as_ref()
            .filter(|note| note.window_id == window.window_handle().window_id())
            .map(|_| {
                self.render_general_note(window.viewport_size(), cx)
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
        let command_bar =
            (self.mode == VimMode::Command).then(|| self.render_command_bar(cx).into_any_element());
        div()
            .id("hay")
            .key_context(if self.action_form.is_some() {
                "HayModal"
            } else if self.general_note.is_some() {
                "HayGeneralNote"
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
            .bg(rgb(ds.gutter_color()))
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
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.workspace_key(event, window, cx) {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                let musical_focus = this.focus_handle.is_focused(window)
                    || this.fx.view.read(cx).focus.is_focused(window);
                if musical_focus && this.handle_performance_key(event) {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
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
                    let track_drag_changed =
                        this.update_track_reorder_drag(event.position, event.dragging());
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
                        if track_drag_changed
                            || hover_changed
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
                        if this.timeline.slipping() {
                            this.status_message = if this.timeline.slip_has_room() {
                                "Desplazando contenido · bordes y pista fijos".into()
                            } else {
                                "Sin contenido oculto: recorta un borde antes de desplazar".into()
                            };
                        } else {
                            this.status_message = format!(
                                "Clip · pista {} · {}",
                                track + 1,
                                this.timeline.position_label()
                            );
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    let track_dragged = this.track_reorder_drag.is_some();
                    this.finish_track_reorder_drag(event.position);
                    this.end_track_control_drag();
                    this.release_mouse_note();
                    this.timeline.end_clip_drag();
                    let split_dragged = this.split_drag.take().is_some();
                    if track_dragged || split_dragged {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    let track_dragged = this.track_reorder_drag.is_some();
                    this.finish_track_reorder_drag(event.position);
                    this.end_track_control_drag();
                    this.release_mouse_note();
                    this.timeline.end_clip_drag();
                    let split_dragged = this.split_drag.take().is_some();
                    if track_dragged || split_dragged {
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
            .when(
                self.utility_open && self.workspace_focus.expanded.is_none(),
                |root| root.child(self.render_utility(cx)),
            )
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
            .when_some(general_note, |root, overlay| root.child(overlay))
            .when_some(command_bar, |root, bar| root.child(bar))
            .when_some(cursor_overlay, |root, cursor| root.child(cursor))
    }
}
