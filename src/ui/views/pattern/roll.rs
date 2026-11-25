//! Piano Roll Light dentro del panel de pasos. Comparte eje, notas y carril de velocidad.
use crate::ui::{
    DawApp,
    components::button::chip_option,
    piano_roll::{RollLane, geometry::Grid},
    views::piano_roll::{
        control_lane::paint_velocity_stem,
        grid_background::{label, paint_rows, paint_time, rectangle},
        note_frame,
    },
};
use elevate::musical::{drums::DrumPiece, pitch::note_name, scale::PitchScale, time::PPQ};
use gpui::{
    Bounds, ContentMask, Context, Div, MouseButton, MouseDownEvent, Stateful, canvas, div, fill,
    point, prelude::*, px, rgb, size,
};

impl DawApp {
    pub(super) fn render_drum_roll(
        &self,
        piece: &DrumPiece,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let editor = &self.pattern_editor;
        let ds = self.ds.clone();
        let root = piece.base_note();
        let piece_id = piece.id;
        let snap = editor.roll.snap;
        let meter = self.timeline.meter;
        let viewport = editor.roll.viewport;
        let grid_handle = editor.roll.grid.clone();
        let selected = editor.roll.selected;
        let preview = editor.roll.preview.clone();
        let notes: Vec<_> = editor
            .pattern()
            .notes()
            .iter()
            .filter(|note| note.instrument == piece.id)
            .cloned()
            .collect();
        let total_beats = editor.pattern().length().0 as f64 / PPQ as f64;
        #[cfg(feature = "p2p")]
        let peer = self.peer_drum_step(piece.id).map(|(alias, step)| {
            let length = (editor.pattern().length().0 - step.tick).min(editor.ticks_per_step());
            (alias, step, elevate::musical::time::Tick(length))
        });
        let playhead = self.drum_pattern_playhead();
        let title = format!("{} · raíz {}", piece.name, note_name(root));
        div()
            .id("drum-roll-light")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .gap_2()
                    .p_1()
                    .items_center()
                    .bg(rgb(ds.panel))
                    .child(chip_option(&ds, "‹ Pasos", false, ds.cyan).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.pattern_editor.roll.piece = None;
                            cx.notify();
                        }),
                    ))
                    .child(div().flex_1().min_w_0().truncate().text_xs().child(title))
                    .children(
                        [("−", snap.coarser()), ("+", snap.finer())]
                            .into_iter()
                            .map(|(label, resolution)| {
                                chip_option(&ds, label, false, ds.cyan).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.pattern_editor.roll.snap = resolution;
                                        cx.notify();
                                    }),
                                )
                            }),
                    )
                    .child(
                        div()
                            .font_family("monospace")
                            .text_xs()
                            .child(format!("1/{}", snap.denominator(),)),
                    ),
            )
            .child(
                div()
                    .id("drum-roll-notes")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            let grid = this.pattern_editor.roll.grid.get();
                            let key_position = point(grid.notes.left(), event.position.y);
                            let length = this.pattern_editor.pattern().length();
                            if event.position.x < grid.notes.left()
                                && let Some((_, pitch)) = grid.pick(key_position, length)
                            {
                                if !this.preview_drum_piece(piece_id, pitch, 100) {
                                    this.status_message =
                                        "Pulsa Play para escuchar las teclas del kit".into();
                                }
                            } else {
                                this.begin_drum_roll_input(event.position, event.click_count);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                        if event.dragging() && this.move_drum_roll_input(event.position) {
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                            if this.finish_drum_roll_input(event.position) {
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                            if this.finish_drum_roll_input(event.position) {
                                cx.notify();
                            }
                        }),
                    )
                    .on_scroll(cx.listener(move |this, event: &gpui::ScrollEvent, _, cx| {
                        let roll = &mut this.pattern_editor.roll;
                        roll.viewport.scroll(roll.grid.get(), total_beats, event);
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                let grid = Grid::new(bounds, total_beats, true, viewport);
                                grid_handle.set(grid);
                                grid
                            },
                            move |bounds, grid, window, cx| {
                                window.paint_quad(fill(bounds, rgb(ds.panel_dark)));
                                let rows = Bounds::new(
                                    point(bounds.left(), grid.notes.top()),
                                    size(bounds.size.width, grid.notes.size.height),
                                );
                                window.with_content_mask(
                                    Some(ContentMask { bounds: rows }),
                                    |window| {
                                        paint_rows(
                                            &grid,
                                            &ds,
                                            false,
                                            false,
                                            PitchScale::C_NATURAL_MINOR,
                                            window,
                                            cx,
                                        );
                                        rectangle(
                                            window,
                                            grid.notes.left(),
                                            grid.y(root),
                                            f32::from(grid.notes.size.width),
                                            grid.row_height,
                                            (ds.accent << 8) | 0x29,
                                        );
                                    },
                                );
                                paint_time(&grid, &ds, total_beats, meter, snap, window, cx);
                                for note in &notes {
                                    let note = preview
                                        .as_ref()
                                        .filter(|target| target.id == note.id)
                                        .unwrap_or(note);
                                    let rect =
                                        grid.note_bounds(note.start, note.duration, note.pitch);
                                    let active = selected == Some(note.id);
                                    let color = if active { ds.accent } else { ds.cyan };
                                    window.with_content_mask(
                                        Some(ContentMask { bounds: grid.notes }),
                                        |window| {
                                            note_frame::paint(
                                                rect, color, active, false, &ds, window,
                                            );
                                        },
                                    );
                                    window.with_content_mask(
                                        Some(ContentMask {
                                            bounds: grid.velocity,
                                        }),
                                        |window| {
                                            paint_velocity_stem(
                                                &grid,
                                                rect.left(),
                                                RollLane::Velocity.note_fraction(&note),
                                                color,
                                                window,
                                            );
                                        },
                                    );
                                }
                                #[cfg(feature = "p2p")]
                                if let Some((alias, step, duration)) = &peer {
                                    let cell = grid.note_bounds(
                                        elevate::musical::time::Tick(step.tick),
                                        *duration,
                                        step.pitch,
                                    );
                                    super::steps::paint_step_peer(
                                        grid.notes, cell, alias, &ds, window, cx,
                                    );
                                }
                                if let Some(beat) = playhead {
                                    let x = grid.x(beat);
                                    if x >= grid.notes.left() && x < grid.notes.right() {
                                        rectangle(
                                            window,
                                            x,
                                            grid.notes.top(),
                                            1.0,
                                            f32::from(grid.notes.size.height),
                                            (ds.cyan << 8) | 0xc0,
                                        );
                                    }
                                }
                                if grid.velocity.size.height > px(18.0) {
                                    label(
                                        window,
                                        cx,
                                        "Velocity",
                                        point(
                                            bounds.left() + px(8.0),
                                            grid.velocity.top() + px(3.0),
                                        ),
                                        ds.muted,
                                        ds.font_size_xs,
                                    );
                                }
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}
