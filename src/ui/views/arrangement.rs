//! Layout Arrangement y adaptación de eventos de puntero al timeline.
use super::super::{
    DawApp,
    components::splitter::{SplitDrag, vertical_splitter},
    timeline::TimelineArea,
};
use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_arrangement(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .h(px(self.tracks.len() as f32 * ds.track_header_height))
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
                                        (0..self.tracks.len())
                                            .map(|i| self.render_track_header(i, cx)),
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
                        let midi_hit = this
                            .timeline
                            .activate_midi_clip_at(event.position, this.ds.track_header_height);
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
                        this.cursor_row = track;
                    }
                    cx.notify();
                }),
            )
            .when(area == TimelineArea::Lanes, |element| {
                element
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.open_midi_clip_menu(event.position, window.viewport_size());
                            window.focus(&this.focus_handle, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_drop(
                        cx.listener(move |this, paths: &gpui::ExternalPaths, window, cx| {
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
            .on_scroll(cx.listener(move |this, event, _, cx| {
                if this.timeline.scroll(event, area) {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                self.timeline
                    .render(area, track_count, self.cursor_row, &self.ds),
            )
    }
}
