//! Pads y previsualización de notas reales en la fila de cada pieza.
use crate::ui::{
    DawApp, DesignSystem,
    components::{button::flat_control, meter::track_peak_meter},
    design::blend_color,
    pattern_editor::{PatternAction, TICKS_PER_STEP},
};
use elevate::musical::{model::NoteEvent, pitch::note_name, time::Tick};
use gpui::{
    Context, Div, IntoElement, MouseButton, Stateful, canvas, div, fill, point, prelude::*, px,
    rgb, size,
};

const NAME_WIDTH: f32 = 104.0;
const STEP_WIDTH: f32 = 27.0;

impl DawApp {
    pub(super) fn render_drum_steps(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let ds = &self.ds;
        let editor = &self.pattern_editor;
        let steps = editor.step_count();
        let playing = self.pattern_play_step();
        let bar = self.timeline.meter.ticks_per_bar();
        let ruler = div()
            .flex()
            .h(px(ds.timeline_bar_height))
            .flex_shrink_0()
            .bg(rgb(ds.panel))
            .items_center()
            .child(
                div()
                    .w(px(NAME_WIDTH))
                    .flex_shrink_0()
                    .px_2()
                    .text_color(rgb(ds.muted))
                    .child("PASOS"),
            )
            .children((0..steps).map(|step| {
                let at = step as i64 * TICKS_PER_STEP;
                let downbeat = at % bar == 0;
                div()
                    .w(px(STEP_WIDTH))
                    .flex_shrink_0()
                    .text_center()
                    .text_color(rgb(if playing == Some(step) {
                        ds.cyan
                    } else {
                        ds.muted
                    }))
                    .child(if downbeat {
                        (at / bar + 1).to_string()
                    } else {
                        "·".into()
                    })
            }));
        div()
            .id("drum-steps")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_x_scroll()
            .overflow_y_scroll()
            .font_family("monospace")
            .text_xs()
            .child(ruler)
            .children(
                (0..editor.session.kit.pieces.len())
                    .map(|lane| self.render_drum_step_lane(lane, playing, cx)),
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
        let showing_preview = notes.iter().any(|note| {
            note.pitch != root
                || note.start.0 % TICKS_PER_STEP != 0
                || note.duration.0 != TICKS_PER_STEP
        }) || notes.windows(2).any(|pair| pair[0].start == pair[1].start);
        let steps = editor.step_count();
        let cursor = (editor.cursor_lane == lane).then_some(editor.cursor_step);

        div()
            .flex()
            .items_center()
            .h(px(36.0))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(ds.border))
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
                        Tick(steps as i64 * TICKS_PER_STEP),
                        color,
                    ))
            } else {
                let mut notes = notes.iter().peekable();
                div()
                    .flex()
                    .flex_shrink_0()
                    .children((0..steps).map(|step| {
                        let at = step as i64 * TICKS_PER_STEP;
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
        let surface = if occupied {
            blend_color(ds.panel, color, 35)
        } else if playing {
            ds.panel_hover
        } else if step % 4 == 0 {
            ds.panel
        } else {
            ds.panel_dark
        };
        div().w(px(STEP_WIDTH)).flex_shrink_0().px(px(1.5)).child(
            flat_control(ds, occupied, color)
                .w_full()
                .h(px(26.0))
                .bg(rgb(surface))
                .hover(|style| style.bg(rgb(if occupied { color } else { ds.panel_hover })))
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
    color: u32,
) -> impl IntoElement + use<> {
    let background = ds.panel_dark;
    let guide = ds.muted;
    let (low, high) = notes.iter().fold((root, root), |(low, high), note| {
        (low.min(note.pitch), high.max(note.pitch))
    });
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            window.paint_quad(fill(bounds, rgb(background)));
            let width = f32::from(bounds.size.width);
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
                let x = width * note.start.0 as f32 / length.0 as f32;
                let span = width * note.duration.0 as f32 / length.0 as f32;
                let height = (row * 0.75).max(2.0);
                window.paint_quad(
                    fill(
                        gpui::Bounds::new(
                            point(bounds.left() + px(x), y(note.pitch)),
                            size(px(span.max(2.0)), px(height)),
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
