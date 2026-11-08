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
        selected_track: usize,
        design: &DesignSystem,
    ) -> impl IntoElement {
        let bounds_handle = self.bounds(area).clone();
        let viewport = self.viewport;
        let edit_beat = self.edit_beat;
        let design = *design;
        let clip = self.clip.clone();
        let midi_clip = self.midi_clip.clone();
        let playhead = self.playhead;
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
                        for track in 0..track_count {
                            let top = bounds.top() + px(track as f32 * design.track_header_height);
                            let lane = Bounds::new(
                                point(bounds.left(), top),
                                size(bounds.size.width, px(design.track_header_height)),
                            );
                            let color = if track == selected_track {
                                design.panel_hover
                            } else if track % 2 == 0 {
                                design.bg
                            } else {
                                design.panel_dark
                            };
                            window.paint_quad(fill(lane, rgb(color)));
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
                        if let Some(clip) = &clip {
                            waveform::paint(clip, viewport, bounds, &design, window, cx);
                        }
                        if let Some(midi_clip) = &midi_clip {
                            waveform::paint_midi(midi_clip, viewport, bounds, &design, window, cx);
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
    }
}
