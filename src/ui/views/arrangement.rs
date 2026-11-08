//! Layout Arrangement y adaptación de eventos de puntero al timeline.
use super::super::{DawApp, timeline::TimelineArea};
use super::tracks::TRACK_NAMES;
use gpui::{Context, IntoElement, MouseButton, MouseDownEvent, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_arrangement(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ds = &self.ds;
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
                    .bg(rgb(ds.panel))
                    .child(
                        div()
                            .w(px(ds.track_header_width))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .px_3()
                            .border_b_1()
                            .border_r_1()
                            .border_color(rgb(ds.border))
                            .child(div().text_xs().text_color(rgb(ds.muted)).child("TRACK")),
                    )
                    .child(self.render_timeline_area(TimelineArea::Ruler, TRACK_NAMES.len(), cx)),
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
                            .h(px(TRACK_NAMES.len() as f32 * ds.track_header_height))
                            .child(
                                div()
                                    .w(px(ds.track_header_width))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .bg(rgb(ds.panel))
                                    .border_r_1()
                                    .border_color(rgb(ds.border))
                                    .children(
                                        (0..TRACK_NAMES.len())
                                            .map(|i| self.render_track_header(i, cx)),
                                    ),
                            )
                            .child(self.render_timeline_area(
                                TimelineArea::Lanes,
                                TRACK_NAMES.len(),
                                cx,
                            )),
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
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.stop_audio();
                    this.timeline.end_clip_drag();
                    if area == TimelineArea::Lanes {
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
                element.on_drop(cx.listener(
                    move |this, paths: &gpui::ExternalPaths, window, cx| {
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
                            this.import_audio(path.clone(), track, this.timeline.edit_beat(), cx);
                        }
                    },
                ))
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
