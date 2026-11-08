//! Dibujo de clips de audio y MIDI en el Arrangement. Nunca decodifica audio.
use super::{DesignSystem, viewport::Viewport};
use crate::audio::{AudioClip, BEATS_PER_SECOND, PEAK_FRAMES};
use elevate::musical::{clip::MidiClip, time::PPQ};
use gpui::{
    App, Bounds, ContentMask, Pixels, TextAlign, TextRun, Window, fill, point, px, rgb, rgba, size,
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
    let track_colors = [
        design.accent,
        design.success,
        design.warning,
        design.danger,
        design.cyan,
        design.orange,
        design.pink,
        design.accent,
    ];
    let track_color = track_colors[clip.track.min(track_colors.len() - 1)];
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
            // Fondo gris pizarra neutro + barra superior del color de la pista
            window
                .paint_quad(fill(clip_bounds, rgba(0x32363de8)).corner_radii(px(design.radius_sm)));
            window.paint_quad(fill(
                Bounds::new(clip_bounds.origin, size(clip_bounds.size.width, px(2.0))),
                rgb(track_color),
            ));

            let header_h = design.font_size_xs + 4.0;
            let wave_top = f32::from(top) + header_h;
            let wave_h = (f32::from(height) - header_h - 2.0).max(8.0);
            let center_y = wave_top + wave_h * 0.5;
            let amplitude = wave_h * 0.46;
            let norm_scale = 1.0 / clip.file.max_peak.max(0.05);

            // Línea central horizontal continua (zero-crossing baseline)
            window.paint_quad(fill(
                Bounds::new(
                    point(clip_bounds.left(), px(center_y - 0.75)),
                    size(clip_bounds.size.width, px(1.5)),
                ),
                rgb(design.text),
            ));

            // Agujas verticales discretas (estilo transientes con micro-separación)
            let start_col = left.floor() as usize;
            let end_col = right.ceil() as usize;
            let mut column = start_col;
            while column < end_col {
                let frame_at = |x: f64| {
                    ((viewport.beat_at(x) - clip.start_beat).max(0.0) / BEATS_PER_SECOND
                        * f64::from(clip.file.sample_rate)) as usize
                };
                let first_peak = (frame_at(column as f64) / PEAK_FRAMES).min(clip.file.peaks.len());
                let last_peak =
                    (frame_at((column + 2) as f64) / PEAK_FRAMES + 1).min(clip.file.peaks.len());
                let slice = clip
                    .file
                    .peaks
                    .get(first_peak..last_peak)
                    .unwrap_or_default();
                if !slice.is_empty() {
                    let mut min_v = 0.0_f32;
                    let mut max_v = 0.0_f32;
                    let mut sum_pos = 0.0_f32;
                    let mut sum_neg = 0.0_f32;
                    for &[lo, hi] in slice {
                        min_v = min_v.min(lo);
                        max_v = max_v.max(hi);
                        sum_neg += lo.abs();
                        sum_pos += hi.abs();
                    }
                    let count = slice.len() as f32;
                    let hi_peak = (max_v * norm_scale).clamp(0.0, 1.0).powf(0.85) * amplitude;
                    let lo_peak = (min_v.abs() * norm_scale).clamp(0.0, 1.0).powf(0.85) * amplitude;
                    let hi_rms =
                        ((sum_pos / count) * norm_scale).clamp(0.0, 1.0).powf(0.85) * amplitude;
                    let lo_rms =
                        ((sum_neg / count) * norm_scale).clamp(0.0, 1.0).powf(0.85) * amplitude;

                    let x_pos = bounds.left() + px(column as f32);
                    // Espiga exterior de pico (transiente fino)
                    if hi_peak + lo_peak >= 0.8 {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x_pos, px(center_y - hi_peak)),
                                size(px(1.15), px((hi_peak + lo_peak).max(1.5))),
                            ),
                            rgba((design.text << 8) | 0xdc),
                        ));
                    }
                    // Núcleo interior de energía (RMS más denso y brillante)
                    if hi_rms + lo_rms >= 0.8 {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x_pos, px(center_y - hi_rms)),
                                size(px(1.55), px((hi_rms + lo_rms).max(1.8))),
                            ),
                            rgb(0xffffff),
                        ));
                    }
                }
                column += 2;
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
                point(clip_bounds.left() + px(4.0), top + px(1.0)),
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

pub(super) fn paint_midi(
    clip: &MidiClip,
    viewport: Viewport,
    bounds: Bounds<Pixels>,
    design: &DesignSystem,
    window: &mut Window,
    cx: &mut App,
) {
    let width = f64::from(f32::from(bounds.size.width));
    let left = viewport.x_at(clip.start_beat).max(0.0);
    let right = viewport.x_at(clip.end_beat()).min(width);
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
            window.paint_quad(fill(clip_bounds, rgb(design.panel_hover)));
            window.paint_quad(fill(
                Bounds::new(clip_bounds.origin, size(clip_bounds.size.width, px(2.0))),
                rgb(design.cyan),
            ));
            let notes = clip.notes();
            let min_pitch = notes.iter().map(|n| n.pitch).min().unwrap_or(48);
            let max_pitch = notes.iter().map(|n| n.pitch).max().unwrap_or(84);
            let span = f32::from((max_pitch.saturating_sub(min_pitch)).max(12));
            let inner_top = f32::from(top) + design.font_size_xs + 6.0;
            let inner_height = (f32::from(height) - (design.font_size_xs + 10.0)).max(6.0);
            for note in notes {
                let start_beat = clip.start_beat + note.start.0 as f64 / PPQ as f64;
                let end_beat = start_beat + (note.duration.0 as f64 / PPQ as f64).max(0.05);
                let x0 = viewport.x_at(start_beat).max(left);
                let x1 = viewport.x_at(end_beat).min(right);
                if x1 <= x0 {
                    continue;
                }
                let norm = 1.0 - f32::from(note.pitch.saturating_sub(min_pitch)) / span;
                let y = inner_top + norm.clamp(0.0, 1.0) * (inner_height - 4.0);
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left() + px(x0 as f32), px(y)),
                        size(px(((x1 - x0) as f32).max(2.0)), px(3.0)),
                    ),
                    rgb(design.cyan),
                ));
            }
            let title = format!("🎹 {} ({} notas)", clip.name, notes.len());
            let run = TextRun {
                len: title.len(),
                font: window.text_style().font(),
                color: rgb(design.text).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let label = window.text_system().shape_line(
                title.into(),
                px(design.font_size_xs),
                &[run],
                None,
            );
            let _ = label.paint(
                point(clip_bounds.left() + px(4.0), top + px(2.0)),
                px(design.font_size_xs + 4.0),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        },
    );
}
