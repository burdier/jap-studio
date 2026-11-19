//! Regla, carriles, rejilla y cursores. La waveform se dibuja en waveform.rs.
use super::{DesignSystem, DragKind, Timeline, TimelineArea, waveform};
use crate::ui::track_drag::TrackReorderVisual;
use crate::ui::{
    components::bar_grid,
    time_grid::musical::{self, MarkKind},
};
use gpui::{
    Bounds, ContentMask, IntoElement, Styled, TextAlign, canvas, fill, point, px, rgb, rgba, size,
};

impl Timeline {
    pub(crate) fn render(
        &self,
        area: TimelineArea,
        track_count: usize,
        selected_tracks: Vec<bool>,
        track_visual: Option<&TrackReorderVisual>,
        hilo_names: Vec<(elevate::musical::track::TrackId, String)>,
        design: &DesignSystem,
    ) -> impl IntoElement {
        if area == TimelineArea::TimeRuler {
            return self.render_time_ruler(design).into_any_element();
        }
        let bounds_handle = self.bounds(area).clone();
        let viewport = self.viewport;
        let beats_per_bar = self.meter.beats_per_bar();
        let meter = self.meter;
        let bpm = self.tempo.bpm();
        let original_layout = self.layout.clone().unwrap_or_else(|| {
            crate::ui::track_layout::TrackLayout::uniform(track_count, design.track_header_height)
        });
        let layout =
            track_visual.map_or_else(|| original_layout.clone(), |visual| visual.layout.clone());
        let dragged_rows = track_visual.map_or_else(Vec::new, |visual| visual.source_rows.clone());
        let floating_layout = track_visual.map(|visual| {
            let mut layout = original_layout.clone();
            for row in &mut layout.rows {
                if dragged_rows.contains(&row.track) {
                    row.top += visual.float_offset;
                }
            }
            layout
        });
        let floating_area =
            track_visual.map(|visual| (visual.floating_top, visual.floating_height));
        let import_preview = self.import_preview.clone();
        let edit_beat = self.visual_edit_beat();
        let design = design.clone();
        let ruler_design = design.clone();
        let clip = self.clip.clone();
        let other_audio_clips = self.other_audio_clips.clone();
        let midi_clip = self.midi_clip.clone();
        let other_midi_clips = self.other_midi_clips.clone();
        let selected_midi = self.selected_midi.clone();
        let playhead = self.visual_playhead();
        let hover_track = if track_visual.is_some() {
            None
        } else {
            self.hover_track
        };
        let hover_dragging = track_visual.is_none() && self.hover_dragging();
        let hover_clip = if track_visual.is_some() {
            None
        } else {
            self.hover_clip
        };
        let floating_color = track_visual.map(|visual| design.track_color(visual.source_track));
        let snap_guide = self.snap_guide.clone();
        let particles = if area == TimelineArea::Lanes {
            self.particles.snapshot()
        } else {
            Vec::new()
        };
        canvas(
            move |bounds, window, _| {
                bounds_handle.set(bounds);
                let width = f64::from(f32::from(bounds.size.width));
                let marks = musical::marks(
                    viewport.grid(),
                    meter,
                    width,
                    f64::from(ruler_design.font_size_xs),
                    None,
                );
                let labels = if area == TimelineArea::Ruler {
                    bar_grid::ruler_labels(bounds, viewport.grid(), &marks, &ruler_design, window)
                } else {
                    Vec::new()
                };
                (marks, labels)
            },
            move |bounds, (marks, labels), window, cx| {
                let surface = design.arrangement_surface();
                let lanes = area == TimelineArea::Lanes;
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    window.paint_quad(fill(
                        bounds,
                        rgb(if lanes {
                            surface
                        } else {
                            design.arrangement_header()
                        }),
                    ));
                    if lanes {
                        for row in &layout.rows {
                            let track = row.track;
                            let top = bounds.top() + px(row.top);
                            let lane = Bounds::new(
                                point(bounds.left(), top),
                                size(bounds.size.width, px(row.height)),
                            );
                            let color = if dragged_rows.contains(&track) {
                                Some(design.arrangement_header())
                            } else if selected_tracks.get(track).copied().unwrap_or(false) {
                                Some(design.arrangement_selection(surface))
                            } else {
                                None
                            };
                            if let Some(color) = color {
                                window.paint_quad(fill(lane, rgb(color)));
                            }
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
                                rgba((design.text << 8) | 0x23),
                            ));
                        }
                    }
                    // Evitar una textura de rayas en compases muy cortos al alejar.
                    if beats_per_bar * viewport.pixels_per_beat >= 8.0 {
                        bar_grid::paint_bands(
                            bounds,
                            viewport.grid(),
                            beats_per_bar,
                            (design.text << 8) | 0x0b,
                            window,
                        );
                    }
                    let width = f64::from(f32::from(bounds.size.width));
                    for mark in marks {
                        let (thickness, tick_height, color) = if lanes {
                            let alpha = match mark.kind {
                                MarkKind::Bar => 0x49,
                                MarkKind::Beat => 0x17,
                                MarkKind::Subdivision => 0x0a,
                            };
                            (1.0, 0.0, (design.text << 8) | alpha)
                        } else {
                            bar_grid::mark_style(mark.kind, &design)
                        };
                        let x = bounds.left() + px(viewport.x_at(mark.beat).round() as f32);
                        let height = if lanes {
                            bounds.size.height
                        } else {
                            px(tick_height)
                        };
                        window.paint_quad(fill(
                            Bounds::new(
                                point(x, bounds.bottom() - height),
                                size(px(thickness), height),
                            ),
                            rgba(color),
                        ));
                    }
                    if area == TimelineArea::Lanes {
                        for floating in [false, true] {
                            let clip_layout = if floating {
                                let Some(layout) = floating_layout.as_ref() else {
                                    continue;
                                };
                                if let Some((top, height)) = floating_area {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(bounds.left(), bounds.top() + px(top)),
                                            size(bounds.size.width, px(height)),
                                        ),
                                        rgb(design.panel_hover),
                                    ));
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(bounds.left(), bounds.top() + px(top)),
                                            size(bounds.size.width, px(1.0)),
                                        ),
                                        rgba((floating_color.unwrap_or(design.accent) << 8) | 0xb0),
                                    ));
                                }
                                layout
                            } else {
                                &layout
                            };
                            for item in other_audio_clips
                                .iter()
                                .filter(|item| dragged_rows.contains(&item.track) == floating)
                            {
                                waveform::paint(
                                    item,
                                    bpm,
                                    false,
                                    hover_clip.and_then(|hover| {
                                        hover.edge_for(DragKind::Audio, item.track, item.start_beat)
                                    }),
                                    viewport,
                                    bounds,
                                    clip_layout,
                                    &design,
                                    window,
                                    cx,
                                );
                            }
                            if let Some(item) = clip
                                .as_ref()
                                .filter(|item| dragged_rows.contains(&item.track) == floating)
                            {
                                waveform::paint(
                                    item,
                                    bpm,
                                    true,
                                    hover_clip.and_then(|hover| {
                                        hover.edge_for(DragKind::Audio, item.track, item.start_beat)
                                    }),
                                    viewport,
                                    bounds,
                                    clip_layout,
                                    &design,
                                    window,
                                    cx,
                                );
                            }
                            for item in other_midi_clips
                                .iter()
                                .filter(|item| dragged_rows.contains(&item.track) == floating)
                            {
                                waveform::paint_midi(
                                    item,
                                    selected_midi.contains(&item.id),
                                    hover_clip.and_then(|hover| {
                                        hover.edge_for(DragKind::Midi, item.track, item.start_beat)
                                    }),
                                    viewport,
                                    bounds,
                                    clip_layout,
                                    &hilo_names,
                                    &design,
                                    window,
                                    cx,
                                );
                            }
                            if let Some(item) = midi_clip
                                .as_ref()
                                .filter(|item| dragged_rows.contains(&item.track) == floating)
                            {
                                waveform::paint_midi(
                                    item,
                                    selected_midi.contains(&item.id),
                                    hover_clip.and_then(|hover| {
                                        hover.edge_for(DragKind::Midi, item.track, item.start_beat)
                                    }),
                                    viewport,
                                    bounds,
                                    clip_layout,
                                    &hilo_names,
                                    &design,
                                    window,
                                    cx,
                                );
                            }
                        }
                    }
                    if area == TimelineArea::Lanes && cx.has_active_drag() {
                        if let Some(preview) = &import_preview {
                            preview.paint(
                                viewport,
                                bounds,
                                &layout,
                                beats_per_bar,
                                &design,
                                window,
                                cx,
                            );
                        }
                    }
                    if area == TimelineArea::Lanes {
                        snap_guide.paint(viewport.grid(), bounds, meter, &design, window, cx);
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
