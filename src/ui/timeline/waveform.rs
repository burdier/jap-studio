//! Waveform del clip: consume picos preparados; nunca decodifica audio.
use super::{DesignSystem, viewport::Viewport};
use crate::audio::{AudioClip, BEATS_PER_SECOND, PEAK_FRAMES};
use gpui::{
    App, Bounds, ContentMask, Pixels, TextAlign, TextRun, Window, fill, point, px, rgb, size,
};

pub(super) fn paint(
    clip: &AudioClip,
    viewport: Viewport,
    bounds: Bounds<Pixels>,
    design: &DesignSystem,
    window: &mut Window,
    cx: &mut App,
) {
    let width = f64::from(f32::from(bounds.size.width));
    let left = viewport.x_at(clip.start_beat).max(0.0);
    let right = viewport
        .x_at(clip.start_beat + clip.file.duration() * BEATS_PER_SECOND)
        .min(width);
    if right <= left {
        return;
    }
    let top = bounds.top() + px(clip.track as f32 * design.track_header_height + 3.0);
    let height = px((design.track_header_height - 6.0).max(1.0));
    let clip_bounds = Bounds::new(
        point(bounds.left() + px(left as f32), top),
        size(px((right - left) as f32), height),
    );
    window.with_content_mask(
        Some(ContentMask {
            bounds: clip_bounds,
        }),
        |window| {
            window.paint_quad(fill(clip_bounds, rgb(design.accent_dim)));
            let center = top + height * 0.6;
            let amplitude = height * 0.32;
            for column in (left.floor() as usize)..(right.ceil() as usize) {
                let frame_at = |x| {
                    ((viewport.beat_at(x) - clip.start_beat).max(0.0) / BEATS_PER_SECOND
                        * f64::from(clip.file.sample_rate)) as usize
                };
                let first_peak = (frame_at(column as f64) / PEAK_FRAMES).min(clip.file.peaks.len());
                let last_peak =
                    (frame_at(column as f64 + 1.0) / PEAK_FRAMES + 1).min(clip.file.peaks.len());
                let [low, high] = clip
                    .file
                    .peaks
                    .get(first_peak..last_peak)
                    .unwrap_or_default()
                    .iter()
                    .fold([0.0_f32, 0.0_f32], |[low, high], peak| {
                        [low.min(peak[0]), high.max(peak[1])]
                    });
                window.paint_quad(fill(
                    Bounds::new(
                        point(
                            bounds.left() + px(column as f32),
                            center - amplitude * high.clamp(0.0, 1.0),
                        ),
                        size(
                            px(1.0),
                            (amplitude * (high.clamp(0.0, 1.0) - low.clamp(-1.0, 0.0)))
                                .max(px(1.0)),
                        ),
                    ),
                    rgb(design.accent),
                ));
            }
            let name = clip.file.name.replace(['\n', '\r'], " ");
            let run = TextRun {
                len: name.len(),
                font: window.text_style().font(),
                color: rgb(design.text).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let label =
                window
                    .text_system()
                    .shape_line(name.into(), px(design.font_size_xs), &[run], None);
            if let Err(error) = label.paint(
                point(clip_bounds.left() + px(4.0), top),
                px(design.font_size_xs + 4.0),
                TextAlign::Left,
                None,
                window,
                cx,
            ) {
                eprintln!("No se pudo dibujar el nombre del clip: {error}");
            }
        },
    );
}
