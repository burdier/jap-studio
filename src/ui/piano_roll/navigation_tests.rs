use super::{geometry::Grid, navigation::RollViewport};
use elevate::musical::time::{PPQ, Tick};
use gpui::{Bounds, Modifiers, ScrollDelta, ScrollEvent, point, px, size};

fn grid(view: RollViewport) -> Grid {
    Grid::new(
        Bounds::new(point(px(100.0), px(50.0)), size(px(1096.0), px(518.0))),
        16.0,
        true,
        view,
    )
}

#[test]
fn piano_roll_picking_follows_zoom_and_fractional_vertical_pan() {
    let mut view = RollViewport::default();
    let initial = grid(view);
    view.zoom_at(
        initial,
        point(initial.x(8.0), initial.notes.top()),
        16.0,
        4.0,
    );
    let zoomed = grid(view);
    view.pan(
        zoomed,
        16.0,
        -zoomed.pixels_per_beat as f64,
        zoomed.row_height as f64 * 3.25,
    );
    let moved = grid(view);
    let pointer = point(moved.x(9.0), moved.y(70) + px(moved.row_height * 0.5));
    let (tick, pitch) = moved.pick(pointer, Tick(16 * PPQ)).unwrap();
    assert_eq!(pitch, 70);
    assert!((tick.0 - 9 * PPQ).abs() <= 1);
    assert_eq!(
        moved.pick(
            point(moved.notes.left() - px(1.0), pointer.y),
            Tick(16 * PPQ)
        ),
        None
    );
}

#[test]
fn rapid_gestures_accumulate_even_without_a_render_between_events() {
    let mut view = RollViewport::default();
    let initial = grid(view);
    let pointer = point(initial.x(8.0), initial.notes.top());
    view.zoom_at(initial, pointer, 16.0, 2.0);
    view.zoom_at(initial, pointer, 16.0, 2.0);
    let zoomed = grid(view);
    assert!((f32::from(zoomed.x(8.0) - pointer.x)).abs() < 0.001);
    assert_eq!(view.zoom, 4.0);
    view.pan(zoomed, 16.0, -125.0, 0.0);
    view.pan(zoomed, 16.0, -125.0, 0.0);
    assert_eq!(view.first_beat, 7.0);
}

#[test]
fn piano_roll_navigation_stays_inside_clip_and_midi_pitch_range() {
    let mut view = RollViewport::default();
    let initial = grid(view);
    view.zoom_at(
        initial,
        point(initial.x(8.0), initial.notes.top()),
        16.0,
        1000.0,
    );
    assert_eq!(view.zoom, 64.0);
    view.pan(grid(view), 16.0, -1e8, 1e8);
    assert_eq!(view.first_beat, 15.75);
    assert_eq!(view.top_pitch, 127.0);
    view.pan(grid(view), 16.0, 1e8, -1e8);
    assert_eq!(view.first_beat, 0.0);
    assert!((view.top_pitch - 36.0).abs() < 0.001);
    view.zoom_at(grid(view), initial.notes.origin, 16.0, 0.00001);
    assert_eq!(view.zoom, 1.0);
    assert_eq!(view.first_beat, 0.0);
}

#[test]
fn super_and_ctrl_resize_only_the_requested_visual_axis() {
    let mut view = RollViewport::default();
    let initial = grid(view);
    let pointer = point(initial.x(8.0), initial.y(66) + px(initial.row_height * 0.5));
    let mut event = ScrollEvent {
        position: pointer,
        delta: ScrollDelta::Pixels(point(px(14.0), px(-60.0))),
        modifiers: Modifiers {
            platform: true,
            ..Default::default()
        },
        ..Default::default()
    };
    view.scroll(initial, 16.0, &event);
    assert!(grid(view).row_height > initial.row_height);
    assert_eq!(view.zoom, 1.0);
    assert_eq!(view.first_beat, 0.0);
    assert_eq!(
        grid(view).pick(pointer, Tick(16 * PPQ)),
        initial.pick(pointer, Tick(16 * PPQ))
    );

    let height_zoom = view.vertical_zoom;
    let top_pitch = view.top_pitch;
    event.modifiers = Modifiers {
        control: true,
        ..Default::default()
    };
    event.delta = ScrollDelta::Pixels(point(px(14.0), px(60.0)));
    view.scroll(grid(view), 16.0, &event);
    assert!(view.zoom > 1.0);
    assert_eq!(view.vertical_zoom, height_zoom);
    assert_eq!(view.top_pitch, top_pitch);
    let (tick, pitch) = grid(view).pick(pointer, Tick(16 * PPQ)).unwrap();
    assert!((tick.0 - 8 * PPQ).abs() <= 1);
    assert_eq!(pitch, 66);

    // Los gestos contrarios reducen la dimensión correspondiente.
    event.delta = ScrollDelta::Pixels(point(px(0.0), px(-60.0)));
    view.scroll(grid(view), 16.0, &event);
    assert!((view.zoom - 1.0).abs() < 1e-6);
    event.modifiers = Modifiers {
        platform: true,
        ..Default::default()
    };
    event.delta = ScrollDelta::Pixels(point(px(0.0), px(60.0)));
    view.scroll(grid(view), 16.0, &event);
    assert!((view.vertical_zoom - 1.0).abs() < 1e-6);
}

#[test]
fn vertical_zoom_and_pan_accumulate_before_repaint_without_losing_the_note_anchor() {
    let mut view = RollViewport::default();
    let initial = grid(view);
    let pointer = point(initial.x(8.0), initial.y(66) + px(initial.row_height * 0.5));
    let selection = initial.pick(pointer, Tick(16 * PPQ));
    view.zoom_vertical_at(initial, pointer, 1.5);
    view.zoom_vertical_at(initial, pointer, 1.5);
    let zoomed = grid(view);
    assert!((zoomed.row_height / initial.row_height - 2.25).abs() < 1e-5);
    assert_eq!(zoomed.pick(pointer, Tick(16 * PPQ)), selection);
    let before = view.top_pitch;
    for _ in 0..2 {
        view.pan(initial, 16.0, 0.0, f64::from(zoomed.row_height) * 0.5);
    }
    assert!((view.top_pitch - before - 1.0).abs() < 1e-5);
}

#[test]
fn vertical_zoom_limits_keep_all_rows_within_midi_range_and_reset_restores_both_axes() {
    let mut view = RollViewport::default();
    let initial = grid(view);
    view.zoom_vertical_at(initial, initial.notes.origin, 1e6);
    assert_eq!(view.vertical_zoom, 8.0);
    view.pan(grid(view), 16.0, 0.0, -1e8);
    assert!((view.top_pitch - (view.visible_rows() - 1.0)).abs() < 1e-5);
    view.zoom_vertical_at(initial, initial.notes.origin, 1e-6);
    assert_eq!(view.visible_rows(), 128.0);
    assert_eq!(view.top_pitch, 127.0);
    let small = grid(view);
    assert_eq!(small.y(127), small.notes.top());
    assert!((f32::from(small.y(0) + px(small.row_height) - small.notes.bottom())).abs() < 0.001);
    for invalid in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
        view.zoom_vertical_at(initial, initial.notes.origin, invalid);
        assert_eq!(view.visible_rows(), 128.0);
    }
    view = RollViewport::default();
    assert_eq!(view.zoom, 1.0);
    assert_eq!(view.vertical_zoom, 1.0);
    assert_eq!(grid(view).row_height, initial.row_height);
}

#[test]
fn for_clip_frames_four_bars_and_chord_register() {
    // Clip largo de 16 compases (64 negras en 4/4): encuadra 4 compases (zoom = 4.0) y ~22 filas.
    let long = RollViewport::for_clip(64.0, 4.0, &[]);
    assert!((long.zoom - 4.0).abs() < 1e-6);
    assert!((long.visible_rows() - 22.0).abs() < 0.25);
    assert_eq!(long.first_beat, 0.0);

    // Clip corto de 4 compases (16 negras): ocupa todo el ancho (zoom = 1.0).
    let short = RollViewport::for_clip(16.0, 4.0, &[]);
    assert_eq!(short.zoom, 1.0);
}
