use super::*;
use gpui::{Modifiers, ScrollDelta, ScrollEvent, point, px, size};

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
fn vertical_scroll_bubbles_but_horizontal_scroll_and_zoom_keep_the_edit_position() {
    let mut timeline = Timeline::new(32.0);
    let mut event = ScrollEvent {
        delta: ScrollDelta::Lines(point(0.0, -2.0)),
        ..Default::default()
    };
    assert!(!timeline.scroll(&event, TimelineArea::Lanes));
    assert_eq!(timeline.viewport.first_beat, 0.0);
    event.modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };
    assert!(timeline.scroll(&event, TimelineArea::Lanes));
    assert!(timeline.viewport.first_beat > 0.0);
    event.modifiers = Modifiers {
        control: true,
        ..Default::default()
    };
    assert!(timeline.scroll(&event, TimelineArea::Lanes));
    assert!(timeline.viewport.pixels_per_beat < 32.0);
    assert_eq!(timeline.edit_beat, 0.0);
}
