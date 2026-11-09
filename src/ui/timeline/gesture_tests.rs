use super::{Timeline, TimelineArea};
use gpui::{Bounds, PinchEvent, ScrollDelta, ScrollEvent, TouchPhase, point, px, size};

#[test]
fn pinch_and_diagonal_scroll_preserve_edit_cursor_and_time_anchor() {
    let mut timeline = Timeline::new(32.0);
    timeline.lanes_bounds.set(Bounds::new(
        point(px(200.0), px(100.0)),
        size(px(800.0), px(400.0)),
    ));
    timeline.viewport.first_beat = 10.0;
    timeline.edit_beat = 12.0;
    let anchor = timeline.viewport.beat_at(200.0);
    let mut pinch = PinchEvent {
        position: point(px(400.0), px(150.0)),
        delta: 0.25,
        ..Default::default()
    };
    timeline.pinch(&pinch, TimelineArea::Lanes);
    assert_eq!(timeline.viewport.pixels_per_beat, 40.0);
    assert_eq!(timeline.viewport.beat_at(200.0), anchor);
    let before = timeline.viewport.first_beat;
    let scroll = ScrollEvent {
        position: pinch.position,
        delta: ScrollDelta::Pixels(point(px(-20.0), px(-30.0))),
        ..Default::default()
    };
    assert_eq!(timeline.scroll(&scroll, TimelineArea::Lanes), -30.0);
    assert_eq!(timeline.viewport.first_beat, before + 0.5);
    pinch.phase = TouchPhase::Cancelled;
    timeline.pinch(&pinch, TimelineArea::Lanes);
    assert_eq!(timeline.viewport.pixels_per_beat, 40.0);
    assert_eq!(timeline.edit_beat, 12.0);
}
