//! Sombra de inserción de archivos externos. No carga archivos ni modifica clips/cursor.
use super::{DesignSystem, Timeline, viewport::Viewport};
use gpui::{
    App, Bounds, ContentMask, Pixels, TextAlign, TextRun, Window, fill, point, px, rgb, rgba, size,
};
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct ImportPreview {
    label: String,
}

impl Timeline {
    pub(crate) fn preview_import(&mut self, paths: &[PathBuf]) {
        self.import_preview = match paths {
            [path] => {
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let kind = match extension.as_str() {
                    "mid" | "midi" => "MIDI",
                    "wav" | "wave" | "flac" | "mp3" | "ogg" | "oga" | "aac" | "m4a" | "mp4" => {
                        "Audio"
                    }
                    _ => {
                        self.import_preview = None;
                        return;
                    }
                };
                Some(ImportPreview {
                    label: format!(
                        "{kind} · {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                })
            }
            _ => None,
        };
    }
}

#[cfg(test)]
fn target(
    viewport: Viewport,
    bounds: Bounds<Pixels>,
    position: gpui::Point<Pixels>,
    height: f32,
    count: usize,
) -> Option<(usize, f64)> {
    if height <= 0.0 || !bounds.contains(&position) {
        return None;
    }
    let track = (f32::from(position.y - bounds.top()) / height).floor() as usize;
    let beat = viewport
        .beat_at(f64::from(f32::from(position.x - bounds.left())))
        .max(0.0);
    (track < count).then_some((track, beat))
}

impl ImportPreview {
    pub(super) fn paint(
        &self,
        viewport: Viewport,
        bounds: Bounds<Pixels>,
        layout: &crate::ui::track_layout::TrackLayout,
        beats_per_bar: f64,
        design: &DesignSystem,
        window: &mut Window,
        cx: &mut App,
    ) {
        let position = window.mouse_position();
        // El canvas comprende todas las pistas; solo se admite el área visible tras scroll.
        if !window.content_mask().bounds.contains(&position) {
            return;
        }
        if !bounds.contains(&position) {
            return;
        }
        let Some(row) = layout.at_y(f32::from(position.y - bounds.top())) else {
            return;
        };
        let track = row.track;
        let beat = viewport
            .beat_at(f64::from(f32::from(position.x - bounds.left())))
            .max(0.0);
        let color = design.track_color(track);
        let x = bounds.left() + px(viewport.x_at(beat) as f32);
        let y = bounds.top() + px(row.top + 4.0);
        // Marca de entrada de un compás, no una estimación de duración del archivo.
        let rect = Bounds::new(
            point(x, y),
            size(
                px((viewport.pixels_per_beat * beats_per_bar) as f32),
                px((row.height - 8.0).max(1.0)),
            ),
        );
        window.paint_quad(fill(rect, rgba((color << 8) | 0x38)));
        window.paint_quad(fill(
            Bounds::new(rect.origin, size(px(2.0), rect.size.height)),
            rgb(color),
        ));
        for offset in (0..f32::from(rect.size.width) as usize).step_by(10) {
            for y in [rect.top(), rect.bottom() - px(1.0)] {
                window.paint_quad(fill(
                    Bounds::new(point(x + px(offset as f32), y), size(px(5.0), px(1.0))),
                    rgba((color << 8) | 0xbb),
                ));
            }
        }
        let text = format!("{} · soltar aquí", self.label);
        let run = TextRun {
            len: text.len(),
            font: window.text_style().font(),
            color: rgb(design.text).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line =
            window
                .text_system()
                .shape_line(text.into(), px(design.font_size_xs), &[run], None);
        window.with_content_mask(Some(ContentMask { bounds: rect }), |window| {
            let _ = line.paint(
                point(x + px(6.0), y + px(4.0)),
                px(design.font_size_xs + 5.0),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::timeline::TimelineArea;

    #[test]
    fn preview_and_drop_agree_after_zoom_and_vertical_scroll_without_moving_cursor() {
        let mut timeline = Timeline::new(64.0);
        timeline.viewport.first_beat = 12.5;
        timeline.edit_beat = 3.0;
        let bounds = Bounds::new(point(px(220.0), px(-80.0)), size(px(800.0), px(576.0)));
        timeline.lanes_bounds.set(bounds);
        let position = point(px(353.0), px(90.0));
        let (track, beat) = target(timeline.viewport, bounds, position, 72.0, 8).unwrap();
        timeline.preview_import(&[PathBuf::from("take.MID")]);
        assert!(timeline.import_preview.is_some());
        assert_eq!(timeline.edit_beat(), 3.0);
        assert_eq!(
            timeline.place_cursor(position, TimelineArea::Lanes, 72.0, 8),
            Some(track)
        );
        assert_eq!(timeline.edit_beat(), beat);
        assert_eq!(track, 2);
        assert!(
            target(
                timeline.viewport,
                bounds,
                point(px(100.0), px(90.0)),
                72.0,
                8
            )
            .is_none()
        );
        timeline.preview_import(&[PathBuf::from("take.WAV")]);
        assert!(timeline.import_preview.is_some());
        timeline.preview_import(&[PathBuf::from("take.txt")]);
        assert!(timeline.import_preview.is_none());
        timeline.preview_import(&[PathBuf::from("one.mid"), PathBuf::from("two.wav")]);
        assert!(timeline.import_preview.is_none());
    }
}
