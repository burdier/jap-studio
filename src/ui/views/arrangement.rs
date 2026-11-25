//! Layout Arrangement y adaptación de eventos de puntero al timeline.
use super::super::{
    DawApp,
    components::splitter::{SplitDrag, vertical_splitter},
    explorer::ExplorerSampleDrag,
    timeline::TimelineArea,
    track_drag::TrackReorderVisual,
};
use gpui::{
    Context, IntoElement, MouseButton, MouseDownEvent, div, prelude::*, px, relative, rgb, rgba,
};

impl DawApp {
    pub(in crate::ui) fn render_arrangement(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tracks.is_empty() {
            return self.render_empty_arrangement(cx).into_any_element();
        }
        let track_visual = self.track_reorder_visual();
        let layout = track_visual.as_ref().map_or_else(
            || {
                self.timeline
                    .layout
                    .clone()
                    .unwrap_or_else(|| self.track_layout())
            },
            |visual| visual.layout.clone(),
        );
        let track_preview = track_visual.as_ref().and_then(|visual| {
            let index = visual.source_rows.first().copied()?;
            let original = self.track_layout();
            let rows = visual
                .source_rows
                .iter()
                .enumerate()
                .filter_map(|(position, track)| {
                    let row = original.row(*track)?;
                    Some((
                        row.height,
                        self.track_title(*track),
                        self.ds.track_color(*track),
                        position == 0,
                        self.tracks[*track].parent.is_some(),
                    ))
                })
                .collect::<Vec<_>>();
            Some((
                visual.floating_top,
                visual.placeholder_top,
                self.ds.track_color(index),
                rows,
            ))
        });
        let ds = &self.ds;
        let dragging_headers = matches!(self.split_drag, Some(SplitDrag::TrackHeaders { .. }));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(self.render_arrangement_navigation(cx))
            .child(
                div()
                    .h(px(ds.timeline_bar_height))
                    .flex_shrink_0()
                    .flex()
                    .child(
                        div()
                            .w(px(ds.track_header_width))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .bg(rgb(ds.arrangement_header()))
                            .border_1()
                            .border_color(rgb(ds.frame_border()))
                            .child(self.render_wave_zoom(cx)),
                    )
                    .child(div().w(px(7.0)).h_full().flex_shrink_0().bg(rgb(ds.gutter_color())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .bg(rgb(ds.arrangement_header()))
                            .border_1()
                            .border_color(rgb(ds.frame_border()))
                            .overflow_hidden()
                            .child(self.render_timeline_area(
                                TimelineArea::Ruler,
                                self.tracks.len(),
                                None,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .id("arrangement-tracks")
                    .bg(rgb(ds.arrangement_surface()))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.track_scroll)
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .h(px(layout.height + 36.0))
                            .min_h(relative(1.0))
                            .child(
                                div()
                                    .w(px(ds.track_header_width))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .relative()
                                    .bg(rgb(ds.arrangement_header()))
                                    .rounded_bl(px(ds.radius_lg))
                                    .border_x_1()
                                    .border_b_1()
                                    .border_color(rgb(ds.frame_border()))
                                    .overflow_hidden()
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                            this.open_blank_track_menu(
                                                event.position,
                                                window.viewport_size(),
                                            );
                                            window.focus(&this.focus_handle, cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                                    .children(layout.rows.iter().map(|row| {
                                        if track_visual.as_ref().is_some_and(|visual| {
                                            visual.source_rows.contains(&row.track)
                                        }) {
                                            div()
                                                .w_full()
                                                .h(px(row.height))
                                                .flex_shrink_0()
                                                .bg(rgb(ds.panel_dark))
                                                .border_l_2()
                                                .border_color(rgb(ds.track_color(row.track)))
                                                .into_any_element()
                                        } else {
                                            self.render_tejer_header(*row, cx).into_any_element()
                                        }
                                    }))
                                    .child(
                                        div()
                                            .id("arrangement-add-track-footer")
                                            .mx(px(6.0))
                                            .my(px(6.0))
                                            .h(px(26.0))
                                            .flex_shrink_0()
                                            .rounded(px(ds.radius_md))
                                            .bg(rgb(ds.panel_dark))
                                            .border_1()
                                            .border_color(gpui::rgba(if ds.is_dark() {
                                                0xffffff10
                                            } else {
                                                0x0f172a16
                                            }))
                                            .hover(|b| {
                                                b.bg(rgb(ds.panel_hover)).text_color(rgb(ds.text))
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .gap(px(5.0))
                                            .text_size(px(ds.font_size_xs))
                                            .text_color(rgb(ds.muted))
                                            .cursor_pointer()
                                            .child("+ Agregar pista")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, window, cx| {
                                                    this.insert_audio_track_at_end();
                                                    window.focus(&this.focus_handle, cx);
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                    .when_some(
                                        track_preview,
                                        |column, (top, line, color, rows)| {
                                            column
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .left(px(0.0))
                                                        .top(px(line.clamp(
                                                            1.0,
                                                            (layout.height - 2.0).max(1.0),
                                                        ) - 1.0))
                                                        .w_full()
                                                        .h(px(2.0))
                                                        .bg(rgb(color)),
                                                )
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .left(px(0.0))
                                                        .top(px(top))
                                                        .w_full()
                                                        .flex()
                                                        .flex_col()
                                                        .children(rows.into_iter().map(|(height, title, row_color, first, child)| {
                                                            div()
                                                                .w_full()
                                                                .h(px(height))
                                                                .flex_shrink_0()
                                                                .flex()
                                                                .items_center()
                                                                .px_3()
                                                                .when(child, |row| row.pl(px(24.0)))
                                                                .bg(rgb(ds.panel_hover))
                                                                .border_1()
                                                                .border_color(rgb(row_color))
                                                                .text_size(px(ds.font_size_xs))
                                                                .text_color(rgb(ds.text))
                                                                .when(first, |row| row.child(div().mr_2().text_color(rgba((color << 8) | 0xe0)).child("↕")))
                                                                .child(title)
                                                        })),
                                                )
                                        },
                                    ),
                            )
                            .child(
                                vertical_splitter(ds, dragging_headers)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                            this.split_drag = Some(SplitDrag::TrackHeaders {
                                                start_x: f32::from(event.position.x),
                                                start_width: this.ds.track_header_width,
                                            });
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(|this, _, _, cx| {
                                            this.ds.track_header_width = 200.0;
                                            this.status_message =
                                                "Ancho de pistas restablecido (200 px)".into();
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .rounded_br(px(ds.radius_lg))
                                    .border_x_1()
                                    .border_b_1()
                                    .border_color(rgb(ds.frame_border()))
                                    .overflow_hidden()
                                    .child(self.render_timeline_area(
                                        TimelineArea::Lanes,
                                        self.tracks.len(),
                                        track_visual.as_ref(),
                                        cx,
                                    )),
                            ),
                    ),
            )
            .when_some(self.timeline.crossfade_pair(), |panel, (from, to)| {
                panel.child(self.render_crossfade_controls(from, to, cx))
            })
            .child(
                div()
                    .h(px(ds.timeline_bar_height.max(28.0)))
                    .flex_shrink_0()
                    .flex()
                    .child(
                        div()
                            .w(px(ds.track_header_width))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .px_3()
                            .bg(rgb(ds.panel_dark))
                            .border_1()
                            .border_color(rgb(ds.frame_border()))
                            .rounded_bl(px(ds.radius_lg))
                            .child(
                                if let Some((label, time, over)) = self.timeline.overlap.readout() {
                                    div().flex().flex_col().justify_center().text_size(px(ds.font_size_xs))
                                        .line_height(px(ds.font_size_xs + 1.))
                                        .text_color(rgb(if over { ds.danger } else { ds.cyan }))
                                        .child(label).child(div().text_color(rgb(ds.muted)).child(time))
                                } else {
                                    div().text_size(px(ds.font_size_xs)).text_color(rgb(ds.muted)).child("MIN:SEG")
                                },
                            ),
                    )
                    .child(div().w(px(7.0)).h_full().flex_shrink_0().bg(rgb(ds.gutter_color())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .border_x_1()
                            .border_b_1()
                            .border_color(rgb(ds.frame_border()))
                            .rounded_br(px(ds.radius_lg))
                            .overflow_hidden()
                            .child(self.render_timeline_area(
                                TimelineArea::TimeRuler,
                                self.tracks.len(),
                                None,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_crossfade_controls(
        &self,
        from: &crate::audio::AudioClip,
        to: &crate::audio::AudioClip,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let ds = &self.ds;
        use crate::ui::components::slider::param_slider;
        use std::rc::Rc;
        let (a, b) = (from.id, to.id);
        let link = to.processing.edit.crossfade;
        let center = link
            .filter(|fade| fade.from == from.crossfade_root())
            .map(|fade| fade.center);
        let mut controls = div()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py_1()
            .flex_shrink_0()
            .bg(rgb(ds.panel_dark))
            .text_xs();
        let title = if (from.start_beat, from.end_beat(self.timeline.tempo.bpm()))
            > (to.start_beat, to.end_beat(self.timeline.tempo.bpm()))
        {
            format!("{} → {}", to.name, from.name)
        } else {
            format!("{} → {}", from.name, to.name)
        };
        if let Some(value) = center {
            controls = controls.child(param_slider(
                ds,
                "Centro del cruce".into(),
                format!("{value} %"),
                f32::from(value) / 100.,
                Rc::new(cx.listener(move |s, &fraction: &f32, _, cx| {
                    s.set_crossfade(a, b, Some((fraction * 100.).round() as u8));
                    cx.notify();
                })),
                Rc::new(cx.listener(move |s, _: &(), _, cx| {
                    s.set_crossfade(a, b, Some(50));
                    cx.notify();
                })),
            ));
        }
        controls = controls.child(
            div()
                .id("crossfade-switch")
                .cursor_pointer()
                .px_2()
                .py_1()
                .bg(rgb(ds.panel_hover))
                .text_color(rgb(ds.cyan))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |s, _, _, cx| {
                        s.set_crossfade(a, b, center.is_none().then_some(50));
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .child(if center.is_some() {
                    "Quitar cruce"
                } else if link.is_some() {
                    "Reemplazar cruce"
                } else {
                    "Crear cruce"
                }),
        );
        controls.child(div().text_color(rgb(ds.muted)).child(title))
    }

    fn render_timeline_area(
        &self,
        area: TimelineArea,
        track_count: usize,
        track_visual: Option<&TrackReorderVisual>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .when(area == TimelineArea::Lanes && self.timeline.automation_mode, |el| {
                el.cursor(if self.timeline.automation_dragging() {
                    gpui::CursorStyle::ClosedHand
                } else if self.timeline.hovering_automation_point() {
                    gpui::CursorStyle::OpenHand
                } else {
                    gpui::CursorStyle::Crosshair
                })
            })
            .when(
                area == TimelineArea::Lanes
                    && !self.timeline.automation_mode
                    && self.timeline.hovering_clip_edge(),
                |el| el.cursor(gpui::CursorStyle::ResizeLeftRight),
            )
            .when(
                area == TimelineArea::Lanes
                    && !self.timeline.automation_mode
                    && !self.timeline.hovering_clip_edge()
                    && self.timeline.region_tool,
                |el| el.cursor(gpui::CursorStyle::IBeam),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if area != TimelineArea::Lanes {
                        if let Some(beat) = this.timeline.beat_at_position(event.position, area)
                            && let Err(error) = this.seek_audio(beat)
                        {
                            this.status_message = error;
                        }
                        cx.notify();
                        return;
                    }
                    this.timeline.end_clip_drag();
                    this.workspace_focus.active = crate::ui::workspace_focus::Panel::Arrangement;
                    window.focus(&this.focus_handle, cx);
                    if this.timeline.automation_mode
                        && !event.modifiers.alt
                        && !event.modifiers.control
                        && !event.modifiers.platform
                    {
                        if let Some((track_index, track_id, msg)) =
                            this.timeline.begin_automation_edit(
                                event.position,
                                event.click_count,
                                event.modifiers.shift,
                                this.ds.track_header_height,
                                &this.tracks,
                            )
                        {
                            this.select_track(track_index, false, false);
                            this.publish_track_automation(track_id);
                            this.refresh_project_dirty();
                            this.status_message = msg;
                        }
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    let over_clip_title = this.timeline.is_over_clip_title(
                        event.position,
                        this.ds.track_header_height,
                        &this.ds,
                    );
                    if (this.timeline.region_tool && !over_clip_title)
                        || (event.modifiers.alt && !event.modifiers.control && !event.modifiers.platform)
                    {
                        this.timeline
                            .begin_region_drag(event.position, event.modifiers.shift);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if !event.modifiers.control && !event.modifiers.shift && !event.modifiers.alt && !event.modifiers.platform
                        && this.begin_clip_fade(event.position, event.click_count >= 2)
                    {
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if event.modifiers.control && event.modifiers.shift && !event.modifiers.alt && !event.modifiers.platform
                        && this.timeline.begin_wave_gain(event.position, &this.ds)
                    {
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if event.modifiers.control && this.timeline.toggle_overlap_clip(event.position, &this.ds) {
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    this.timeline.overlap.clear_selection();
                    if event.modifiers.control {
                        if this
                            .timeline
                            .activate_midi_clip_at(event.position, this.ds.track_header_height)
                        {
                            this.timeline.select_active_midi(true);
                            this.cursor_row = this.timeline.midi_clip.as_ref().unwrap().track;
                            let total = this.timeline.selected_midi.len()
                                + this.timeline.selected_audio.len();
                            this.status_message = format!("{total} clips seleccionados");
                        } else if this
                            .timeline
                            .activate_audio_clip_at(event.position, this.ds.track_header_height)
                        {
                            if let Some(clip) = &this.timeline.clip {
                                let id = clip.id;
                                if this.timeline.selected_audio.contains(&id) {
                                    this.timeline.selected_audio.retain(|c| *c != id);
                                } else {
                                    this.timeline.selected_audio.push(id);
                                }
                                this.cursor_row = clip.track;
                            }
                            let total = this.timeline.selected_midi.len()
                                + this.timeline.selected_audio.len();
                            this.status_message = format!("{total} clips seleccionados");
                        } else {
                            this.timeline.begin_midi_box(event.position, true);
                        }
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if event.modifiers.shift && !event.modifiers.alt && !event.modifiers.platform {
                        if let Some(track) = this.timeline.begin_slip(event.position, this.ds.track_header_height) {
                            this.select_track(track, false, false);
                            this.status_message = if this.timeline.slip_has_room() {
                                "Desplazar contenido · bordes y pista fijos".into()
                            } else {
                                "El clip muestra toda la fuente: recorta un borde para desplazar contenido oculto".into()
                            };
                            window.focus(&this.focus_handle, cx);
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                    }
                    // Mantén el ancho del Arrangement desde el primer clic hasta soltar el clip.
                    let sample_open = this.shown_sample().is_some();
                    if !sample_open { this.timeline.sample_editor.id = None; }
                    if let Some(hit) = this
                        .timeline
                        .hit_clip_bar(event.position, this.ds.track_header_height)
                    {
                        this.handle_clip_bar_hit(hit, event.position, window.viewport_size());
                        window.focus(&this.focus_handle, cx);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    let midi_hit = this
                        .timeline
                        .activate_midi_clip_at(event.position, this.ds.track_header_height);
                    if midi_hit {
                        if event.click_count == 2 || this.timeline.midi_clip.as_ref()
                            .is_some_and(|clip| !this.timeline.selected_midi.contains(&clip.id))
                        {
                            this.timeline.selected_audio.clear();
                            this.timeline.select_active_midi(false);
                        }
                    }
                    if !midi_hit {
                        let audio_hit = this.timeline.activate_audio_clip_at(
                            event.position,
                            this.ds.track_header_height,
                        );
                        if audio_hit {
                            if sample_open && this.timeline.sample_editor.id != this.timeline.focused_audio {
                                this.show_selected_editor(crate::ui::timeline::DragKind::Audio);
                            }
                            let already_selected = this
                                .timeline
                                .clip
                                .as_ref()
                                .is_some_and(|clip| this.timeline.selected_audio.contains(&clip.id));
                            if !already_selected {
                                this.timeline.selected_midi.clear();
                                this.timeline.selected_audio =
                                    this.timeline.clip.iter().map(|c| c.id).collect();
                            }
                            this.timeline.sample_editor.pending=Some(crate::ui::timeline::DragKind::Audio);
                        } else {
                            this.timeline.begin_midi_box(event.position, false);
                        }
                    }
                    if midi_hit {
                        this.timeline.sample_editor.pending=Some(crate::ui::timeline::DragKind::Midi);
                    }
                    if midi_hit && event.click_count == 2 {
                        this.show_selected_editor(crate::ui::timeline::DragKind::Midi);
                        this.focus_piano_roll(window, cx);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    this.timeline
                        .begin_clip_drag(event.position, this.ds.track_header_height);
                    if let Some(track) = this.timeline.place_cursor(
                        event.position,
                        area,
                        this.ds.track_header_height,
                        track_count,
                    ) {
                        this.select_track(track, false, event.modifiers.shift);
                    }
                    cx.notify();
                }),
            )
            .when(area == TimelineArea::Lanes, |element| {
                element
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            if this.timeline.automation_mode
                                && let Some((track_id, msg)) =
                                    this.timeline.remove_automation_point_at(
                                        event.position,
                                        this.ds.track_header_height,
                                        &this.tracks,
                                    )
                            {
                                this.publish_track_automation(track_id);
                                this.refresh_project_dirty();
                                this.status_message = msg;
                                window.focus(&this.focus_handle, cx);
                                cx.stop_propagation();
                                cx.notify();
                                return;
                            }
                            this.open_clip_menu(event.position, window.viewport_size());
                            window.focus(&this.focus_handle, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_drag_move(cx.listener(
                        |this, event: &gpui::DragMoveEvent<gpui::ExternalPaths>, _, cx| {
                            this.timeline.preview_import(event.drag(cx).paths());
                            cx.notify();
                        },
                    ))
                    .on_drag_move(cx.listener(
                        |this, event: &gpui::DragMoveEvent<ExplorerSampleDrag>, _, cx| {
                            this.timeline.preview_import(std::slice::from_ref(&event.drag(cx).path));
                            cx.notify();
                        },
                    ))
                    .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                        this.drop_arrangement_files(paths.paths(), None, window, cx);
                    }))
                    .on_drop(cx.listener(|this, sample: &ExplorerSampleDrag, window, cx| {
                        this.drop_arrangement_files(std::slice::from_ref(&sample.path), None, window, cx);
                    }))
            })
            .on_pinch(cx.listener(move |this, event, _, cx| {
                this.timeline.pinch(event, area);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_scroll(cx.listener(move |this, event, _, cx| {
                let vertical = this.timeline.scroll(event, area);
                let mut offset = this.track_scroll.offset();
                offset.y = (offset.y + px(vertical))
                    .clamp(-this.track_scroll.max_offset().y.max(px(0.0)), px(0.0));
                this.track_scroll.set_offset(offset);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(self.timeline.render(
                area,
                track_count,
                (0..track_count).map(|i| self.track_selected(i)).collect(),
                track_visual,
                self.tracks
                    .iter()
                    .filter(|track| track.hilo.is_some())
                    .map(|track| (track.id, track.name.to_string()))
                    .collect(),
                self.tracks
                    .iter()
                    .enumerate()
                    .map(|(i, track)| {
                        let vol = self.track_levels.get(i).copied().unwrap_or(75) as f32 / 100.0;
                        let pan = self.track_pans.get(i).copied().unwrap_or(50) as f32 / 100.0;
                        (track.id, vol, pan)
                    })
                    .collect(),
                self.live_audio_record_preview(),
                &self.ds,
            ))
    }
}
