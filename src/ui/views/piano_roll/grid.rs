//! Canvas de notas y velocities sobre la misma geometría que utiliza el controlador.
use super::grid_background::{label, paint_rows, paint_time, rectangle};
use crate::ui::{DawApp, DesignSystem, piano_roll::geometry::Grid};
use elevate::musical::{clip::MidiClip, model::NoteId, time::PPQ};
use gpui::{
    Bounds, ContentMask, Context, IntoElement, MouseButton, Window, canvas, div, fill, point,
    prelude::*, px, rgb, size,
};

impl DawApp {
    pub(super) fn render_roll_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let clip = self.timeline.midi_clip.clone().expect("clip visible");
        let ds = self.ds.clone();
        let cursor = self.piano_roll.cursor;
        let selected = self.piano_roll.selected;
        let velocities = self.piano_roll.velocities;
        let highlight = self.piano_roll.highlight_scale;
        let grid_handle = self.piano_roll.grid.clone();
        let beats = clip.pattern_beats();
        let viewport = self.piano_roll.viewport;
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .on_scroll(cx.listener(|this, event, _, cx| {
                this.scroll_piano_roll(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_pinch(cx.listener(|this, event, _, cx| {
                this.pinch_piano_roll(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.select_roll_note(event.position);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    move |bounds, _, _| {
                        let grid = Grid::new(bounds, beats, velocities, viewport);
                        grid_handle.set(grid);
                        grid
                    },
                    move |bounds, grid, window, cx| {
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            window.paint_quad(fill(bounds, rgb(ds.panel_dark)));
                            let rows = Bounds::new(
                                point(bounds.left(), grid.notes.top()),
                                size(bounds.size.width, grid.notes.size.height),
                            );
                            window.with_content_mask(
                                Some(ContentMask { bounds: rows }),
                                |window| {
                                    paint_rows(&grid, &ds, highlight, window, cx);
                                },
                            );
                            let time = Bounds::new(
                                point(grid.notes.left(), bounds.top()),
                                size(grid.notes.size.width, bounds.size.height),
                            );
                            window.with_content_mask(
                                Some(ContentMask { bounds: time }),
                                |window| {
                                    paint_time(&grid, &ds, beats, window, cx);
                                    paint_notes(&grid, &ds, &clip, selected, velocities, window);
                                },
                            );
                            if velocities {
                                label(
                                    window,
                                    cx,
                                    "Velocity ▾",
                                    point(bounds.left() + px(10.0), grid.velocity.top()),
                                    ds.text,
                                    10.0,
                                );
                                for value in [127, 96, 64, 32] {
                                    let y = grid.velocity.bottom()
                                        - px(value as f32 / 127.0
                                            * (f32::from(grid.velocity.size.height) - 16.0)
                                                .max(0.0));
                                    label(
                                        window,
                                        cx,
                                        &value.to_string(),
                                        point(grid.notes.left() - px(26.0), y - px(4.0)),
                                        ds.muted,
                                        9.0,
                                    );
                                }
                            }
                            let x = grid.x(cursor.0 as f64 / PPQ as f64);
                            if x >= grid.notes.left() && x < grid.notes.right() {
                                rectangle(
                                    window,
                                    x,
                                    grid.notes.top(),
                                    1.0,
                                    f32::from(grid.all.bottom() - grid.notes.top()) - 10.0,
                                    (ds.accent << 8) | 0xdb,
                                );
                                label(
                                    window,
                                    cx,
                                    "▼",
                                    point(x - px(5.0), grid.notes.top() - px(14.0)),
                                    ds.accent,
                                    11.0,
                                );
                            }
                            rectangle(
                                window,
                                grid.notes.left(),
                                bounds.bottom() - px(8.0),
                                f32::from(grid.notes.size.width),
                                6.0,
                                (ds.panel_dark << 8) | 0xff,
                            );
                            rectangle(
                                window,
                                grid.notes.left()
                                    + grid.notes.size.width * (grid.first_beat / beats) as f32,
                                bounds.bottom() - px(6.0),
                                f32::from(grid.notes.size.width) / viewport.zoom as f32,
                                3.0,
                                (ds.grid << 8) | 0xff,
                            );
                        });
                    },
                )
                .size_full(),
            )
    }
}

fn paint_notes(
    grid: &Grid,
    ds: &DesignSystem,
    clip: &MidiClip,
    selected: Option<NoteId>,
    velocities: bool,
    window: &mut Window,
) {
    for note in clip.notes() {
        let x = grid.x(note.start.0 as f64 / PPQ as f64) + px(2.0);
        let width = (note.duration.0 as f32 / PPQ as f32 * grid.pixels_per_beat - 3.0).max(2.0);
        let color = if selected == Some(note.id) {
            ds.text
        } else {
            ds.accent
        };
        window.with_content_mask(Some(ContentMask { bounds: grid.notes }), |window| {
            let y = grid.y(note.pitch) + px(grid.row_height * 0.13);
            let height = (grid.row_height * 0.74).max(1.0);
            window.paint_quad(
                fill(
                    Bounds::new(point(x, y), size(px(width), px(height))),
                    rgb(color),
                )
                .corner_radii(px(1.7)),
            );
            rectangle(
                window,
                x + px(1.0),
                y + px(1.0),
                (width - 2.0).max(0.0),
                1.0,
                0xffffff40,
            );
        });
        if velocities {
            window.with_content_mask(
                Some(ContentMask {
                    bounds: grid.velocity,
                }),
                |window| {
                    let height = f32::from(note.velocity) / 127.0
                        * (f32::from(grid.velocity.size.height) - 16.0).max(0.0);
                    rectangle(
                        window,
                        x + px(5.0),
                        grid.velocity.bottom() - px(height),
                        5.0,
                        height,
                        (color << 8) | 0xff,
                    );
                    rectangle(
                        window,
                        x + px(5.0),
                        grid.velocity.bottom() - px(height),
                        5.0,
                        1.0,
                        0xffffff60,
                    );
                },
            );
        }
    }
}
