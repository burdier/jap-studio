//! Canvas de notas y velocities sobre la misma geometría que utiliza el controlador.
use super::grid_background::{label, paint_rows, paint_time, rectangle};
use crate::ui::{
    DawApp, DesignSystem,
    piano_roll::{RollDrag, RollTool, geometry::Grid, motion::NotePreview, pitch::pitch_position},
};
use elevate::musical::{
    clip::MidiClip,
    model::{NoteEvent, NoteId},
    pitch_curve::{PitchCurve, PitchPoint},
    scale::PitchScale,
    time::PPQ,
};
use gpui::{
    Bounds, ContentMask, Context, IntoElement, MouseButton, PathBuilder, Window, canvas, div, fill,
    point, prelude::*, px, rgb, rgba, size,
};

use std::collections::HashSet;

struct GhostNote {
    beat: f64,
    duration: f64,
    pitch: u8,
}

impl DawApp {
    pub(super) fn render_roll_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let clip = self.timeline.midi_clip.clone().expect("clip visible");
        #[cfg(feature = "p2p")]
        let peer_note = self.peer_note_presence(&clip);
        let ds = self.ds.clone();
        let playhead = if self.timeline.playhead.is_some() {
            self.timeline.clip_visual_playhead(&clip)
        } else {
            Some(self.piano_roll.cursor.0 as f64 / PPQ as f64)
        };
        let selected = self.piano_roll.selected;
        let selected_notes: HashSet<_> = self.piano_roll.selected_notes.iter().copied().collect();
        let hovered_note = self.piano_roll.hovered_note;
        let pitch_mode = self.piano_roll.tool == RollTool::Pitch;
        let pitch_preview = self
            .piano_roll
            .pitch_drag
            .as_ref()
            .map(|drag| (drag.note, drag.preview.clone()));
        let hovered_resize_edge =
            self.piano_roll.hovered_resize_edge || self.piano_roll.tool == RollTool::Length;
        let drag_box = match &self.piano_roll.drag {
            Some(RollDrag::Box {
                origin,
                current,
                erase,
                ..
            }) => Some((*origin, *current, *erase)),
            _ => None,
        };
        let motion = self.piano_roll.motion.clone();
        let snap_guide = self.piano_roll.snap_guide.clone();
        let app = cx.weak_entity();
        let ghost_notes = if self.piano_roll.ghost_notes {
            ghost_notes_for(&clip, &self.timeline.other_midi_clips)
        } else {
            Vec::new()
        };
        let velocities = self.piano_roll.velocities;
        let active_lane = self.piano_roll.active_lane;
        let lane_contour = self.piano_roll.lane_contour;
        let lane_span = self.piano_roll.lane_span;
        let selected_control = self.piano_roll.selected_control;
        let highlight = self.piano_roll.highlight_scale;
        let scale_keyboard = self.piano_roll.show_scale_keyboard;
        let show_note_names = self.piano_roll.show_note_names;
        let pitch_scale = self.piano_roll.pitch_scale();
        let grid_handle = self.piano_roll.grid.clone();
        let beats = clip.pattern_beats();
        let meter = self.timeline.meter;
        let resolution = self.piano_roll.resolution;
        let viewport = self.piano_roll.viewport;
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .on_scroll(cx.listener(|this, event, _, cx| {
                this.scroll_piano_roll(event, cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_pinch(cx.listener(|this, event, _, cx| {
                this.pinch_piano_roll(event, cx);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.begin_roll_input(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    if this.open_lane_preset_menu(event.position) {
                        cx.notify();
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.cursor_fx.on_mouse_move(event.position);
                if event.dragging() {
                    if this.piano_roll.pitch_drag.is_some() {
                        this.drag_roll_pitch(event.position, event.modifiers.shift);
                    } else if this.piano_roll.lane_span.is_some() {
                        this.apply_lane_span_to(event.position, event.modifiers.shift);
                    } else if this.piano_roll.control_drag {
                        if this
                            .piano_roll
                            .grid
                            .get()
                            .velocity
                            .contains(&event.position)
                        {
                            this.edit_roll_at(event.position);
                        }
                    } else if this.piano_roll.paint_stroke.is_some() {
                        this.paint_roll_to(event.position);
                    } else {
                        this.drag_roll_to(event.position);
                    }
                    cx.notify();
                } else {
                    if this.piano_roll.drag.is_some()
                        || this.piano_roll.pitch_drag.is_some()
                        || this.piano_roll.control_drag
                        || this.piano_roll.lane_span.is_some()
                        || this.piano_roll.paint_stroke.is_some()
                    {
                        this.end_roll_drag();
                    }
                    if this.hover_roll_at(event.position) {
                        cx.notify();
                    }
                }
                if this.track_reorder_drag.is_none() {
                    cx.stop_propagation();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.finish_roll_drag(event.position);
                    if this.track_reorder_drag.is_none() {
                        cx.stop_propagation();
                    }
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
                        let mut motion = motion.borrow_mut();
                        if motion.advance() | snap_guide.animating() {
                            let app = app.clone();
                            window.on_next_frame(move |_, cx| {
                                let _ = app.update(cx, |_, cx| cx.notify());
                            });
                        }
                        let preview = motion.preview.as_ref();
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
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
                                        highlight,
                                        scale_keyboard,
                                        pitch_scale,
                                        window,
                                        cx,
                                    );
                                },
                            );
                            let time = Bounds::new(
                                point(grid.notes.left(), bounds.top()),
                                size(grid.notes.size.width, bounds.size.height),
                            );
                            window.with_content_mask(
                                Some(ContentMask { bounds: time }),
                                |window| {
                                    paint_time(&grid, &ds, beats, meter, resolution, window, cx);
                                    paint_ghost_notes(&grid, &ds, &ghost_notes, window);
                                    paint_notes(
                                        &grid,
                                        &ds,
                                        &clip,
                                        selected,
                                        &selected_notes,
                                        hovered_note,
                                        hovered_resize_edge,
                                        preview,
                                        velocities,
                                        active_lane,
                                        show_note_names,
                                        pitch_scale,
                                        pitch_mode,
                                        pitch_preview.as_ref(),
                                        window,
                                        cx,
                                    );
                                    #[cfg(feature = "p2p")]
                                    if let Some((alias, note)) = &peer_note {
                                        super::note_frame::paint_peer(
                                            &grid, *note, alias, &ds, window, cx,
                                        );
                                    }
                                    snap_guide.paint(
                                        grid.axis(),
                                        grid.notes,
                                        meter,
                                        &ds,
                                        window,
                                        cx,
                                    );
                                    if let Some((a, b, erase)) = drag_box {
                                        let color = if erase { ds.danger } else { ds.accent };
                                        let left = a.x.min(b.x);
                                        let top = a.y.min(b.y);
                                        let width = f32::from(a.x.max(b.x) - left);
                                        let height = f32::from(a.y.max(b.y) - top);
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(left, top),
                                                size(px(width), px(height)),
                                            ),
                                            rgba((color << 8) | 0x28),
                                        ));
                                        rectangle(
                                            window,
                                            left,
                                            top,
                                            width,
                                            1.0,
                                            (color << 8) | 0xc0,
                                        );
                                        rectangle(
                                            window,
                                            left,
                                            top + px(height),
                                            width,
                                            1.0,
                                            (color << 8) | 0xc0,
                                        );
                                        rectangle(
                                            window,
                                            left,
                                            top,
                                            1.0,
                                            height,
                                            (color << 8) | 0xc0,
                                        );
                                        rectangle(
                                            window,
                                            left + px(width),
                                            top,
                                            1.0,
                                            height,
                                            (color << 8) | 0xc0,
                                        );
                                    }
                                },
                            );
                            if velocities {
                                let header_label = format!("{} ▾", active_lane.short_label());
                                rectangle(
                                    window,
                                    bounds.left() + px(6.0),
                                    grid.velocity.top() + px(2.0),
                                    84.0,
                                    17.0,
                                    (ds.panel << 8) | 0xff,
                                );
                                rectangle(
                                    window,
                                    bounds.left() + px(6.0),
                                    grid.velocity.top() + px(2.0),
                                    84.0,
                                    1.0,
                                    (ds.border << 8) | 0x90,
                                );
                                label(
                                    window,
                                    cx,
                                    &header_label,
                                    point(bounds.left() + px(10.0), grid.velocity.top() + px(4.0)),
                                    ds.text,
                                    ds.font_size_xs - 1.0,
                                );
                                if active_lane.controller().is_none() {
                                    let contour_label = format!("∿ {}", lane_contour.label());
                                    rectangle(
                                        window,
                                        bounds.left() + px(6.0),
                                        grid.velocity.top() + px(21.0),
                                        84.0,
                                        16.0,
                                        (ds.panel << 8) | 0xff,
                                    );
                                    rectangle(
                                        window,
                                        bounds.left() + px(6.0),
                                        grid.velocity.top() + px(21.0),
                                        84.0,
                                        1.0,
                                        (ds.border << 8) | 0x90,
                                    );
                                    label(
                                        window,
                                        cx,
                                        &contour_label,
                                        point(
                                            bounds.left() + px(10.0),
                                            grid.velocity.top() + px(23.0),
                                        ),
                                        ds.cyan,
                                        ds.font_size_xs - 1.0,
                                    );
                                }
                                if active_lane.is_bipolar() {
                                    let center_y = grid.velocity.bottom()
                                        - px(0.5
                                            * (f32::from(grid.velocity.size.height) - 16.0)
                                                .max(0.0));
                                    rectangle(
                                        window,
                                        grid.notes.left(),
                                        center_y,
                                        f32::from(grid.notes.size.width),
                                        1.0,
                                        (ds.border << 8) | 0x50,
                                    );
                                }
                                for (fraction, text) in active_lane.scale_marks() {
                                    let y = grid.velocity.bottom()
                                        - px(fraction
                                            * (f32::from(grid.velocity.size.height) - 16.0)
                                                .max(0.0));
                                    label(
                                        window,
                                        cx,
                                        text,
                                        point(grid.notes.left() - px(28.0), y - px(4.0)),
                                        ds.muted,
                                        8.5,
                                    );
                                }
                            }
                            if velocities {
                                super::control_lane::paint_note_contour(
                                    &grid,
                                    &ds,
                                    &clip,
                                    active_lane,
                                    lane_contour,
                                    lane_span,
                                    window,
                                );
                                super::control_lane::paint_controls(
                                    &grid,
                                    &ds,
                                    &clip.pattern,
                                    active_lane,
                                    selected_control,
                                    window,
                                );
                            }
                            if let Some(x) = playhead.map(|beat| grid.x(beat))
                                && x >= grid.notes.left()
                                && x < grid.notes.right()
                            {
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

fn ghost_notes_for(active: &MidiClip, clips: &[MidiClip]) -> Vec<GhostNote> {
    let mut result = Vec::new();
    for clip in clips.iter().filter(|clip| clip.track == active.track) {
        let offset = clip.start_beat - active.start_beat;
        let source_start = (clip.trim_start * PPQ as f64).round() as i64;
        let source_end =
            source_start.saturating_add((clip.duration_beats() * PPQ as f64).round() as i64);
        for note in clip
            .notes()
            .iter()
            .filter(|note| !clip.muted_notes.contains(&note.id))
        {
            let start = note.start.0.max(source_start);
            let end = (note.start.0 + note.duration.0).min(source_end);
            if end <= start {
                continue;
            }
            result.push(GhostNote {
                beat: offset + (start - source_start) as f64 / PPQ as f64,
                duration: (end - start) as f64 / PPQ as f64,
                pitch: note.pitch,
            });
        }
    }
    result
}

fn paint_ghost_notes(grid: &Grid, ds: &DesignSystem, notes: &[GhostNote], window: &mut Window) {
    window.with_content_mask(Some(ContentMask { bounds: grid.notes }), |window| {
        for note in notes {
            let x = grid.x(note.beat) + px(2.0);
            let width = (note.duration as f32 * grid.pixels_per_beat - 3.0).max(2.0);
            let y = grid.y(note.pitch) + px(grid.row_height * 0.2);
            window.paint_quad(fill(
                Bounds::new(
                    point(x, y),
                    size(px(width), px((grid.row_height * 0.6).max(1.0))),
                ),
                rgba((ds.muted << 8) | 0x68),
            ));
        }
    });
}

fn paint_notes(
    grid: &Grid,
    ds: &DesignSystem,
    clip: &MidiClip,
    selected: Option<NoteId>,
    selected_notes: &HashSet<NoteId>,
    hovered_note: Option<NoteId>,
    hovered_resize_edge: bool,
    preview: Option<&NotePreview>,
    velocities: bool,
    active_lane: crate::ui::piano_roll::RollLane,
    show_note_names: bool,
    pitch_scale: PitchScale,
    pitch_mode: bool,
    pitch_preview: Option<&(NoteId, PitchCurve)>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let clip_accent = ds.clip_color(clip.track, clip.props);
    let preview_pitch_delta = match preview {
        Some(preview) if !preview.resize => {
            let (min_pitch, max_pitch) = clip
                .notes()
                .iter()
                .filter(|note| preview.notes.contains(&note.id))
                .fold((127, 0), |(min, max), note| {
                    let pitch = i32::from(note.pitch);
                    (min.min(pitch), max.max(pitch))
                });
            (-(f32::from(preview.offset.y) / grid.row_height).round() as i32)
                .clamp(-min_pitch, 127 - max_pitch)
        }
        _ => 0,
    };
    for note in clip.notes() {
        let mut bounds = grid.note_bounds(note.start, note.duration, note.pitch);
        let mut preview_pitch = note.pitch;
        let mut moving = false;
        let mut resizing = false;
        if let Some(edges) = preview.and_then(|preview| preview.neighbors.get(&note.id)) {
            bounds.origin.x += edges.offset.x;
            bounds.size.width =
                px(f32::from(bounds.size.width + edges.offset.y - edges.offset.x).max(2.0));
        }
        if let Some(preview) = preview.filter(|preview| preview.notes.contains(&note.id)) {
            if preview.resize {
                bounds.size.width = px(f32::from(bounds.size.width + preview.offset.x).max(2.0));
                resizing = preview.dragging;
            } else {
                bounds.origin += preview.offset;
                preview_pitch = (i32::from(note.pitch) + preview_pitch_delta) as u8;
                moving = preview.dragging && preview.offset != point(px(0.0), px(0.0));
            }
        }
        let x = bounds.left();
        let width = f32::from(bounds.size.width);
        let invalid = moving && !pitch_scale.contains(preview_pitch);
        let color = if clip.muted_notes.contains(&note.id) {
            ds.muted
        } else if invalid {
            ds.danger
        } else if selected == Some(note.id) || selected_notes.contains(&note.id) {
            ds.text
        } else {
            clip_accent
        };
        window.with_content_mask(Some(ContentMask { bounds: grid.notes }), |window| {
            if pitch_mode {
                let active = selected == Some(note.id);
                let color = if clip.muted_notes.contains(&note.id) {
                    ds.muted
                } else {
                    [ds.cyan, ds.warning, ds.accent, ds.pink][usize::from(note.pitch) % 4]
                };
                let curve = pitch_preview
                    .filter(|(id, _)| *id == note.id)
                    .map_or(&note.pitch_curve, |(_, curve)| curve);
                paint_pitch_curve(*grid, note, curve, color, active, window, cx);
                return;
            }
            if bounds.right() >= grid.notes.left()
                && bounds.left() <= grid.notes.right()
                && bounds.bottom() >= grid.notes.top()
                && bounds.top() <= grid.notes.bottom()
            {
                super::note_frame::paint(
                    bounds,
                    color,
                    hovered_note == Some(note.id) || resizing,
                    (hovered_note == Some(note.id) && hovered_resize_edge) || resizing,
                    ds,
                    window,
                );
                if !note.pitch_curve.points().is_empty() {
                    paint_pitch_curve(*grid, note, &note.pitch_curve, ds.cyan, false, window, cx);
                }
            }
        });
        if !pitch_mode && show_note_names && width > 30.0 {
            super::grid_background::label(
                window,
                cx,
                &elevate::musical::pitch::note_name(note.pitch),
                point(x + px(4.0), bounds.top() + px(1.0)),
                ds.panel_dark,
                ds.font_size_xs - 1.0,
            );
        }
        if velocities && active_lane.controller().is_none() {
            window.with_content_mask(
                Some(ContentMask {
                    bounds: grid.velocity,
                }),
                |window| {
                    let max_h = (f32::from(grid.velocity.size.height) - 16.0).max(0.0);
                    let frac = active_lane.note_fraction(note);
                    if active_lane.is_bipolar() {
                        let center_y = grid.velocity.bottom() - px(0.5 * max_h);
                        let delta = (frac - 0.5) * max_h;
                        let (stem_y, stem_h) = if delta >= 0.0 {
                            (center_y - px(delta), delta.max(1.0))
                        } else {
                            (center_y, (-delta).max(1.0))
                        };
                        rectangle(
                            window,
                            x + px(5.0),
                            stem_y,
                            5.0,
                            stem_h,
                            (color << 8) | 0xff,
                        );
                        rectangle(
                            window,
                            x + px(5.0),
                            if delta >= 0.0 {
                                stem_y
                            } else {
                                stem_y + px(stem_h - 1.0)
                            },
                            5.0,
                            1.0,
                            0xffffff80,
                        );
                    } else {
                        super::control_lane::paint_velocity_stem(grid, x, frac, color, window);
                    }
                },
            );
        }
    }
}

fn paint_pitch_curve(
    grid: Grid,
    note: &NoteEvent,
    curve: &PitchCurve,
    color: u32,
    selected: bool,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let flat = [
        PitchPoint { at: 0, cents: 0 },
        PitchPoint {
            at: u16::MAX,
            cents: 0,
        },
    ];
    let points = if curve.points().is_empty() {
        &flat[..]
    } else {
        curve.points()
    };
    let first = pitch_position(grid, note, points[0]);
    let end = pitch_position(grid, note, points[points.len() - 1]);
    if end.x < grid.notes.left() || first.x > grid.notes.right() {
        return;
    }
    let mut path = PathBuilder::stroke(px(if selected { 2.0 } else { 1.3 }));
    path.move_to(first);
    for &anchor in &points[1..] {
        path.line_to(pitch_position(grid, note, anchor));
    }
    let alpha = if selected { 0xff } else { 0xb0 };
    if let Ok(path) = path.build() {
        window.paint_path(path, rgba((color << 8) | alpha));
    }
    for &anchor in points.iter().take(if selected { points.len() } else { 1 }) {
        let center = pitch_position(grid, note, anchor);
        let radius = px(if selected { 3.0 } else { 2.2 });
        let bounds = Bounds::new(
            point(center.x - radius, center.y - radius),
            size(radius * 2.0, radius * 2.0),
        );
        window.paint_quad(fill(bounds, rgb(color)).corner_radii(radius));
    }
    if end.x - first.x >= px(25.0) {
        label(
            window,
            cx,
            &elevate::musical::pitch::note_name(note.pitch),
            point(first.x + px(6.0), first.y - px(14.0)),
            color,
            11.0,
        );
    }
}
