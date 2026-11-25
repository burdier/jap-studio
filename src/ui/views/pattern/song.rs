//! Matriz de ocurrencias de Canción. Cada celda conserva su posición en ticks.
use crate::ui::{
    DawApp,
    components::button::{flat_control, led_dot},
    pattern_editor::{PatternAction, SONG_BARS_PER_PAGE},
};
use elevate::musical::{
    drums::{DrumPlaybackSource, DrumSongCell},
    time::{PPQ, Tick},
};
use gpui::{
    Context, Div, FontWeight, MouseButton, Stateful, component::tooltip::Tooltip, div, prelude::*,
    px, rgb, rgba,
};

const LABEL_WIDTH: f32 = 118.0;

impl DawApp {
    pub(super) fn render_drum_song(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let session = &self.pattern_editor.session;
        let meter = self.timeline.meter;
        let bar = meter.ticks_per_bar();
        let page_start = self.pattern_editor.song_page_start(meter);
        let page_end = page_start.saturating_add(SONG_BARS_PER_PAGE * bar);
        let mut columns: Vec<_> = (0..SONG_BARS_PER_PAGE)
            .map(|index| Tick(page_start.saturating_add(index * bar)))
            .collect();
        columns.extend(session.song.iter().filter_map(|entry| {
            (entry.at.0 >= page_start && entry.at.0 < page_end && entry.at.0 % bar != 0)
                .then_some(entry.at)
        }));
        columns.sort_unstable();
        columns.dedup();
        let song_end = session.song_end().ok().map_or(0, |end| end.0);
        let last_page = song_end / (SONG_BARS_PER_PAGE * bar);
        let now = if session.source == DrumPlaybackSource::Song {
            self.pattern_loop_beats
                .filter(|length| length.is_finite() && *length > 0.0)
                .and_then(|length| {
                    self.player
                        .as_ref()
                        .map(|player| (player.beat().rem_euclid(length) * PPQ as f64) as i64)
                })
        } else {
            None
        };
        let dark = ds.is_dark();
        let header = div()
            .flex()
            .w_full()
            .min_w_0()
            .gap(px(4.0))
            .items_center()
            .flex_shrink_0()
            .child(
                div()
                    .w(px(LABEL_WIDTH))
                    .flex_shrink_0()
                    .px(px(6.0))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(ds.muted))
                    .child("CANCIÓN"),
            )
            .children(columns.iter().enumerate().map(|(index, &at)| {
                let next = columns.get(index + 1).map_or(page_end, |next| next.0);
                let playing = now.is_some_and(|tick| tick >= at.0 && tick < next);
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(18.0))
                    .font_family("monospace")
                    .text_size(px(10.0))
                    .text_center()
                    .text_color(rgb(if playing { ds.cyan } else { ds.muted }))
                    .child(if at.0 % bar == 0 {
                        (at.0 / bar + 1).to_string()
                    } else {
                        meter.position_label(at)
                    })
            }));
        div()
            .id("drum-song")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .max_h(px(156.0))
            .px(px(10.0))
            .py(px(8.0))
            .gap(px(6.0))
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgba(if ds.is_dark() { 0xffffff0d } else { 0x0f172a12 }))
            .text_xs()
            .child(
                div()
                    .id("drum-song-cells")
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_w_0()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap(px(4.0))
                    .child(header)
                    .children(session.patterns.iter().map(|pattern| {
                        let id = pattern.id;
                        let can_delete = session.patterns.len() > 1;
                        div()
                            .flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .flex_shrink_0()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(2.0))
                                    .flex_shrink_0()
                                    .w(px(LABEL_WIDTH))
                                    .child(
                                        self.drum_action_chip(
                                            pattern.name.clone(),
                                            id == session.active,
                                            PatternAction::Select(id),
                                            cx,
                                        )
                                        .justify_start()
                                        .flex_1()
                                        .min_w_0(),
                                    )
                                    .when(can_delete, |row| {
                                        row.child(
                                            flat_control(ds, false, ds.danger)
                                                .id(("drum-song-del-pat", id.0 as usize))
                                                .w(px(20.0))
                                                .h(px(22.0))
                                                .rounded(px(ds.radius_sm))
                                                .text_size(px(11.0))
                                                .text_color(rgb(ds.danger))
                                                .tooltip(|window, cx| {
                                                    Tooltip::new("Eliminar patrón")
                                                        .build(window, cx)
                                                })
                                                .child("×")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.apply_pattern_action(
                                                            PatternAction::DeletePattern(id),
                                                        );
                                                        cx.stop_propagation();
                                                        cx.notify();
                                                    }),
                                                ),
                                        )
                                    }),
                            )
                            .children(columns.iter().enumerate().map(|(col_idx, &at)| {
                                let cell = DrumSongCell { at, pattern: id };
                                let occupied = session
                                    .song
                                    .binary_search_by_key(&(at, id), |entry| {
                                        (entry.at, entry.pattern)
                                    })
                                    .is_ok();
                                let next = columns.get(col_idx + 1).map_or(page_end, |next| next.0);
                                let playing = now.is_some_and(|tick| tick >= at.0 && tick < next);
                                let alt_group = (col_idx / 4) % 2 == 1;
                                let cell_bg = if playing {
                                    rgba((ds.cyan << 8) | if dark { 0x16 } else { 0x18 })
                                } else if dark {
                                    rgba(if alt_group { 0xffffff04 } else { 0xffffff07 })
                                } else {
                                    rgba(if alt_group { 0x0f172a06 } else { 0x0f172a0b })
                                };
                                let hover_bg = rgba(if dark { 0xffffff12 } else { 0x0f172a14 });
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(22.0))
                                    .rounded(px(ds.radius_sm))
                                    .bg(cell_bg)
                                    .hover(move |style| style.bg(hover_bg))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(occupied, |el| el.child(led_dot(ds.accent, 9.5, true)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.apply_pattern_action(PatternAction::SongCell(
                                                cell,
                                            ));
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    )
                            }))
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(6.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .font_family("monospace")
                            .text_size(px(10.0))
                            .text_color(rgb(ds.muted))
                            .child(format!(
                                "Compases {}–{}",
                                page_start / bar + 1,
                                page_end / bar,
                            )),
                    )
                    .child(div().flex().items_center().gap(px(4.0)).children(
                        [("Ant", -1), ("Sig", 1)].into_iter().map(|(label, delta)| {
                            flat_control(ds, false, ds.cyan)
                                .h(px(20.0))
                                .px(px(8.0))
                                .rounded(px(ds.radius_sm))
                                .text_size(px(10.0))
                                .text_color(rgb(ds.muted))
                                .child(label)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        let page = &mut this.pattern_editor.song_page;
                                        *page = (*page + delta).clamp(0, last_page + 1);
                                        cx.notify();
                                    }),
                                )
                        }),
                    )),
            )
    }
}
