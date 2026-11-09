//! Layout Arrangement y adaptación de eventos de puntero al timeline.
use super::super::{
    DawApp,
    components::splitter::{SplitDrag, vertical_splitter},
    timeline::TimelineArea,
};
use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_arrangement(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tracks.is_empty() {
            return self.render_empty_arrangement(cx).into_any_element();
        }
        let layout = self.track_layout();
        let ds = &self.ds;
        let dragging_headers = matches!(self.split_drag, Some(SplitDrag::TrackHeaders { .. }));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
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
                            .bg(rgb(ds.panel))
                            .rounded_tl(px(ds.radius_lg))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .child(div().text_xs().text_color(rgb(ds.muted)).child("TRACK"))
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(rgb(ds.muted))
                                    .child(format!("{:.0}px", ds.track_header_width)),
                            ),
                    )
                    .child(div().w(px(7.0)).h_full().flex_shrink_0())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .bg(rgb(ds.panel))
                            .rounded_tr(px(ds.radius_lg))
                            .border_1()
                            .border_color(rgb(ds.border))
                            .overflow_hidden()
                            .child(self.render_timeline_area(
                                TimelineArea::Ruler,
                                self.tracks.len(),
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .id("arrangement-tracks")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.track_scroll)
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .h(px(layout.height))
                            .child(
                                div()
                                    .w(px(ds.track_header_width))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .bg(rgb(ds.panel))
                                    .rounded_bl(px(ds.radius_lg))
                                    .border_x_1()
                                    .border_b_1()
                                    .border_color(rgb(ds.border))
                                    .overflow_hidden()
                                    .children(
                                        layout
                                            .rows
                                            .iter()
                                            .map(|row| self.render_track_header(row.track, cx)),
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
                                    .border_color(rgb(ds.border))
                                    .overflow_hidden()
                                    .child(self.render_timeline_area(
                                        TimelineArea::Lanes,
                                        self.tracks.len(),
                                        cx,
                                    )),
                            ),
                    ),
            )
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
                            .border_color(rgb(ds.border))
                            .rounded_bl(px(ds.radius_lg))
                            .child(
                                div()
                                    .text_size(px(ds.font_size_xs))
                                    .text_color(rgb(ds.muted))
                                    .child("MIN:SEG"),
                            ),
                    )
                    .child(div().w(px(7.0)).h_full().flex_shrink_0())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .border_x_1()
                            .border_b_1()
                            .border_color(rgb(ds.border))
                            .rounded_br(px(ds.radius_lg))
                            .overflow_hidden()
                            .child(self.render_timeline_area(
                                TimelineArea::TimeRuler,
                                self.tracks.len(),
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_timeline_area(
        &self,
        area: TimelineArea,
        track_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.stop_audio();
                    this.timeline.end_clip_drag();
                    if area == TimelineArea::Lanes {
                        if event.modifiers.control {
                            if this
                                .timeline
                                .activate_midi_clip_at(event.position, this.ds.track_header_height)
                            {
                                this.timeline.select_active_midi(true);
                                this.cursor_row = this.timeline.midi_clip.as_ref().unwrap().track;
                                this.status_message = format!(
                                    "{} clips MIDI seleccionados · :materialize",
                                    this.timeline.selected_midi.len()
                                );
                            }
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
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
                            this.timeline.select_active_midi(false);
                        } else {
                            this.timeline.selected_midi.clear();
                        }
                        if !midi_hit {
                            let _ = this.timeline.activate_audio_clip_at(
                                event.position,
                                this.ds.track_header_height,
                            );
                        }
                        if midi_hit && (this.piano_roll.open || event.click_count == 2) {
                            this.open_piano_roll();
                        }
                        if midi_hit && event.click_count == 2 {
                            this.focus_piano_roll(window, cx);
                            cx.stop_propagation();
                            cx.notify();
                            return;
                        }
                        this.timeline
                            .begin_clip_drag(event.position, this.ds.track_header_height);
                    }
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
                    .on_drop(
                        cx.listener(move |this, paths: &gpui::ExternalPaths, window, cx| {
                            this.timeline.import_preview = None;
                            cx.notify();
                            if paths.paths().len() != 1 {
                                this.status_message =
                                    "Arrastra un solo archivo en este incremento".into();
                                cx.notify();
                                return;
                            }
                            if let Some(track) = this.timeline.place_cursor(
                                window.mouse_position(),
                                area,
                                this.ds.track_header_height,
                                track_count,
                            ) && let Some(path) = paths.paths().first()
                            {
                                this.import_audio(
                                    path.clone(),
                                    track,
                                    this.timeline.edit_beat(),
                                    cx,
                                );
                            }
                        }),
                    )
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
                &self.ds,
            ))
    }
}
