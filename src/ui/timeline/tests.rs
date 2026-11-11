use super::*;
use crate::audio::{AudioClip, AudioFile};
use elevate::musical::{
    clip::MidiClip,
    model::Pattern,
    time::{PPQ, Tick},
};
use gpui::{Modifiers, ScrollDelta, ScrollEvent, point, px, size};
use std::sync::Arc;

fn midi_clip(start_beat: f64, beats: i64) -> MidiClip {
    MidiClip::new(
        "clip",
        0,
        start_beat,
        Pattern::new(Tick(beats * PPQ)).expect("longitud MIDI válida"),
    )
}

fn timeline_with_lanes() -> Timeline {
    let timeline = Timeline::new(32.0);
    timeline.lanes_bounds.set(Bounds::new(
        point(px(0.0), px(0.0)),
        size(px(800.0), px(576.0)),
    ));
    timeline
}

#[test]
fn trimming_midi_edges_changes_visible_window_without_changing_source_pattern() {
    let mut timeline = timeline_with_lanes();
    let mut clip = midi_clip(4.0, 8);
    clip.pattern
        .add_note(
            elevate::musical::model::InstrumentId(1),
            Tick(0),
            Tick(PPQ),
            60,
            100,
        )
        .expect("nota válida");
    let original_pattern = clip.pattern.clone();
    timeline.midi_clip = Some(clip);

    timeline.begin_clip_drag(point(px(130.0), px(30.0)), 72.0);
    timeline.move_clip(point(px(162.0), px(30.0)), 72.0, 8);
    timeline.end_clip_drag();

    let clip = timeline.midi_clip.as_ref().unwrap();
    assert_eq!(clip.start_beat, 5.0);
    assert_eq!(clip.trim_start, 1.0);
    assert_eq!(clip.duration_beats(), 7.0);
    assert_eq!(clip.pattern.notes(), original_pattern.notes());

    timeline.begin_clip_drag(point(px(382.0), px(30.0)), 72.0);
    timeline.move_clip(point(px(224.0), px(30.0)), 72.0, 8);
    timeline.end_clip_drag();
    let clip = timeline.midi_clip.as_ref().unwrap();
    assert_eq!(clip.start_beat, 5.0);
    assert_eq!(clip.trim_start, 1.0);
    assert_eq!(clip.duration_beats(), 2.0);
    assert_eq!(clip.pattern.notes(), original_pattern.notes());
}

#[test]
fn overlapping_clips_remain_independent_when_one_is_moved_onto_another() {
    let mut timeline = timeline_with_lanes();
    timeline.insert_midi_clip(midi_clip(0.0, 4));
    timeline.insert_midi_clip(midi_clip(8.0, 4));

    // Arrastrar el clip superior desde beat 8 hasta el inicio del clip inferior.
    timeline.begin_clip_drag(point(px(288.0), px(30.0)), 72.0);
    timeline.move_clip(point(px(32.0), px(30.0)), 72.0, 8);
    timeline.end_clip_drag();

    assert_eq!(timeline.other_midi_clips.len(), 1);
    assert_eq!(timeline.other_midi_clips[0].start_beat, 0.0);
    assert_eq!(timeline.midi_clip.as_ref().unwrap().start_beat, 0.0);
    assert_eq!(timeline.other_midi_clips[0].duration_beats(), 4.0);
    assert_eq!(timeline.midi_clip.as_ref().unwrap().duration_beats(), 4.0);
}

#[test]
fn trimming_audio_keeps_the_decoded_source_and_limits_the_visible_region() {
    let mut timeline = timeline_with_lanes();
    let source = Arc::new(AudioFile {
        name: "source.wav".into(),
        sample_rate: 100,
        frames: vec![[0.25, -0.25]; 800],
        peaks: vec![[0.0, 0.25]],
        max_peak: 0.25,
    });
    timeline.clip = Some(AudioClip {
        name: source.name.clone(),
        file: source.clone(),
        track: 0,
        start_beat: 4.0,
        props: elevate::musical::clip::ClipProps::default(),
        trim_start: 0.0,
        trim_len: None,
    });

    timeline.begin_clip_drag(point(px(130.0), px(30.0)), 72.0);
    timeline.move_clip(point(px(162.0), px(30.0)), 72.0, 8);
    timeline.end_clip_drag();

    let clip = timeline.clip.as_ref().unwrap();
    assert_eq!(clip.start_beat, 5.0);
    assert_eq!(clip.trim_start, 0.5);
    assert_eq!(clip.length_beats(elevate::musical::time::DEFAULT_BPM), 15.0);
    assert!(Arc::ptr_eq(&clip.file, &source));
    assert_eq!(clip.file.frames.len(), 800);
}

#[test]
fn zoom_preserves_the_time_under_the_pointer_including_at_scale_limits() {
    let mut viewport = Viewport {
        first_beat: 100.0,
        pixels_per_beat: 32.0,
    };
    let anchor = viewport.beat_at(384.0);
    for factor in [2.0, 1000.0, 0.0001, 4.0] {
        viewport.zoom_at(384.0, factor);
        assert!((viewport.beat_at(384.0) - anchor).abs() < 1e-9);
        assert!((viewport.x_at(anchor) - 384.0).abs() < 1e-9);
        assert!((8.0..=256.0).contains(&viewport.pixels_per_beat));
    }
}
#[test]
fn ruler_and_lane_picking_agree_after_horizontal_zoom_and_vertical_scrolling() {
    let mut timeline = Timeline::new(32.0);
    timeline.viewport.first_beat = 20.0;
    timeline.viewport.zoom_at(128.0, 2.0);
    timeline.ruler_bounds.set(Bounds::new(
        point(px(200.0), px(108.0)),
        size(px(800.0), px(28.0)),
    ));
    timeline.lanes_bounds.set(Bounds::new(
        point(px(200.0), px(-8.0)),
        size(px(800.0), px(576.0)),
    ));
    assert_eq!(
        timeline.place_cursor(point(px(328.0), px(120.0)), TimelineArea::Ruler, 72.0, 8),
        None
    );
    let ruler_beat = timeline.edit_beat;
    assert_eq!(ruler_beat, 24.0);
    assert_eq!(
        timeline.place_cursor(point(px(328.0), px(140.0)), TimelineArea::Lanes, 72.0, 8),
        Some(2)
    );
    assert_eq!(timeline.edit_beat, ruler_beat);
    assert_eq!(timeline.position_label(), "7:1.00");
    assert_eq!(
        timeline.place_cursor(point(px(199.0), px(140.0)), TimelineArea::Lanes, 72.0, 8),
        None
    );
    assert_eq!(timeline.edit_beat, ruler_beat);
}
#[test]
fn keyboard_navigation_keeps_the_cursor_visible_and_stops_at_the_origin() {
    let mut timeline = Timeline::new(32.0);
    timeline.ruler_bounds.set(Bounds::new(
        point(px(200.0), px(108.0)),
        size(px(400.0), px(28.0)),
    ));
    for movement in [1.0; 100].into_iter().chain([-1.0; 110]) {
        timeline.move_cursor(movement);
        assert!((0.0..400.0).contains(&timeline.viewport.x_at(timeline.edit_beat)));
        assert!(timeline.viewport.first_beat >= 0.0);
    }
    assert_eq!(timeline.edit_beat, 0.0);
    assert_eq!(timeline.viewport.first_beat, 0.0);
}
#[test]
fn scroll_returns_vertical_motion_without_changing_the_edit_position() {
    let mut timeline = Timeline::new(32.0);
    let mut event = ScrollEvent {
        delta: ScrollDelta::Lines(point(0.0, -2.0)),
        ..Default::default()
    };
    assert_eq!(timeline.scroll(&event, TimelineArea::Lanes), -48.0);
    assert_eq!(timeline.viewport.first_beat, 0.0);
    event.modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };
    assert_eq!(timeline.scroll(&event, TimelineArea::Lanes), 0.0);
    assert!(timeline.viewport.first_beat > 0.0);
    event.modifiers = Modifiers {
        control: true,
        ..Default::default()
    };
    assert_eq!(timeline.scroll(&event, TimelineArea::Lanes), 0.0);
    assert!(timeline.viewport.pixels_per_beat < 32.0);
    assert_eq!(timeline.edit_beat, 0.0);
}
