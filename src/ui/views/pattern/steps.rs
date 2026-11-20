//! Pads y previsualización de notas reales en la fila de cada pieza.
use crate::ui::{
    DawApp, DesignSystem,
    components::{button::flat_control, meter::track_peak_meter},
    design::blend_color,
    pattern_editor::PatternAction,
};
use elevate::musical::{
    model::NoteEvent,
    pitch::note_name,
    time::{Meter, Tick},
};
use gpui::{
    Context, Div, IntoElement, MouseButton, Stateful, canvas, div, fill, point, prelude::*, px,
    rgb, size,
};

const NAME_WIDTH: f32 = 116.0;
const STEP_WIDTH: f32 = 26.0;

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
        let steps = editor.step_count();
        let playing = self.pattern_play_step();
        let pattern = editor.session.active_pattern();
        let meter = pattern.meter;
        let step_ticks = editor.ticks_per_step();
        let span = steps as i64 * step_ticks;
        let width = |ticks: i64| px(STEP_WIDTH * ticks as f32 / step_ticks as f32);
        let play_at = playing.map(|step| step as i64 * step_ticks);

        // Las reglas se dibujan en ticks para quedar alineadas con las notas.
        let ruler = |label, stride: i64, measure| {
            div()
                .flex()
                .h(px(18.0))
                .flex_shrink_0()
                .bg(rgb(ds.panel))
                .items_center()
                .child(
                    div()
                        .w(px(NAME_WIDTH))
                        .flex_shrink_0()
                        .px_2()
                        .text_color(rgb(ds.muted))
                        .child(label),
                )
                .children(
                    (0..pattern.notes.length().0)
                        .step_by(stride as usize)
                        .map(|at| {
                            let active =
                                play_at.is_some_and(|play| (at..at + stride).contains(&play));
                            let index = if measure {
                                at / stride + 1
                            } else {
                                (at % meter.ticks_per_bar()) / stride + 1
                            };
                            div()
                                .h_full()
                                .w(width(stride.min(span - at)))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .px_1()
                                .border_l_1()
                                .border_color(rgb(beat_line(ds, meter, at)))
                                .bg(rgb(if measure {
                                    ds.panel
                                } else {
                                    beat_surface(ds, meter, at)
                                }))
                                .text_color(rgb(if active {
                                    ds.cyan
                                } else if measure {
                                    ds.text
                                } else {
                                    ds.muted
                                }))
                                .overflow_hidden()
                                .child(index.to_string())
                        }),
                )
        };
        div()
            .id("drum-steps")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_x_scroll()
            .font_family("monospace")
            .text_xs()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w(px(NAME_WIDTH) + width(span))
                    .child(ruler("COMPÁS", meter.ticks_per_bar(), true))
                    .child(ruler("PULSO", meter.ticks_per_beat(), false))
                    .child(
                        div()
                            .id("drum-step-lanes")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&editor.steps_scroll)
                            .when(editor.session.kit.pieces.is_empty(), |lanes| {
                                lanes.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .self_start()
                                        .w_full()
                                        .gap_2()
                                        .p_4()
                                        .text_color(rgb(ds.muted))
                                        .child("Arrastra una muestra desde el Explorador")
                                        .when(!self.explorer_visible(), |hint| {
                                            hint.child(
                                                flat_control(ds, false, ds.cyan)
                                                    .h(px(22.0))
                                                    .px_2()
                                                    .text_color(rgb(ds.cyan))
                                                    .child("Abrir explorador")
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.toggle_explorer();
                                                            cx.notify();
                                                        }),
                                                    ),
                                            )
                                        }),
                                )
                            })
                            .children(
                                (0..editor.session.kit.pieces.len())
                                    .map(|lane| self.render_drum_step_lane(lane, playing, cx)),
                            ),
                    ),
            )
    }

    fn render_drum_step_lane(
        &self,
        lane: usize,
        playing: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let editor = &self.pattern_editor;
        let piece = &editor.session.kit.pieces[lane];
        let palette = [ds.accent, ds.orange, ds.cyan, ds.warning];
        let color = palette[lane % palette.len()];
        let root = piece.base_note();
        let step_ticks = editor.ticks_per_step();
        let meter = editor.session.active_pattern().meter;
        let peak = self
            .player
            .as_ref()
            .map_or(0.0, |player| player.drum_piece_peak(piece.id));
        let notes: Vec<_> = editor
            .pattern()
            .notes()
            .iter()
            .filter(|note| note.instrument == piece.id)
            .copied()
            .collect();
        let showing_preview = notes
            .iter()
            .any(|note| note.pitch != root || note.start.0 % step_ticks != 0)
            || notes.windows(2).any(|pair| pair[0].start == pair[1].start);
        let steps = editor.step_count();
        let cursor = (editor.cursor_lane == lane).then_some(editor.cursor_step);

        div()
            .flex()
            .items_center()
            .h(px(36.0))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(blend_color(ds.panel_dark, ds.border, 45)))
            .child(
                flat_control(ds, cursor.is_some(), color)
                    .relative()
                    .w(px(NAME_WIDTH))
                    .h_full()
                    .justify_start()
                    .px_2()
                    .gap_1()
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
                    ),
            )
            .child(if showing_preview {
                div()
                    .w(px(STEP_WIDTH * steps as f32))
                    .h_full()
                    .flex_shrink_0()
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
                        Tick(steps as i64 * step_ticks),
                        meter,
                        color,
                    ))
            } else {
                let mut notes = notes.iter().peekable();
                div()
                    .flex()
                    .flex_shrink_0()
                    .children((0..steps).map(|step| {
                        let at = step as i64 * step_ticks;
                        let note = notes.next_if(|note| note.start.0 == at);
                        self.render_drum_pad(
                            lane,
                            step,
                            note,
                            color,
                            cursor == Some(step),
                            playing == Some(step),
                            cx,
                        )
                    }))
            })
    }

    fn render_drum_pad(
        &self,
        lane: usize,
        step: usize,
        note: Option<&NoteEvent>,
        color: u32,
        selected: bool,
        playing: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let ds = &self.ds;
        let occupied = note.is_some();
        let meter = self.pattern_editor.session.active_pattern().meter;
        let at = step as i64 * self.pattern_editor.ticks_per_step();
        let surface = if occupied {
            blend_color(ds.panel, color, 53)
        } else if playing {
            ds.panel_hover
        } else {
            blend_color(beat_surface(ds, meter, at), ds.text, 15)
        };
        div()
            .w(px(STEP_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(1.5))
            .bg(rgb(beat_surface(ds, meter, at)))
            .when(at % meter.ticks_per_beat() == 0, |column| {
                column
                    .border_l_1()
                    .border_color(rgb(beat_line(ds, meter, at)))
            })
            .child(
                flat_control(ds, occupied, color)
                    .w_full()
                    .h(px(25.0))
                    .bg(rgb(surface))
                    .when(selected || playing, |pad| {
                        pad.border_b_1()
                            .border_color(rgb(if playing { ds.cyan } else { color }))
                    })
                    .text_color(rgb(ds.text))
                    .child(note.map_or_else(String::new, |note| note.velocity.to_string()))
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
                            this.apply_pattern_action(PatternAction::Velocity {
                                lane,
                                step,
                                delta: delta.signum() as i16 * 5,
                            });
                            cx.stop_propagation();
                            cx.notify();
                        }
                    })),
            )
    }
}

fn drum_mini_roll(
    ds: &DesignSystem,
    notes: Vec<NoteEvent>,
    root: u8,
    length: Tick,
    meter: Meter,
    color: u32,
) -> impl IntoElement + use<> {
    let ds = ds.clone();
    let background = ds.panel_dark;
    let guide = ds.border;
    let (low, high) = notes.iter().fold((root, root), |(low, high), note| {
        (low.min(note.pitch), high.max(note.pitch))
    });
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let width = f32::from(bounds.size.width);
            let x_at = |tick| px(width * tick as f32 / length.0 as f32);
            let beat = meter.ticks_per_beat();
            for at in (0..length.0).step_by(beat as usize) {
                let origin = point(bounds.left() + x_at(at), bounds.top());
                window.paint_quad(fill(
                    gpui::Bounds::new(
                        origin,
                        size(x_at(beat.min(length.0 - at)), bounds.size.height),
                    ),
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
                let x = x_at(note.start.0);
                let span = x_at(note.duration.0);
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
