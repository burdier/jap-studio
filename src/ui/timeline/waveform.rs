//! Dibujo de clips de audio y MIDI en el Arrangement.
use super::{DesignSystem, clip_frame, viewport::Viewport};
use crate::audio::{AudioClip, PEAK_FRAMES};
use elevate::musical::{
    clip::{ClipProps, MidiClip},
    time::PPQ,
};
use gpui::{
    App, Bounds, ContentMask, Pixels, TextAlign, TextRun, Window, fill, point, px, rgb, rgba, size,
};

/// Controles compactos dentro del clip, junto al nombre.
fn paint_clip_control_pill(
    clip_left: Pixels,
    clip_top: Pixels,
    clip_width: Pixels,
    props: ClipProps,
    accent_color: u32,
    selected: bool,
    design: &DesignSystem,
    window: &mut Window,
    cx: &mut App,
) {
    if f32::from(clip_width) < clip_frame::CONTROLS_MIN_WIDTH {
        return;
    }
    let pill_bounds = Bounds::new(
        point(
            clip_left + px(clip_frame::CONTROL_LEFT),
            clip_top + px(clip_frame::CONTROL_TOP),
        ),
        size(
            px(clip_frame::CONTROL_WIDTH),
            px(clip_frame::CONTROL_HEIGHT),
        ),
    );
    let edge = px(1.0);
    let frame = rgba((0xe3978c << 8) | if selected { 0xd0 } else { 0x85 });
    for bounds in [
        Bounds::new(pill_bounds.origin, size(pill_bounds.size.width, edge)),
        Bounds::new(
            point(pill_bounds.left(), pill_bounds.bottom() - edge),
            size(pill_bounds.size.width, edge),
        ),
        Bounds::new(pill_bounds.origin, size(edge, pill_bounds.size.height)),
        Bounds::new(
            point(pill_bounds.right() - edge, pill_bounds.top()),
            size(edge, pill_bounds.size.height),
        ),
    ] {
        window.paint_quad(fill(bounds, frame));
    }

    let items: [(&str, bool, u32); 4] = [
        ("i", selected, accent_color),
        ("M", props.muted, design.danger),
        ("L", props.locked, design.warning),
        ("⟳", props.looped, design.cyan),
    ];
    for (idx, (glyph, active, active_col)) in items.into_iter().enumerate() {
        let slot_x = f32::from(pill_bounds.left()) + idx as f32 * clip_frame::CONTROL_SLOT + 2.0;
        if slot_x + 18.0 > f32::from(pill_bounds.right()) {
            break;
        }
        let text_col = if active { active_col } else { design.text };
        let run = TextRun {
            len: glyph.len(),
            font: window.text_style().font(),
            color: rgb(text_col).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = window
            .text_system()
            .shape_line(glyph.into(), px(10.0), &[run], None);
        let _ = shaped.paint(
            point(px(slot_x + 5.0), pill_bounds.top() + px(1.5)),
            px(12.0),
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

pub(super) fn paint(
    clip: &AudioClip,
    bpm: f64,
    selected: bool,
    hover_edge: Option<clip_frame::ClipEdge>,
    viewport: Viewport,
    bounds: Bounds<Pixels>,
    layout: &crate::ui::track_layout::TrackLayout,
    design: &DesignSystem,
    window: &mut Window,
    cx: &mut App,
) {
    let width = f64::from(f32::from(bounds.size.width));
    let left = viewport.x_at(clip.start_beat).max(0.0);
    let right = viewport.x_at(clip.end_beat(bpm)).min(width);
    if right <= left {
        return;
    }
    let track_color = design.clip_color(clip.track, clip.props);
    let Some(row) = layout.row(clip.track) else {
        return;
    };
    let lane_top = bounds.top() + px(row.top);
    let body_top = lane_top;
    let body_height = px((row.height - 3.0).max(12.0));
    let clip_left = bounds.left() + px(left as f32);
    let clip_width = px((right - left) as f32);
    let clip_bounds = Bounds::new(point(clip_left, body_top), size(clip_width, body_height));

    window.with_content_mask(
        Some(ContentMask {
            bounds: clip_bounds,
        }),
        |window| {
            clip_frame::paint_frame_without_top_border(
                clip_bounds,
                track_color,
                selected,
                clip.props.muted,
                hover_edge,
                design,
                window,
            );

            let header_h = clip_frame::title_height(design) + 1.0;
            let wave_top = f32::from(body_top) + header_h;
            let wave_h = (f32::from(body_height) - header_h - 2.0).max(8.0);
            let center_y = wave_top + wave_h * 0.5;
            let gain_scale = clip.props.gain_linear().clamp(0.0, 2.5);
            let amplitude = wave_h * 0.46 * gain_scale.min(1.35);
            let norm_scale = 1.0 / clip.file.max_peak.max(0.05);

            window.paint_quad(fill(
                Bounds::new(
                    point(clip_bounds.left() + px(2.0), px(center_y - 0.75)),
                    size((clip_bounds.size.width - px(4.0)).max(px(1.0)), px(1.2)),
                ),
                rgb(if clip.props.muted {
                    design.muted
                } else {
                    track_color
                }),
            ));

            let start_col = left.floor() as usize;
            let end_col = right.ceil() as usize;
            let mut column = start_col;
            while column < end_col {
                let frame_at = |x: f64| {
                    (((viewport.beat_at(x) - clip.start_beat).max(0.0) / (bpm / 60.0)
                        + clip.trim_start)
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
                    let wave_col = if clip.props.muted {
                        design.muted
                    } else {
                        track_color
                    };
                    if hi_peak + lo_peak >= 0.8 {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x_pos, px(center_y - hi_peak)),
                                size(px(1.15), px((hi_peak + lo_peak).max(1.5))),
                            ),
                            rgba((wave_col << 8) | 0xdc),
                        ));
                    }
                    if hi_rms + lo_rms >= 0.8 {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x_pos, px(center_y - hi_rms)),
                                size(px(1.55), px((hi_rms + lo_rms).max(1.8))),
                            ),
                            rgb(if clip.props.muted {
                                design.muted
                            } else {
                                design.text
                            }),
                        ));
                    }
                }
                column += 2;
            }

            let mut name = clip.name.replace(['\n', '\r'], " ");
            if clip.props.gain_db.abs() >= 0.1 {
                name.push_str(&format!(" [{:+.1}dB]", clip.props.gain_db));
            }
            if clip.props.pitch_semitones != 0 {
                name.push_str(&format!(" [{:+}st]", clip.props.pitch_semitones));
            }
            let run = TextRun {
                len: name.len(),
                font: window.text_style().font(),
                color: rgb(if clip.props.muted {
                    design.muted
                } else {
                    design.text
                })
                .into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let label =
                window
                    .text_system()
                    .shape_line(name.into(), px(design.font_size_xs), &[run], None);
            let _ = label.paint(
                point(
                    clip_bounds.left()
                        + px(if f32::from(clip_width) >= clip_frame::CONTROLS_MIN_WIDTH {
                            102.0
                        } else {
                            6.0
                        }),
                    body_top + px(3.0),
                ),
                px(design.font_size_xs + 4.0),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        },
    );

    paint_clip_control_pill(
        clip_left,
        body_top,
        clip_width,
        clip.props,
        track_color,
        selected,
        design,
        window,
        cx,
    );
}

pub(super) fn paint_midi(
    clip: &MidiClip,
    selected: bool,
    hover_edge: Option<clip_frame::ClipEdge>,
    viewport: Viewport,
    bounds: Bounds<Pixels>,
    layout: &crate::ui::track_layout::TrackLayout,
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
    let clip_color = design.clip_color(clip.track, clip.props);
    let Some(row) = layout.row(clip.track) else {
        return;
    };
    let lane_top = bounds.top() + px(row.top);
    let body_top = lane_top;
    let body_height = px((row.height - 3.0).max(12.0));
    let clip_left = bounds.left() + px(left as f32);
    let clip_width = px((right - left) as f32);
    let clip_bounds = Bounds::new(point(clip_left, body_top), size(clip_width, body_height));

    window.with_content_mask(
        Some(ContentMask {
            bounds: clip_bounds,
        }),
        |window| {
            clip_frame::paint_frame_without_top_border(
                clip_bounds,
                clip_color,
                selected,
                clip.props.muted,
                hover_edge,
                design,
                window,
            );

            let notes = clip.notes();
            let min_pitch = notes.iter().map(|n| n.pitch).min().unwrap_or(48);
            let max_pitch = notes.iter().map(|n| n.pitch).max().unwrap_or(84);
            let span = f32::from((max_pitch.saturating_sub(min_pitch)).max(12));
            let inner_top = f32::from(body_top) + clip_frame::title_height(design) + 3.0;
            let inner_height =
                (f32::from(body_height) - (clip_frame::title_height(design) + 5.0)).max(6.0);
            let note_color = if clip.props.muted {
                design.muted
            } else {
                clip_color
            };
            for note in notes {
                let start_beat =
                    clip.start_beat + note.start.0 as f64 / PPQ as f64 - clip.trim_start;
                let end_beat = start_beat + (note.duration.0 as f64 / PPQ as f64).max(0.05);
                let x0 = viewport.x_at(start_beat).max(left);
                let x1 = viewport.x_at(end_beat).min(right);
                if x1 <= x0 {
                    continue;
                }
                let norm = 1.0 - f32::from(note.pitch.saturating_sub(min_pitch)) / span;
                let y = inner_top + norm.clamp(0.0, 1.0) * (inner_height - 4.0);
                window.paint_quad(
                    fill(
                        Bounds::new(
                            point(bounds.left() + px(x0 as f32), px(y)),
                            size(px(((x1 - x0) as f32).max(2.0)), px(3.0)),
                        ),
                        rgb(note_color),
                    )
                    .corner_radii(px(1.5)),
                );
            }
            let mut title = format!("🎹 {} ({})", clip.name, notes.len());
            if clip.props.gain_db.abs() >= 0.1 {
                title.push_str(&format!(" {:+.1}dB", clip.props.gain_db));
            }
            if clip.props.pitch_semitones != 0 {
                title.push_str(&format!(" {:+}st", clip.props.pitch_semitones));
            }
            let run = TextRun {
                len: title.len(),
                font: window.text_style().font(),
                color: rgb(if clip.props.muted {
                    design.muted
                } else {
                    design.text
                })
                .into(),
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
                point(
                    clip_bounds.left()
                        + px(if f32::from(clip_width) >= clip_frame::CONTROLS_MIN_WIDTH {
                            102.0
                        } else {
                            6.0
                        }),
                    body_top + px(3.0),
                ),
                px(design.font_size_xs + 4.0),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        },
    );

    paint_clip_control_pill(
        clip_left, body_top, clip_width, clip.props, clip_color, selected, design, window, cx,
    );
}
