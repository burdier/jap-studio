//! Pads y previsualización de notas reales en la fila de cada pieza.
use crate::ui::{
    DawApp, DesignSystem,
    components::{button::flat_control, meter::track_peak_meter},
    design::blend_color,
    explorer::ExplorerSampleDrag,
    pattern_editor::PatternAction,
};
use elevate::musical::{
    drums::{DrumPiece, OffsetMode},
    model::NoteEvent,
    pitch::note_name,
    time::{Meter, PPQ},
};
use gpui::{
    Context, CursorStyle, Div, IntoElement, MouseButton, Stateful, canvas,
    component::tooltip::Tooltip, div, fill, point, prelude::*, px, relative, rgb, rgba, size,
};

const NAME_WIDTH: f32 = 190.0;

fn beat_surface(ds: &DesignSystem, meter: Meter, at: i64) -> u32 {
    let beat = (at % meter.ticks_per_bar()) / meter.ticks_per_beat();
    blend_color(ds.panel_dark, ds.panel, if beat % 2 == 0 { 74 } else { 38 })
}

fn beat_line(ds: &DesignSystem, meter: Meter, at: i64) -> u32 {
    if at % meter.ticks_per_bar() == 0 {
        blend_color(ds.border, ds.muted, 35)
    } else {
        ds.border
    }
}

impl DawApp {
    pub(super) fn render_drum_steps(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let editor = &self.pattern_editor;
        let playing = self.pattern_play_step();
        let meter = self.timeline.meter;
        let play_at = self
            .drum_pattern_playhead()
            .map(|beat| (beat * PPQ as f64) as i64);

        // Regla unificada compacta de compases y pulsos para la página visible (100% ancho, sin scroll horizontal).
        let beat_stride = meter.ticks_per_beat().max(1);
        let bar_ticks = meter.ticks_per_bar().max(1);
        let (_, page_start, page_end) = editor.visible_step_window(meter, play_at);
        let page_span = (page_end - page_start).max(1);
        let piece_count = editor.session.kit.pieces.len();
        let beat_ruler = div()
            .flex()
            .w_full()
            .h(px(22.0))
            .flex_shrink_0()
            .bg(rgb(ds.panel))
            .border_b_1()
            .border_color(rgba(if ds.is_dark() { 0xffffff0d } else { 0x0f172a12 }))
            .items_center()
            .child(
                div()
                    .w(px(NAME_WIDTH))
                    .flex_shrink_0()
                    .px(px(10.0))
                    .text_size(px(10.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(ds.muted))
                    .child(format!("KIT · {piece_count}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .children(
                        (page_start..page_end)
                            .step_by(beat_stride as usize)
                            .map(|at| {
                                let active = play_at
                                    .is_some_and(|play| (at..at + beat_stride).contains(&play));
                                let is_bar_start = at % bar_ticks == 0;
                                let is_bar_divider = at > page_start && is_bar_start;
                                let bar_num = at / bar_ticks + 1;
                                let beat_num = (at % bar_ticks) / beat_stride + 1;
                                let label = if is_bar_start {
                                    format!("{bar_num}")
                                } else {
                                    format!("{bar_num}.{beat_num}")
                                };
                                let frac = beat_stride.min(page_end - at) as f32 / page_span as f32;
                                div()
                                    .h_full()
                                    .flex_grow(frac)
                                    .flex_basis(px(0.0))
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .px(px(6.0))
                                    .when(is_bar_divider, |el| el.border_l_2())
                                    .when(!is_bar_divider, |el| el.border_l_1())
                                    .border_color(rgb(if is_bar_divider {
                                        ds.accent
                                    } else {
                                        beat_line(ds, meter, at)
                                    }))
                                    .bg(rgb(beat_surface(ds, meter, at)))
                                    .font_family("monospace")
                                    .text_size(px(10.0))
                                    .font_weight(if is_bar_start || active {
                                        gpui::FontWeight::BOLD
                                    } else {
                                        gpui::FontWeight::NORMAL
                                    })
                                    .text_color(rgb(if active {
                                        ds.cyan
                                    } else if is_bar_start {
                                        ds.text
                                    } else {
                                        ds.muted
                                    }))
                                    .overflow_hidden()
                                    .child(label)
                            }),
                    ),
            );
        div()
            .id("drum-steps")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .min_w_0()
                    .child(beat_ruler)
                    .child(
                        div()
                            .id("drum-step-lanes")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .w_full()
                            .min_h_0()
                            .min_w_0()
                            .overflow_y_scroll()
                            .track_scroll(&editor.steps_scroll)
                            .children((0..editor.session.kit.pieces.len()).map(|lane| {
                                self.render_drum_step_lane(lane, playing, page_start, page_end, cx)
                            })),
                    ),
            )
    }

    fn render_drum_step_lane(
        &self,
        lane: usize,
        playing: Option<usize>,
        page_start: i64,
        page_end: i64,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        let editor = &self.pattern_editor;
        let piece = &editor.session.kit.pieces[lane];
        let piece_id = piece.id;
        let cyan = ds.cyan;
        let palette = [ds.accent, ds.orange, ds.cyan, ds.warning];
        let color = palette[lane % palette.len()];
        let root = piece.base_note();
        let step_ticks = editor.ticks_per_step().max(1);
        #[cfg(feature = "p2p")]
        let length = editor.pattern().length();
        let meter = self.timeline.meter;
        let peak = self
            .player
            .as_ref()
            .map_or(0.0, |player| player.drum_piece_peak(piece.id));
        let notes: Vec<_> = editor
            .pattern()
            .notes()
            .iter()
            .filter(|note| note.instrument == piece.id)
            .cloned()
            .collect();
        let showing_preview = notes.iter().any(|note| note.pitch != root)
            || notes.windows(2).any(|pair| pair[0].start == pair[1].start);
        let total_steps = editor.step_count().max(1);
        let first_step = ((page_start / step_ticks) as usize).min(total_steps);
        let end_step = (((page_end + step_ticks - 1) / step_ticks) as usize).min(total_steps);
        let visible_steps = end_step.saturating_sub(first_step).max(1);
        let page_span = (page_end - page_start).max(1);
        let cursor = (editor.cursor_lane == lane).then_some(editor.cursor_step);

        let row = div()
            .relative()
            .id(("drum-step-lane", piece_id.0 as usize))
            .flex()
            .items_center()
            .w_full()
            .min_w_0()
            .h(px(40.0))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(blend_color(ds.panel_dark, ds.border, 45)))
            .on_drop(
                cx.listener(move |this, files: &gpui::ExternalPaths, window, cx| {
                    this.drop_drum_files(files.paths(), Some(piece_id), window, cx);
                }),
            )
            .drag_over::<ExplorerSampleDrag>(move |style, _, _, _| style.border_color(rgb(cyan)))
            .on_drop(
                cx.listener(move |this, sample: &ExplorerSampleDrag, window, cx| {
                    this.drop_drum_sample(sample.path.clone(), Some(piece_id), window, cx);
                }),
            )
            .child(
                flat_control(ds, cursor.is_some(), color)
                    .id(("drum-lane-head", piece_id.0 as usize))
                    .relative()
                    .w(px(NAME_WIDTH))
                    .h_full()
                    .justify_start()
                    .px(px(4.0))
                    .gap(px(4.0))
                    .child({
                        let max = DrumPiece::MAX_TIMING_OFFSET;
                        let offset = editor.piece_timing_offset(piece_id, piece.timing_offset);
                        let mode = piece.timing_mode;
                        let mode_color = match mode {
                            OffsetMode::Fixed => ds.muted,
                            OffsetMode::EachNote => ds.cyan,
                            OffsetMode::Random => ds.accent,
                        };
                        let ms =
                            f64::from(offset) * 60_000.0 / (self.timeline.tempo.bpm() * PPQ as f64);
                        let tip = format!(
                            "Timing {:+} ms ({:+} ticks) · {} · arrastra izq/der o rueda · doble clic: 0",
                            ms.round() as i32,
                            offset,
                            mode.label()
                        );
                        let norm = (f32::from(offset) / f32::from(max)).clamp(-1.0, 1.0);
                        let mag = norm.abs();
                        let bar_color = if mag >= 0.72 {
                            ds.danger
                        } else if mag >= 0.42 {
                            ds.warning
                        } else {
                            ds.success
                        };
                        let bar_left = if norm < 0.0 { 0.5 + norm * 0.5 } else { 0.5 };
                        let bar_width = mag * 0.5;
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.0))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .id(("drum-inline-offset", piece_id.0 as usize))
                                    .relative()
                                    .w(px(38.0))
                                    .h(px(12.0))
                                    .flex_shrink_0()
                                    .rounded(px(2.5))
                                    .bg(rgb(ds.panel_dark))
                                    .border_1()
                                    .border_color(if mag >= 0.72 {
                                        rgb(ds.danger)
                                    } else {
                                        rgba((ds.text << 8) | 0x26)
                                    })
                                    .overflow_hidden()
                                    .cursor(CursorStyle::ResizeLeftRight)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(tip.clone()).build(window, cx)
                                    })
                                    // Zonas rojas sutiles en ambos extremos
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .left_0()
                                            .w(relative(0.14))
                                            .bg(rgba((ds.danger << 8) | 0x28)),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .right_0()
                                            .w(relative(0.14))
                                            .bg(rgba((ds.danger << 8) | 0x28)),
                                    )
                                    // Relleno desde el centro hacia izquierda o derecha
                                    .when(bar_width > 0.005, |bar| {
                                        bar.child(
                                            div()
                                                .absolute()
                                                .top(px(1.5))
                                                .bottom(px(1.5))
                                                .left(relative(bar_left))
                                                .w(relative(bar_width))
                                                .bg(rgb(bar_color)),
                                        )
                                    })
                                    // Marca central (0)
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .left(relative(0.5))
                                            .w(px(1.0))
                                            .bg(rgba((ds.text << 8) | 0x70)),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |this, event: &gpui::MouseDownEvent, _, cx| {
                                                if event.click_count == 2 {
                                                    this.apply_pattern_action(
                                                        PatternAction::PieceTimingOffset {
                                                            piece: piece_id,
                                                            value: 0,
                                                        },
                                                    );
                                                } else {
                                                    this.begin_drum_timing_drag(
                                                        piece_id,
                                                        offset,
                                                        event.position.x,
                                                    );
                                                }
                                                cx.stop_propagation();
                                                cx.notify();
                                            },
                                        ),
                                    )
                                    .on_scroll(cx.listener(
                                        move |this, event: &gpui::ScrollEvent, _, cx| {
                                            let delta =
                                                f32::from(event.delta.pixel_delta(px(16.0)).y);
                                            if delta != 0.0 {
                                                let step = if event.modifiers.shift { 1 } else { 6 };
                                                let value = (offset
                                                    + delta.signum() as i16 * step)
                                                    .clamp(-max, max);
                                                this.apply_pattern_action(
                                                    PatternAction::PieceTimingOffset {
                                                        piece: piece_id,
                                                        value,
                                                    },
                                                );
                                                cx.stop_propagation();
                                                cx.notify();
                                            }
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .id(("drum-inline-offset-mode", piece_id.0 as usize))
                                    .flex_shrink_0()
                                    .px(px(3.0))
                                    .py(px(1.0))
                                    .rounded(px(2.0))
                                    .bg(rgb(ds.panel_dark))
                                    .border_1()
                                    .border_color(rgba((mode_color << 8) | 0x66))
                                    .text_size(px(8.5))
                                    .text_color(rgb(mode_color))
                                    .cursor_pointer()
                                    .tooltip(|window, cx| {
                                        Tooltip::new(
                                            "Modo timing: Fijo · Cada nota (groove) · Random (clic para cambiar)",
                                        )
                                        .build(window, cx)
                                    })
                                    .child(mode.short_label())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.apply_pattern_action(
                                                PatternAction::CyclePieceTimingMode {
                                                    piece: piece_id,
                                                },
                                            );
                                            cx.stop_propagation();
                                            cx.notify();
                                        }),
                                    ),
                            )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(color))
                            .child(piece.name.clone()),
                    )
                    .child(
                        div()
                            .pr_2()
                            .text_color(rgb(ds.muted))
                            .child(note_name(root)),
                    )
                    .child(track_peak_meter(ds, peak))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.show_roll(lane);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.pattern_editor.cursor_lane = lane;
                            this.pattern_editor.lane_menu =
                                Some(crate::ui::pattern_editor::DrumLaneMenu {
                                    lane,
                                    step: None,
                                    position: event.position,
                                });
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(if showing_preview {
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.show_roll(lane);
                            cx.notify();
                        }),
                    )
                    .child(drum_mini_roll(
                        ds,
                        notes,
                        root,
                        page_start,
                        page_end,
                        meter,
                        color,
                    ))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .children((first_step..end_step).map(|step| {
                                let at = step as i64 * step_ticks;
                                let span = step_ticks.min(page_end - at).max(1);
                                let frac = span as f32 / page_span as f32;
                                self.with_drum_split(
                                    div()
                                        .flex_grow(frac)
                                        .flex_basis(px(0.0))
                                        .min_w_0()
                                        .h_full(),
                                    lane,
                                    step,
                                    cx,
                                )
                            })),
                    )
            } else {
                let first_idx = notes.partition_point(|note| note.start.0 < page_start);
                let mut pending = &notes[first_idx..];
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .children((first_step..end_step).map(|step| {
                        let at = step as i64 * step_ticks;
                        let next = pending.partition_point(|note| note.start.0 < at + step_ticks);
                        let (hits, rest) = pending.split_at(next);
                        pending = rest;
                        let lead = playing.and_then(|play| {
                            let dist = (step + total_steps - play) % total_steps;
                            (dist <= 2).then_some(dist as u8)
                        });
                        self.render_drum_pad(
                            lane,
                            step,
                            hits,
                            color,
                            cursor == Some(step),
                            lead,
                            page_start,
                            page_end,
                            visible_steps,
                            cx,
                        )
                    }))
            });
        #[cfg(feature = "p2p")]
        let row = row.when_some(
            self.peer_drum_step(piece_id)
                .filter(|(_, target)| target.tick >= page_start && target.tick < page_end),
            |row, (alias, target)| {
                let step = target.tick / step_ticks;
                let start = step * step_ticks;
                let rel_start = (start - page_start).max(0) as f32 / page_span as f32;
                let rel_width = step_ticks.min(length.0 - start).max(1) as f32 / page_span as f32;
                let design = ds.clone();
                row.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, cx| {
                            let lane_w = (f32::from(bounds.size.width) - NAME_WIDTH).max(1.0);
                            let x = bounds.left() + px(NAME_WIDTH + lane_w * rel_start + 1.0);
                            let width = (lane_w * rel_width).max(3.0);
                            let cell = gpui::Bounds::new(
                                point(x, bounds.top() + px(4.0)),
                                size(px((width - 2.0).max(1.0)), px(31.0)),
                            );
                            paint_step_peer(bounds, cell, &alias, &design, window, cx);
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                )
            },
        );
        row
    }

    fn with_drum_split(&self, pad: Div, lane: usize, step: usize, cx: &mut Context<Self>) -> Div {
        pad.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                this.pattern_editor.cursor_lane = lane;
                this.pattern_editor.cursor_step = step;
                let occupied = this.pattern_editor.step_notes(lane, step).next().is_some();
                if occupied {
                    this.apply_pattern_action(PatternAction::SplitStep { lane, step });
                } else {
                    this.pattern_editor.lane_menu = Some(crate::ui::pattern_editor::DrumLaneMenu {
                        lane,
                        step: None,
                        position: event.position,
                    });
                }
                cx.stop_propagation();
                cx.notify();
            }),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_drum_pad(
        &self,
        lane: usize,
        step: usize,
        notes: &[NoteEvent],
        color: u32,
        selected: bool,
        lead: Option<u8>,
        page_start: i64,
        page_end: i64,
        visible_steps: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let occupied = !notes.is_empty();
        let hits = notes.len();
        let meter = self.timeline.meter;
        let step_ticks = self.pattern_editor.ticks_per_step().max(1);
        let at = step as i64 * step_ticks;
        let span = step_ticks.min(page_end - at).max(1);
        let page_span = (page_end - page_start).max(1);
        let frac = span as f32 / page_span as f32;
        let show_velocity = visible_steps <= 16;
        let is_bar_divider = at > page_start && at % meter.ticks_per_bar().max(1) == 0;
        let aligned = notes.first().is_some_and(|note| note.start.0 == at);
        let lead_1 = blend_color(ds.cyan, ds.accent, 45);
        let lead_2 = blend_color(ds.accent, ds.muted, 35);
        let base_surface = if occupied {
            blend_color(ds.panel_dark, color, if aligned { 24 } else { 14 })
        } else {
            blend_color(beat_surface(ds, meter, at), ds.panel_dark, 45)
        };
        let surface = match lead {
            Some(0) => blend_color(base_surface, ds.cyan, 36),
            Some(1) => blend_color(base_surface, lead_1, 22),
            Some(2) => blend_color(base_surface, lead_2, 12),
            _ => base_surface,
        };
        let border = match lead {
            Some(0) => ds.cyan,
            Some(1) => blend_color(ds.panel, lead_1, 74),
            Some(2) => blend_color(ds.panel, lead_2, 46),
            _ if occupied => blend_color(color, ds.text, 28),
            _ => blend_color(ds.border, ds.muted, 24),
        };
        let dot_color = match lead {
            Some(0) => ds.cyan,
            Some(1) if occupied => blend_color(color, lead_1, 45),
            Some(1) => lead_1,
            Some(2) if occupied => blend_color(color, lead_2, 25),
            Some(2) => blend_color(ds.muted, lead_2, 58),
            _ if occupied => color,
            _ => ds.muted,
        };
        let first_note = notes.first();
        let root_pitch = self
            .pattern_editor
            .session
            .kit
            .pieces
            .get(lane)
            .map_or(36, |p| p.base_note());
        let first_prob = first_note.map_or(100, |n| n.expression.probability);
        let first_pitch_delta =
            first_note.map_or(0, |n| i16::from(n.pitch) - i16::from(root_pitch));
        let overlay_label = first_note.filter(|_| show_velocity).map(|note| {
            if hits > 1 {
                format!("×{hits}")
            } else if first_pitch_delta != 0 {
                format!("{first_pitch_delta:+}")
            } else {
                note.velocity.to_string()
            }
        });
        let prob_badge =
            (occupied && first_prob < 100 && show_velocity).then(|| format!("{first_prob}%"));
        let pad_tooltip = first_note.map(|note| {
            let pitch_txt = if first_pitch_delta == 0 {
                note_name(note.pitch)
            } else {
                format!("{} ({first_pitch_delta:+} st)", note_name(note.pitch))
            };
            format!(
                "Vel {} · Prob {}% · {} · Rueda: vel · Shift+rueda: prob · Alt+rueda: pitch · Clic der: roll",
                note.velocity, note.expression.probability, pitch_txt
            )
        });
        div()
            .relative()
            .flex_grow(frac)
            .flex_basis(px(0.0))
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .px(px(if visible_steps <= 16 { 2.0 } else { 1.0 }))
            .bg(rgb(beat_surface(ds, meter, at)))
            .when(is_bar_divider, |column| {
                column.border_l_2().border_color(rgb(ds.accent))
            })
            .when(
                !is_bar_divider && at % meter.ticks_per_beat() == 0,
                |column| {
                    column
                        .border_l_1()
                        .border_color(rgb(beat_line(ds, meter, at)))
                },
            )
            .child(
                flat_control(ds, occupied, color)
                    .id(("drum-step", lane * self.pattern_editor.step_count() + step))
                    .relative()
                    .w_full()
                    .min_w_0()
                    .h(px(31.0))
                    .bg(rgb(surface))
                    .when(occupied || selected || lead.is_some(), |pad| {
                        pad.border_1().border_color(rgb(border))
                    })
                    .child(
                        div()
                            .text_color(rgb(dot_color))
                            .when(occupied && first_prob < 100, |el| {
                                el.opacity(0.35 + 0.65 * (f32::from(first_prob) / 100.0))
                            })
                            .text_size(px(if occupied { 13.0 } else { 12.0 }))
                            .child(if occupied { "●" } else { "·" }),
                    )
                    .when_some(overlay_label, |pad, label| {
                        let label_color = if first_pitch_delta != 0 && hits == 1 {
                            ds.warning
                        } else {
                            ds.text
                        };
                        pad.child(
                            div()
                                .absolute()
                                .top(px(1.5))
                                .left(px(1.5))
                                .right(px(1.5))
                                .h(px(13.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.0))
                                .bg(rgba((ds.panel_dark << 8) | 0x9c))
                                .border_1()
                                .border_color(rgba((ds.text << 8) | 0x2a))
                                .text_size(px(9.0))
                                .text_color(rgb(label_color))
                                .overflow_hidden()
                                .child(label),
                        )
                    })
                    .when_some(prob_badge, |pad, label| {
                        pad.child(
                            div()
                                .absolute()
                                .bottom(px(1.0))
                                .left(px(1.5))
                                .right(px(1.5))
                                .h(px(10.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(2.0))
                                .bg(rgba((ds.panel_dark << 8) | 0xb8))
                                .text_size(px(7.5))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(ds.cyan))
                                .overflow_hidden()
                                .child(label),
                        )
                    })
                    .when_some(pad_tooltip, |pad, tip| {
                        pad.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.pattern_editor.cursor_lane = lane;
                            this.pattern_editor.cursor_step = step;
                            if occupied {
                                this.apply_pattern_action(PatternAction::SplitStep { lane, step });
                            } else {
                                this.pattern_editor.lane_menu =
                                    Some(crate::ui::pattern_editor::DrumLaneMenu {
                                        lane,
                                        step: None,
                                        position: event.position,
                                    });
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.pattern_editor.cursor_lane = lane;
                            this.pattern_editor.cursor_step = step;
                            this.apply_pattern_action(PatternAction::Toggle { lane, step });
                            cx.notify();
                        }),
                    )
                    .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(24.0)).y);
                        if occupied && delta != 0.0 {
                            let dir = delta.signum();
                            let action = if event.modifiers.shift {
                                PatternAction::StepProbability {
                                    lane,
                                    step,
                                    delta: dir as i16 * 5,
                                }
                            } else if event.modifiers.alt {
                                PatternAction::StepPitch {
                                    lane,
                                    step,
                                    delta: dir as i8,
                                }
                            } else {
                                PatternAction::Velocity {
                                    lane,
                                    step,
                                    delta: dir as i16 * 5,
                                }
                            };
                            this.apply_pattern_action(action);
                            cx.stop_propagation();
                            cx.notify();
                        }
                    })),
            )
            .children(notes.iter().map(|note| {
                let rel = ((note.start.0 - at) as f32 / span as f32).clamp(0.05, 0.92);
                div()
                    .absolute()
                    .bottom(px(6.0))
                    .left(relative(rel))
                    .w(px(2.0))
                    .h(px(5.0))
                    .bg(rgb(border))
            }))
    }
}

#[cfg(feature = "p2p")]
pub(super) fn paint_step_peer(
    area: gpui::Bounds<gpui::Pixels>,
    cell: gpui::Bounds<gpui::Pixels>,
    alias: &str,
    ds: &DesignSystem,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    use gpui::{Bounds, ContentMask, TextAlign, TextRun, rgba};
    if cell.left() >= area.right()
        || cell.right() <= area.left()
        || cell.top() >= area.bottom()
        || cell.bottom() <= area.top()
    {
        return;
    }
    window.with_content_mask(Some(ContentMask { bounds: area }), |window| {
        window.paint_quad(
            fill(cell, rgba(0))
                .corner_radii(px(2.0))
                .border_widths(px(1.5))
                .border_color(rgb(ds.cyan)),
        );
        let run = TextRun {
            len: alias.len(),
            font: window.text_style().font(),
            color: rgb(ds.panel_dark).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window.text_system().shape_line(
            alias.to_owned().into(),
            px(ds.font_size_xs),
            &[run],
            None,
        );
        let height = px(ds.font_size_xs + 3.0);
        let width = (line.width + px(6.0)).min(area.size.width);
        let x = cell.left().clamp(area.left(), area.right() - width);
        let y = (cell.top() - height).max(area.top());
        window.paint_quad(
            fill(Bounds::new(point(x, y), size(width, height)), rgb(ds.cyan)).corner_radii(px(2.0)),
        );
        let _ = line.paint(
            point(x + px(3.0), y),
            height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    });
}

fn drum_mini_roll(
    ds: &DesignSystem,
    notes: Vec<NoteEvent>,
    root: u8,
    page_start: i64,
    page_end: i64,
    meter: Meter,
    color: u32,
) -> impl IntoElement + use<> {
    let ds = ds.clone();
    let background = ds.panel_dark;
    let guide = ds.border;
    let page_span = (page_end - page_start).max(1);
    let (low, high) = notes.iter().fold((root, root), |(low, high), note| {
        (low.min(note.pitch), high.max(note.pitch))
    });
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let width = f32::from(bounds.size.width);
            let x_at = |tick: i64| px(width * (tick - page_start) as f32 / page_span as f32);
            let beat = meter.ticks_per_beat().max(1);
            for at in (page_start..page_end).step_by(beat as usize) {
                let origin = point(bounds.left() + x_at(at), bounds.top());
                let w = px(width * beat.min(page_end - at) as f32 / page_span as f32);
                window.paint_quad(fill(
                    gpui::Bounds::new(origin, size(w, bounds.size.height)),
                    rgb(beat_surface(&ds, meter, at)),
                ));
                window.paint_quad(fill(
                    gpui::Bounds::new(origin, size(px(1.0), bounds.size.height)),
                    rgb(beat_line(&ds, meter, at)),
                ));
            }
            let row =
                (f32::from(bounds.size.height) - 8.0).max(2.0) / f32::from(high - low + 1).max(5.0);
            let mid = f32::from(high - low) / 2.0;
            let y = |pitch| {
                bounds.top()
                    + bounds.size.height * 0.5
                    + px((mid - f32::from(pitch - low) - 0.5) * row)
            };
            window.paint_quad(fill(
                gpui::Bounds::new(
                    point(bounds.left(), y(root) + px(row * 0.5)),
                    size(bounds.size.width, px(1.0)),
                ),
                rgb(guide),
            ));
            for note in &notes {
                if note.start.0 + note.duration.0 <= page_start || note.start.0 >= page_end {
                    continue;
                }
                let start = note.start.0.max(page_start);
                let end = (note.start.0 + note.duration.0).min(page_end);
                let x = x_at(start);
                let span = px(width * (end - start) as f32 / page_span as f32);
                let height = (row * 0.75).max(2.0);
                window.paint_quad(
                    fill(
                        gpui::Bounds::new(
                            point(bounds.left() + x, y(note.pitch)),
                            size(span.max(px(2.0)), px(height)),
                        ),
                        rgb(blend_color(
                            background,
                            color,
                            u32::from(note.velocity) / 2 + 30,
                        )),
                    )
                    .corner_radii(px(1.0)),
                );
            }
        },
    )
    .size_full()
}
