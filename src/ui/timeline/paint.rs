//! Regla, carriles, rejilla y cursores. La waveform se dibuja en waveform.rs.
use super::{BEATS_PER_BAR, DesignSystem, Timeline, TimelineArea, waveform};
use gpui::{
    Bounds, ContentMask, IntoElement, Styled, TextAlign, TextRun, canvas, fill, point, px, rgb,
    rgba, size,
};

impl Timeline {
    pub(crate) fn render(
        &self,
        area: TimelineArea,
        track_count: usize,
        selected_tracks: Vec<bool>,
        design: &DesignSystem,
    ) -> impl IntoElement {
        if area == TimelineArea::TimeRuler {
            return self.render_time_ruler(design).into_any_element();
        }
        let bounds_handle = self.bounds(area).clone();
        let viewport = self.viewport;
        let layout = self.layout.clone().unwrap_or_else(|| {
            crate::ui::track_layout::TrackLayout::uniform(track_count, design.track_header_height)
        });
        let import_preview = self.import_preview.clone();
        let edit_beat = self.edit_beat;
        let design = design.clone();
        let clip = self.clip.clone();
        let other_audio_clips = self.other_audio_clips.clone();
        let midi_clip = self.midi_clip.clone();
        let other_midi_clips = self.other_midi_clips.clone();
        let selected_midi = self.selected_midi.clone();
        let playhead = self.playhead;
        let hover_track = self.hover_track;
        let hover_dragging = self.hover_dragging();
        let particles = if area == TimelineArea::Lanes {
            self.particles.snapshot()
        } else {
            Vec::new()
        };
        canvas(
            move |bounds, window, _| {
                bounds_handle.set(bounds);
                let mut labels = Vec::new();
                if area == TimelineArea::Ruler {
                    let width = f64::from(f32::from(bounds.size.width));
                    let first_bar = (viewport.first_beat / BEATS_PER_BAR).ceil() as u64;
                    let last_bar = (viewport.beat_at(width) / BEATS_PER_BAR).floor() as u64;
                    let mut previous_end = bounds.left();
                    for bar in first_bar..=last_bar {
                        let x =
                            bounds.left() + px(viewport.x_at(bar as f64 * BEATS_PER_BAR) as f32);
                        let origin = point(x + px(design.padding_xs), bounds.top());
                        if origin.x < previous_end {
                            continue;
                        }
                        let text = (bar + 1).to_string();
                        let run = TextRun {
                            len: text.len(),
                            font: window.text_style().font(),
                            color: rgb(design.muted).into(),
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let line = window.text_system().shape_line(
                            text.into(),
                            px(design.font_size_xs),
                            &[run],
                            None,
                        );
                        previous_end = origin.x + line.width + px(design.padding_sm);
                        labels.push((origin, line));
                    }
                }
                labels
            },
            move |bounds, labels, window, cx| {
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    window.paint_quad(fill(
                        bounds,
                        rgb(if area == TimelineArea::Ruler {
                            design.panel
                        } else {
                            design.panel_dark
                        }),
                    ));
                    if area == TimelineArea::Lanes {
                        for row in &layout.rows {
                            let track = row.track;
                            let top = bounds.top() + px(row.top);
                            let lane = Bounds::new(
                                point(bounds.left(), top),
                                size(bounds.size.width, px(row.height)),
                            );
                            let color = if selected_tracks.get(track).copied().unwrap_or(false) {
                                design.selection_background()
                            } else if track % 2 == 0 {
                                design.bg
                            } else {
                                design.panel_dark
                            };
                            window.paint_quad(fill(lane, rgb(color)));
                            if hover_track == Some(track) {
                                let t_color = design.track_color(track);
                                let alpha = if hover_dragging { 0x32 } else { 0x12 };
                                window.paint_quad(fill(lane, rgba((t_color << 8) | alpha)));
                                if hover_dragging {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(lane.left(), lane.top()),
                                            size(lane.size.width, px(1.5)),
                                        ),
                                        rgb(t_color),
                                    ));
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(lane.left(), lane.bottom() - px(1.5)),
                                            size(lane.size.width, px(1.5)),
                                        ),
                                        rgb(t_color),
                                    ));
                                }
                            }
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(lane.left(), lane.bottom() - px(1.0)),
                                    size(lane.size.width, px(1.0)),
                                ),
                                rgb(design.border),
                            ));
                        }
                    }
                    let width = f64::from(f32::from(bounds.size.width));
                    let step = viewport.grid_step();
                    let first_tick = (viewport.first_beat / step).ceil() as u64;
                    let last_tick = (viewport.beat_at(width) / step).floor() as u64;
                    for tick in first_tick..=last_tick {
                        let beat = tick as f64 * step;
                        let is_bar = beat % BEATS_PER_BAR == 0.0;
                        let is_beat = beat.fract() == 0.0;
                        let x = bounds.left() + px(viewport.x_at(beat) as f32);
                        let height = if area == TimelineArea::Lanes {
                            bounds.size.height
                        } else if is_bar {
                            px(10.0)
                        } else {
                            px(4.0)
                        };
                        let color = if is_bar {
                            design.grid
                        } else if is_beat {
                            design.border
                        } else {
                            design.panel
                        };
                        window.paint_quad(fill(
                            Bounds::new(point(x, bounds.bottom() - height), size(px(1.0), height)),
                            rgb(color),
                        ));
                    }
                    if area == TimelineArea::Lanes {
                        for clip in &other_audio_clips {
                            waveform::paint(
                                clip, false, viewport, bounds, &layout, &design, window, cx,
                            );
                        }
                        if let Some(clip) = &clip {
                            waveform::paint(
                                clip, true, viewport, bounds, &layout, &design, window, cx,
                            );
                        }
                        for clip in &other_midi_clips {
                            waveform::paint_midi(
                                clip,
                                selected_midi.contains(&clip.id),
                                viewport,
                                bounds,
                                &layout,
                                &design,
                                window,
                                cx,
                            );
                        }
                        if let Some(midi_clip) = &midi_clip {
                            waveform::paint_midi(
                                midi_clip,
                                selected_midi.contains(&midi_clip.id),
                                viewport,
                                bounds,
                                &layout,
                                &design,
                                window,
                                cx,
                            );
                        }
                    }
                    if area == TimelineArea::Lanes && cx.has_active_drag() {
                        if let Some(preview) = &import_preview {
                            preview.paint(viewport, bounds, &layout, &design, window, cx);
                        }
                    }
                    for (origin, label) in labels {
                        if let Err(error) = label.paint(
                            origin,
                            bounds.size.height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        ) {
                            eprintln!("No se pudo dibujar la regla del timeline: {error}");
                        }
                    }
                    if area == TimelineArea::Ruler {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.left(), bounds.bottom() - px(1.0)),
                                size(bounds.size.width, px(1.0)),
                            ),
                            rgb(design.border),
                        ));
                    }
                    for particle in &particles {
                        let x = viewport.x_at(particle.beat) + particle.drift;
                        let fade = (1.0 - particle.age / particle.life).clamp(0.0, 1.0);
                        let extent = px(particle.size * fade as f32);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(
                                    bounds.left() + px(x as f32),
                                    bounds.top() + px(particle.y as f32),
                                ),
                                size(extent, extent),
                            ),
                            rgba((design.playhead_particle_color << 8) | (fade * 220.0) as u32),
                        ));
                    }
                    let cursor = viewport.x_at(edit_beat);
                    if (0.0..width).contains(&cursor) {
                        let x = bounds.left() + px(cursor as f32);
                        window.paint_quad(fill(
                            Bounds::new(point(x, bounds.top()), size(px(2.0), bounds.size.height)),
                            rgb(design.accent),
                        ));
                        if area == TimelineArea::Ruler {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x - px(3.0), bounds.bottom() - px(4.0)),
                                    size(px(8.0), px(4.0)),
                                ),
                                rgb(design.accent),
                            ));
                        }
                    }
                    if let Some(beat) = playhead {
                        let x = viewport.x_at(beat);
                        if (0.0..width).contains(&x) {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left() + px(x as f32), bounds.top()),
                                    size(px(2.0), bounds.size.height),
                                ),
                                rgb(design.success),
                            ));
                        }
                    }
                });
            },
        )
        .size_full()
        .into_any_element()
    }
}
