//! Regla inferior en tiempo absoluto, con el mismo viewport y cursor del Arrangement.
use super::{DesignSystem, Timeline};
use gpui::{
    Bounds, ContentMask, IntoElement, Styled, TextAlign, TextRun, canvas, fill, point, px, rgb,
    size,
};

fn major_step_ms(pixels_per_second: f64, label_spacing: f64) -> u64 {
    [
        100, 200, 500, 1_000, 2_000, 5_000, 10_000, 15_000, 30_000, 60_000, 120_000, 300_000,
        600_000,
    ]
    .into_iter()
    .find(|ms| *ms as f64 / 1_000.0 * pixels_per_second >= label_spacing)
    .unwrap_or(600_000)
}

fn time_label(milliseconds: u64, fractional: bool) -> String {
    let seconds = milliseconds / 1_000;
    let base = if seconds >= 3_600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3_600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    };
    if fractional {
        format!("{base}.{:03}", milliseconds % 1_000)
    } else {
        base
    }
}

impl Timeline {
    pub(super) fn render_time_ruler(&self, design: &DesignSystem) -> impl IntoElement {
        let handle = self.time_ruler_bounds.clone();
        let viewport = self.viewport;
        let beats_per_second = self.tempo.beats_per_second();
        let cursor = self.visual_edit_beat();
        let playhead = self.visual_playhead();
        let design = design.clone();
        canvas(
            move |bounds, window, _| {
                handle.set(bounds);
                let step = major_step_ms(
                    viewport.pixels_per_beat * beats_per_second,
                    f64::from(design.font_size_xs * 8.0).max(80.0),
                );
                let minor = step / 5;
                let start_ms = viewport.first_beat / beats_per_second * 1_000.0;
                let end_ms = viewport.beat_at(f64::from(f32::from(bounds.size.width)))
                    / beats_per_second
                    * 1_000.0;
                let mut ticks = Vec::new();
                let mut labels = Vec::new();
                let mut previous_end = bounds.left();
                for index in
                    (start_ms / minor as f64).ceil() as u64..=(end_ms / minor as f64).floor() as u64
                {
                    let ms = index * minor;
                    let major = ms % step == 0;
                    let x = bounds.left()
                        + px(viewport.x_at(ms as f64 / 1_000.0 * beats_per_second) as f32);
                    ticks.push((x, major));
                    if major {
                        let text = time_label(ms, step < 1_000);
                        let run = TextRun {
                            len: text.len(),
                            font: window.text_style().font(),
                            color: rgb(design.muted).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let label = window.text_system().shape_line(
                            text.into(),
                            px(design.font_size_xs),
                            &[run],
                            None,
                        );
                        let origin = point(x + px(4.0), bounds.top() + px(8.0));
                        if origin.x >= previous_end && origin.x + label.width <= bounds.right() {
                            previous_end = origin.x + label.width + px(8.0);
                            labels.push((origin, label));
                        }
                    }
                }
                (ticks, labels)
            },
            move |bounds, (ticks, labels), window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    window.paint_quad(fill(bounds, rgb(design.panel_dark)));
                    window.paint_quad(fill(
                        Bounds::new(bounds.origin, size(bounds.size.width, px(1.0))),
                        rgb(design.border),
                    ));
                    for (x, major) in ticks {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x, bounds.top()),
                                size(px(1.0), px(if major { 7.0 } else { 3.0 })),
                            ),
                            rgb(if major { design.muted } else { design.border }),
                        ));
                    }
                    for (origin, label) in labels {
                        let _ = label.paint(
                            origin,
                            px(design.font_size_xs + 4.0),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                    for (beat, color, width) in [
                        (Some(cursor), design.accent, 1.0),
                        (playhead, design.success, 2.0),
                    ] {
                        if let Some(beat) = beat {
                            let x = viewport.x_at(beat);
                            if (0.0..f64::from(f32::from(bounds.size.width))).contains(&x) {
                                let x = bounds.left() + px(x as f32);
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(x, bounds.top()),
                                        size(px(width), bounds.size.height),
                                    ),
                                    rgb(color),
                                ));
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(x - px(3.0), bounds.top()),
                                        size(px(8.0), px(3.0)),
                                    ),
                                    rgb(color),
                                ));
                            }
                        }
                    }
                });
            },
        )
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::timeline::TimelineArea;

    #[test]
    fn time_labels_handle_minutes_hours_and_zoomed_fractions() {
        assert_eq!(time_label(0, false), "0:00");
        assert_eq!(time_label(70_000, false), "1:10");
        assert_eq!(time_label(3_600_000, false), "1:00:00");
        assert_eq!(time_label(59_800, true), "0:59.800");
        assert!(major_step_ms(16.0, 80.0) > major_step_ms(512.0, 80.0));
    }

    #[test]
    fn bottom_ruler_uses_arrangement_position_after_zoom_and_scroll() {
        let mut timeline = Timeline::new(64.0);
        timeline.viewport.first_beat = 120.0;
        timeline.time_ruler_bounds.set(Bounds::new(
            point(px(220.0), px(600.0)),
            size(px(800.0), px(28.0)),
        ));
        timeline.lanes_bounds.set(Bounds::new(
            point(px(220.0), px(-80.0)),
            size(px(800.0), px(576.0)),
        ));
        timeline.place_cursor(
            point(px(348.0), px(610.0)),
            TimelineArea::TimeRuler,
            72.0,
            8,
        );
        assert_eq!(timeline.edit_beat, 122.0);
        assert_eq!(
            time_label(
                (timeline.edit_beat / timeline.tempo.beats_per_second() * 1_000.0) as u64,
                false
            ),
            "1:01"
        );
        timeline.place_cursor(point(px(348.0), px(100.0)), TimelineArea::Lanes, 72.0, 8);
        assert_eq!(timeline.edit_beat, 122.0);
    }
}
