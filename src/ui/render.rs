//! Render raíz y ciclo visual. El refresco de GPUI nunca avanza el transporte musical.
use super::{
    DawApp,
    components::splitter::{SplitDrag, horizontal_splitter},
};
use gpui::{
    Context, IntoElement, KeyDownEvent, KeyUpEvent, MouseButton, Render, Window, div, prelude::*,
    px, rgb,
};

impl Render for DawApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.shown_sample().is_none() || self.center_view != super::CenterView::Arrangement {
            self.timeline.sample_editor.silence();
        }
        self.refresh_project_dirty();
        self.tick_history();
        self.tick_autosave(cx);
        super::kit::sync_theme(&self.ds, cx);
        self.validate_workspace_focus();
        self.sync_explorer(window, cx);
        self.sync_fx(window, cx);
        self.sync_vst_effects();
        self.poll_fx_analyzers();
        self.refresh_track_layout();
        self.sync_wave_pitch(window, cx);
        self.sync_overlap(window, cx);
        if !cx.has_active_drag() {
            self.timeline.import_preview = None;
        }
        self.poll_live_midi();
        self.sync_vst_editors();
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
        self.poll_export(cx);
        self.poll_audio_probe();
        self.poll_file_playback();
        self.poll_mpc_live();
        self.poll_audio_recording();
        self.scan_palette_plugins(cx);
        if (self.fx.editor_track.is_some_and(|id| {
            self.arrangement_visible()
                && self.tracks.get(self.cursor_row).is_some_and(|track| {
                    track.id == id && track.instrument == Some(self.instrument.id())
                })
        }) || self.instrument_detached())
            && !self.sampler_selected()
            && !self.synth_selected()
        {
            self.instrument.request_resources();
        }
        self.poll_piano_resources(cx);
        let piano_editor = self.fx.editor_track.is_some_and(|id| {
            self.arrangement_visible()
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
        let synth_editor = self.synth_selected()
            && (self.instrument_detached()
                || self.fx.editor_track.is_some_and(|id| {
                    self.arrangement_visible()
                        && self
                            .tracks
                            .get(self.cursor_row)
                            .is_some_and(|track| track.id == id)
                }));
        self.synth.set_spectrum_enabled(synth_editor);
        if synth_editor {
            let mut scratch = std::mem::take(&mut self.synth_ui.scope_scratch);
            self.synth.drain_scope_samples(&mut scratch);
            self.synth_ui.ingest_scope(&scratch);
            self.synth_ui.scope_scratch = scratch;
        }
        if let Some(tail) = &mut self.tail_player
            && (tail.finished() || tail.failed())
        {
            self.tail_player = None;
        }
        let mut audio_count_in_done = None;
        if let Some(player) = &mut self.player {
            player.poll_master_meter(
                self.utility_open
                    || self.center_view == super::CenterView::Mixer
                    || self.piano_roll_window.is_some(),
            );
            let counting_in = player.is_counting_in();
            let current_beat = player.beat();
            if !counting_in {
                audio_count_in_done = Some(current_beat);
            }
            if self.recording_waiting_count_in && !counting_in {
                self.recording_waiting_count_in = false;
                if self.midi_capture.is_recording() && !self.midi_capture.has_notes() {
                    self.midi_capture.start_recording();
                }
                self.status_message = format!(
                    "● Grabando MIDI en {} · pulsa ● o Espacio para finalizar",
                    self.tracks
                        .get(self.cursor_row)
                        .map_or("pista", |t| t.name.as_ref())
                );
            }
            self.timeline.playhead = Some(current_beat);
            if player.failed() || player.finished() {
                if player.finished() && self.pattern_loop_beats.is_none() {
                    self.timeline.last_stop_beat = Some(current_beat);
                }
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
        if let Some(beat) = audio_count_in_done {
            self.on_count_in_finished_for_audio(beat);
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
        if self.synth.failed() {
            self.synth.stop();
            self.status_message =
                "La salida de Hay Synth falló; vuelve a tocar para reiniciar".into();
        }
        if self.instrument.failed() {
            self.status_message = format!(
                "La salida de {} falló; consulta la consola",
                self.instrument.active_name()
            );
            self.stop_instrument();
        }
        let arrangement_visible = self.arrangement_visible();
        let particles_alive = self
            .timeline
            .advance_particles(&self.ds, arrangement_visible);
        let cursor_fx_alive = self.cursor_fx.advance();
        if self.player.is_some()
            || self.tail_player.is_some()
            || self.is_audio_recording()
            || self
                .timeline
                .sample_editor
                .audition
                .as_ref()
                .is_some_and(|a| !a.paused() && a.finished() == Ok(false))
            || self.instrument.clip_playing()
            || self.sampler.clip_playing()
            || self.synth.active_note_count() > 0
            || (synth_editor && self.synth_ui.scope_active())
            || self.vst_any_clip_playing()
            || self.vst_any_ready()
            || self.vst_task.is_some()
            || !self.vst_reloads.is_empty()
            || !self.vst_effects.instances.is_empty()
            || self.fx.analyzers.visible()
            || self.file_playback.is_some()
            || self.midi_hw.has_ports()
            || cursor_animating
            || particles_alive
            || self.timeline.snap_animating()
            || cursor_fx_alive
            || self.export_job.is_some()
            || self.settings.audio.probe.is_some()
            || self.instrument.loading()
            || (piano_editor && self.instrument.is_running())
            || self.drum_note_repeat.is_active()
            || self
                .pattern_editor
                .mpc
                .pad_flash
                .iter()
                .any(|t| t.is_some_and(|at| at.elapsed().as_secs_f32() < 0.22))
        {
            cx.on_next_frame(window, |_, _, cx| cx.notify());
            window.request_animation_frame();
        }
        self.sync_audio_selectors(window, cx);
        #[cfg(feature = "p2p")]
        let share_invite = self.render_shared_invite(window, cx);
        #[cfg(not(feature = "p2p"))]
        let share_invite: Option<gpui::Div> = None;
        let ds = &self.ds;
        let palette = self
            .palette
            .is_open()
            .then(|| self.render_palette(cx).into_any_element());
        let settings_overlay = self
            .settings
            .is_visible()
            .then(|| self.render_settings_overlay(window, cx).into_any_element());
        let welcome_overlay = self
            .settings
            .welcome_open
            .then(|| self.render_welcome_overlay(window, cx).into_any_element());
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
        let bar_menu = self
            .bar_menu
            .is_some()
            .then(|| self.render_bar_menu(window, cx));
        let track_menu = self
            .track_menu
            .as_ref()
            .map(|_| self.render_track_menu(cx).into_any_element());
        let track_color_menu = self
            .track_color_menu
            .is_some()
            .then(|| self.render_track_color_menu(cx).into_any_element());
        let drum_lane_menu = self
            .pattern_editor
            .lane_menu
            .is_some()
            .then(|| self.render_drum_lane_menu(window, cx).into_any_element());
        let command_bar = self
            .command_open
            .then(|| self.render_command_bar(cx).into_any_element());
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
            .capture_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                this.cursor_fx.on_mouse_down(event.position);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseDownEvent, window, cx| {
                    this.bar_menu = None;
                    this.history.open = false;
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                }),
            )
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.arrangement_marker_key(event, window, cx) {
                    return;
                }
                if this.explorer_visible()
                    && this.explorer.view.read(cx).search_has_focus(window, cx)
                {
                    return;
                }
                #[cfg(feature = "p2p")]
                if this.shared_invite_open() {
                    if event.keystroke.key == "escape" {
                        this.close_shared_invite(window, cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if this.bar_menu_key(event, window, cx) || this.workspace_key(event, window, cx) {
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
                if this.timeline.marker_edit.is_some() {
                    return;
                }
                if this.explorer_visible()
                    && this.explorer.view.read(cx).search_has_focus(window, cx)
                {
                    return;
                }
                this.handle_key(event, window, cx);
                this.sync_windows(cx);
                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                this.handle_key_up(event);
                cx.notify();
            }))
            .child(self.render_title_bar(window, cx))
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if event.dragging()
                        && let Some((track_id, msg)) = this.timeline.move_automation_drag(
                            event.position,
                            event.modifiers.shift,
                            this.ds.track_header_height,
                        )
                    {
                        this.publish_track_automation(track_id);
                        this.status_message = msg;
                        cx.notify();
                        return;
                    }
                    if event.dragging() && this.timeline.move_loop_drag(event.position) {
                        cx.notify();
                        return;
                    }
                    if event.dragging() && this.timeline.move_region_drag(event.position) {
                        cx.notify();
                        return;
                    }
                    if event.dragging() && this.move_clip_fade(event.position) {
                        cx.notify();
                        return;
                    }
                    if event.dragging() && this.move_wave_gain(event.position) {
                        cx.notify();
                        return;
                    }
                    this.update_tempo_drag(event);
                    if this.update_drum_offset_drag(event)
                        || this.update_mixer_drag(
                            event.position.y,
                            event.dragging(),
                            event.modifiers.shift,
                        )
                    {
                        cx.notify();
                        return;
                    }
                    this.update_track_control_drag(event.position, event.dragging());
                    let track_drag_changed =
                        this.update_track_reorder_drag(event.position, event.dragging());
                    let sampler_visible = this.arrangement_visible()
                        && !this.instrument_detached()
                        && this.fx.editor_track.is_some_and(|id| {
                            this.tracks
                                .iter()
                                .find(|t| t.id == id)
                                .and_then(|t| t.instrument)
                                .is_some_and(|inst| this.is_sampler_id(inst))
                        });
                    if !sampler_visible {
                        this.sampler_ui.panel_bounds.set(None);
                        this.sampler_ui.drop_hover = None;
                    } else if event.dragging()
                        && let Some(kind) = this.timeline.active_clip_drag_kind()
                        && this
                            .sampler_ui
                            .panel_bounds
                            .get()
                            .is_some_and(|b| b.contains(&event.position))
                    {
                        this.sampler_ui.drop_hover = Some(kind);
                    } else {
                        this.sampler_ui.drop_hover = None;
                    }
                    this.update_sampler_wave_drag(event.position, event.dragging(), cx);
                    if let Some((id, origin, fraction)) = this.sampler_ui.drag {
                        if event.dragging() {
                            let delta = f32::from(origin - event.position.y)
                                / if event.modifiers.shift { 1600. } else { 160. };
                            this.set_sample_param(
                                id,
                                elevate::devices::sampler::SamplerParams::value_at(
                                    id,
                                    fraction + delta,
                                ),
                                cx,
                            );
                        } else {
                            this.finish_sample_drag(cx);
                        }
                    }
                    if let Some((id, origin, fraction)) = this.piano_ui.drag {
                        if event.dragging() {
                            let delta = f32::from(origin - event.position.y)
                                / if event.modifiers.shift { 1600. } else { 160. };
                            let spec = crate::instruments::piano_params::spec_of(id);
                            this.set_piano_param(id, spec.from_fraction(fraction + delta));
                        } else {
                            this.piano_ui.drag = None;
                        }
                        cx.notify();
                    }
                    if this.update_synth_drag(
                        event.position,
                        event.dragging(),
                        event.modifiers.shift,
                    ) {
                        cx.notify();
                        return;
                    }
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
                            SplitDrag::Tejer {
                                start_x,
                                start_width,
                                max_width,
                            } => {
                                let dx = f32::from(event.position.x) - start_x;
                                this.fx.tejer_width = Some(
                                    (start_width + dx).clamp(210.0_f32.min(max_width), max_width),
                                );
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
                            SplitDrag::DrumWorkspace {
                                start_x,
                                start_ratio,
                            } => {
                                let width = (f32::from(window.viewport_size().width)
                                    - this.explorer.width)
                                    .max(300.0);
                                let dx = f32::from(event.position.x) - start_x;
                                this.pattern_editor.workspace_ratio =
                                    Some((start_ratio + dx / width).clamp(0.28, 0.72));
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
                    let hover_changed = this.arrangement_visible()
                        && (this.timeline.update_hover(
                            event.position,
                            event.dragging(),
                            this.ds.track_header_height,
                            this.tracks.len(),
                        ) || this.timeline.update_automation_hover(
                            event.position,
                            this.ds.track_header_height,
                            &this.tracks,
                        ));
                    if !event.dragging() {
                        let dropped_to_sampler = this.sampler_ui.drop_hover.take().filter(|_| {
                            this.sampler_ui
                                .panel_bounds
                                .get()
                                .is_some_and(|b| b.contains(&event.position))
                        });
                        if let Some(kind) = dropped_to_sampler {
                            this.timeline.restore_clip_drag_origin();
                            this.timeline.end_clip_drag();
                            this.drop_arrangement_clip_into_sampler(kind, cx);
                            cx.notify();
                            return;
                        }
                        let clip_dragged = this.finish_arrangement_drag();
                        if track_drag_changed
                            || clip_dragged
                            || hover_changed
                            || this.cursor_fx.mode() != super::cursor_fx::CursorFxMode::Off
                        {
                            cx.notify();
                        }
                        return;
                    }
                    if this.arrangement_visible()
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
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    if let Some(track_id) = this.timeline.end_automation_drag() {
                        this.publish_track_automation(track_id);
                        this.refresh_project_dirty();
                        cx.notify();
                        return;
                    }
                    if this.finish_arrangement_loop(event.position)
                        || this.finish_arrangement_region(event.position)
                    {
                        cx.notify();
                        return;
                    }
                    let tempo_dragged = this.end_tempo_drag(window, cx);
                    let track_dragged = this.track_reorder_drag.is_some();
                    this.finish_track_reorder_drag(event.position);
                    let control_dragged = this.end_track_control_drag();
                    let mixer_dragged = this.mixer_drag.take().is_some();
                    let offset_dragged = this.finish_drum_offset_drag();
                    this.finish_sample_drag(cx);
                    this.finish_synth_drag();
                    this.timeline.sample_editor.anchor = None;
                    this.piano_ui.drag = None;
                    this.release_mouse_note();
                    this.release_chord_audition();
                    this.mpc_pad_release();
                    let dropped_to_sampler = this.sampler_ui.drop_hover.take().filter(|_| {
                        this.sampler_ui
                            .panel_bounds
                            .get()
                            .is_some_and(|b| b.contains(&event.position))
                    });
                    if let Some(kind) = dropped_to_sampler {
                        this.timeline.restore_clip_drag_origin();
                        this.timeline.end_clip_drag();
                        this.drop_arrangement_clip_into_sampler(kind, cx);
                        cx.notify();
                        return;
                    }
                    let clip_dragged = this.finish_arrangement_drag();
                    let split_dragged = this.split_drag.take().is_some();
                    if tempo_dragged
                        || track_dragged
                        || split_dragged
                        || control_dragged
                        || mixer_dragged
                        || offset_dragged
                        || clip_dragged
                    {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    if let Some(track_id) = this.timeline.end_automation_drag() {
                        this.publish_track_automation(track_id);
                        this.refresh_project_dirty();
                        cx.notify();
                        return;
                    }
                    if this.finish_arrangement_loop(event.position)
                        || this.finish_arrangement_region(event.position)
                    {
                        cx.notify();
                        return;
                    }
                    let tempo_dragged = this.end_tempo_drag(window, cx);
                    let track_dragged = this.track_reorder_drag.is_some();
                    this.finish_track_reorder_drag(event.position);
                    let control_dragged = this.end_track_control_drag();
                    let mixer_dragged = this.mixer_drag.take().is_some();
                    let offset_dragged = this.finish_drum_offset_drag();
                    this.finish_sample_drag(cx);
                    this.finish_synth_drag();
                    this.timeline.sample_editor.anchor = None;
                    this.piano_ui.drag = None;
                    this.release_mouse_note();
                    this.release_chord_audition();
                    this.mpc_pad_release();
                    let dropped_to_sampler = this.sampler_ui.drop_hover.take().filter(|_| {
                        this.sampler_ui
                            .panel_bounds
                            .get()
                            .is_some_and(|b| b.contains(&event.position))
                    });
                    if let Some(kind) = dropped_to_sampler {
                        this.timeline.restore_clip_drag_origin();
                        this.timeline.end_clip_drag();
                        this.drop_arrangement_clip_into_sampler(kind, cx);
                        cx.notify();
                        return;
                    }
                    let clip_dragged = this.finish_arrangement_drag();
                    let split_dragged = this.split_drag.take().is_some();
                    if tempo_dragged
                        || track_dragged
                        || split_dragged
                        || control_dragged
                        || mixer_dragged
                        || offset_dragged
                        || clip_dragged
                    {
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
            .when_some(self.render_recovery_banner(cx), |root, banner| {
                root.child(banner)
            })
            .child(self.render_center(cx))
            .child(self.render_status_bar())
            .when_some(self.render_history_panel(cx), |root, panel| {
                root.child(panel)
            })
            .when_some(clip_menu, |root, menu| root.child(menu))
            .when_some(track_menu, |root, menu| root.child(menu))
            .when_some(track_color_menu, |root, menu| root.child(menu))
            .when_some(drum_lane_menu, |root, menu| root.child(menu))
            .when_some(fx_menu, |root, menu| root.child(menu))
            .when_some(bar_menu, |root, menu| root.child(menu))
            .when_some(palette, |root, palette| root.child(palette))
            .when_some(settings_overlay, |root, overlay| root.child(overlay))
            .when_some(welcome_overlay, |root, overlay| root.child(overlay))
            .when_some(icon_picker_overlay, |root, overlay| root.child(overlay))
            .when_some(action_form, |root, overlay| root.child(overlay))
            .when_some(
                self.render_arrangement_marker_editor(cx),
                |root, overlay| root.child(overlay),
            )
            .when_some(general_note, |root, overlay| root.child(overlay))
            .when_some(share_invite, |root, overlay| root.child(overlay))
            .when_some(command_bar, |root, bar| root.child(bar))
            .when_some(cursor_overlay, |root, cursor| root.child(cursor))
            .when(
                matches!(
                    window.window_decorations(),
                    gpui::Decorations::Client { .. }
                ),
                |root| root.children(Self::window_resize_edges()),
            )
    }
}
