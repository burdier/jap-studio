//! Regla, carriles, rejilla y cursores. La waveform se dibuja en waveform.rs.
use super::{DesignSystem, Timeline, TimelineArea, waveform};
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
        track_defaults: Vec<(elevate::musical::track::TrackId, f32, f32)>,
        live_record: Option<super::LiveRecordPreview>,
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
        let wave_zoom = self.wave_zoom;
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
        let peak_marker = self.overlap.marker(bpm);
        let peak_selection = self.overlap.selected.clone();
        let import_preview = self.import_preview.clone();
        let edit_beat = self.visual_edit_beat();
        let design = design.clone();
        let ruler_design = design.clone();
        let clip = self.clip.clone();
        let other_audio_clips = self.other_audio_clips.clone();
        let midi_clip = self.midi_clip.clone();
        let other_midi_clips = self.other_midi_clips.clone();
        #[cfg(feature = "p2p")]
        let (clip, other_audio_clips, midi_clip, other_midi_clips) = {
            let (mut clip, mut audio, mut midi_clip, mut midi) =
                (clip, other_audio_clips, midi_clip, other_midi_clips);
            if let Some((id, alias)) = &self.peer_clip {
                for item in clip
                    .iter_mut()
                    .chain(&mut audio)
                    .filter(|c| c.id.value() == *id)
                {
                    item.name = format!("● {alias} · {}", item.name);
                }
                for item in midi_clip
                    .iter_mut()
                    .chain(&mut midi)
                    .filter(|c| c.id.value() == *id)
                {
                    item.name = format!("● {alias} · {}", item.name);
                }
            }
            (clip, audio, midi_clip, midi)
        };
        let selected_midi = self.selected_midi.clone();
        let selected_audio = self.selected_audio.clone();
        let midi_box = self.midi_box_bounds();
        let time_selection = self.visible_time_selection().map(super::loop_beats);
        let loop_guides = self.visible_loop().map(super::loop_beats);
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
        let fade_id = self
            .fade_drag
            .map(|drag| drag.id)
            .or(hover_clip.map(|h| h.id));
        let focus_span = self
            .clip
            .iter()
            .chain(&self.other_audio_clips)
            .find(|c| Some(c.id) == fade_id)
            .map(|c| (c.track, c.start_beat, c.end_beat(bpm)));
        let show_fades = move |c: &crate::audio::AudioClip| {
            Some(c.id) == fade_id
                || focus_span.is_some_and(|(track, start, end)| {
                    track == c.track
                        && ((start - c.end_beat(bpm)).abs() < 0.00001
                            || (end - c.start_beat).abs() < 0.00001)
                        && c.processing.edit.fades.iter().any(|f| f.active())
                })
        };
        let over_fade = self
            .probe
            .pointer
            .is_some_and(|p| self.fade_handle_at(p, &design).is_some());
        let pitch_label = (self.fade_drag.is_none()
            && !over_fade
            && area == TimelineArea::Lanes
            && track_visual.is_none()
            && !self.overlap.dragging())
        .then(|| self.probe.pointer.zip(self.probe.label.clone()))
        .flatten();
        let floating_color = track_visual.map(|visual| design.track_color(visual.source_track));
        let snap_guide = self.snap_guide.clone();
        let automation = self.automation.clone();
        let automation_mode = self.automation_mode;
        let automation_param = self.automation_param;
        let automation_hover = self.automation_hover;
        let automation_drag = self.automation_drag;
        let track_defaults: Vec<_> = track_defaults
            .into_iter()
            .map(
                |(id, vol_norm, pan_norm)| super::automation::PaintTrackDefaults {
                    id,
                    vol_norm,
                    pan_norm,
                },
            )
            .collect();
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
                            if row.bands > 0 {
                                let height = row.height / row.lanes as f32;
                                for band in 0..row.bands {
                                    let y =
                                        top + px((row.lanes - row.bands + band) as f32 * height);
                                    let tint = design.band_color(row.selected_band == Some(band));
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(bounds.left(), y),
                                            size(bounds.size.width, px(1.)),
                                        ),
                                        rgba((tint << 8) | 0x38),
                                    ));
                                    for (_, start, end, _) in
                                        layout.spans.iter().filter(|s| s.0 == track)
                                    {
                                        let x = viewport.x_at(*start).max(0.) as f32;
                                        let end = (viewport.x_at(*end) as f32)
                                            .min(f32::from(bounds.size.width));
                                        if end > x {
                                            window.paint_quad(fill(
                                                Bounds::new(
                                                    point(
                                                        bounds.left() + px(x),
                                                        y + px(height * 0.32),
                                                    ),
                                                    size(px(end - x), px(height * 0.36)),
                                                ),
                                                rgba((tint << 8) | 0x38),
                                            ));
                                        }
                                    }
                                }
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
                            // El velo alterno aporta unos pocos puntos de luminosidad.
                            (design.text << 8) | 0x0d,
                            window,
                        );
                    }
                    let width = f64::from(f32::from(bounds.size.width));
                    for mark in marks {
                        let (thickness, tick_height, color) = if lanes {
                            let (width, alpha) = match mark.kind {
                                MarkKind::Bar => (1.5, 0x4d),
                                MarkKind::Beat => (1.25, 0x27),
                                MarkKind::Subdivision => (1.0, 0x12),
                            };
                            (width, 0.0, (design.text << 8) | alpha)
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
                                    peak_selection.contains(&item.id)
                                        || selected_audio.contains(&item.id),
                                    show_fades(item),
                                    hover_clip.and_then(|hover| hover.edge_for(item.id)),
                                    viewport,
                                    wave_zoom,
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
                                    show_fades(item),
                                    hover_clip.and_then(|hover| hover.edge_for(item.id)),
                                    viewport,
                                    wave_zoom,
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
                                    hover_clip.and_then(|hover| hover.edge_for(item.id)),
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
                                    hover_clip.and_then(|hover| hover.edge_for(item.id)),
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
                        if let Some(rec) = live_record
                            && let Some(row) = layout.row(rec.track)
                        {
                            let left_x = viewport.x_at(rec.start_beat).max(0.0);
                            let right_x = viewport
                                .x_at(rec.end_beat.max(rec.start_beat + 0.15))
                                .min(width);
                            if right_x > left_x {
                                let rect = Bounds::new(
                                    point(
                                        bounds.left() + px(left_x as f32),
                                        bounds.top() + px(row.top + 2.0),
                                    ),
                                    size(
                                        px((right_x - left_x).max(6.0) as f32),
                                        px((row.height - 4.0).max(8.0)),
                                    ),
                                );
                                let color = if rec.counting_in {
                                    design.warning
                                } else {
                                    design.danger
                                };
                                window.paint_quad(
                                    fill(rect, rgba((color << 8) | 0x38)).corner_radii(px(4.0)),
                                );
                                window.paint_quad(fill(
                                    Bounds::new(rect.origin, size(rect.size.width, px(2.0))),
                                    rgb(color),
                                ));
                                // Barra de pico del micrófono en tiempo real dentro del clip en grabación.
                                let meter_h =
                                    ((row.height - 10.0) * rec.peak.clamp(0.0, 1.0)).max(2.0);
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(
                                            rect.right() - px(4.0),
                                            rect.bottom() - px(meter_h + 2.0),
                                        ),
                                        size(px(3.0), px(meter_h)),
                                    ),
                                    rgb(if rec.peak > 0.92 {
                                        design.warning
                                    } else {
                                        design.danger
                                    }),
                                ));
                            }
                        }
                        if automation_mode || !automation.is_empty() {
                            super::automation::paint(
                                &automation,
                                automation_mode,
                                automation_param,
                                automation_hover,
                                automation_drag,
                                &track_defaults,
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
                        if let Some((start, end)) = loop_guides {
                            for beat in [start, end] {
                                let x = viewport.x_at(beat);
                                if (0.0..=width).contains(&x) {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(bounds.left() + px(x as f32), bounds.top()),
                                            size(px(1.0), bounds.size.height),
                                        ),
                                        rgba((design.cyan << 8) | 0x38),
                                    ));
                                }
                            }
                        }
                        if let Some((start, end)) = time_selection {
                            let left = viewport.x_at(start).max(0.0);
                            let right = viewport.x_at(end).min(width);
                            if right > left {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(bounds.left() + px(left as f32), bounds.top()),
                                        size(px((right - left) as f32), bounds.size.height),
                                    ),
                                    rgba((design.accent << 8) | 0x20),
                                ));
                                for edge_beat in [start, end] {
                                    let ex = viewport.x_at(edge_beat);
                                    if (0.0..=width).contains(&ex) {
                                        window.paint_quad(fill(
                                            Bounds::new(
                                                point(bounds.left() + px(ex as f32), bounds.top()),
                                                size(px(1.5), bounds.size.height),
                                            ),
                                            rgba((design.accent << 8) | 0xcc),
                                        ));
                                    }
                                }
                            }
                        }
                        snap_guide.paint(viewport.grid(), bounds, meter, &design, window, cx);
                        if let Some(rect) = midi_box {
                            window.paint_quad(fill(rect, rgba((design.accent << 8) | 0x28)));
                            let line = px(1.);
                            for (origin, size) in [
                                (rect.origin, size(rect.size.width, line)),
                                (
                                    point(rect.left(), rect.bottom() - line),
                                    size(rect.size.width, line),
                                ),
                                (rect.origin, size(line, rect.size.height)),
                                (
                                    point(rect.right() - line, rect.top()),
                                    size(line, rect.size.height),
                                ),
                            ] {
                                window.paint_quad(fill(
                                    Bounds::new(origin, size),
                                    rgb(design.accent),
                                ));
                            }
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
                            Bounds::new(point(x, bounds.top()), size(px(1.0), bounds.size.height)),
                            rgba((design.accent << 8) | 0xa0),
                        ));
                        if area == TimelineArea::Ruler {
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(x - px(2.0), bounds.bottom() - px(3.0)),
                                    size(px(5.0), px(3.0)),
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
                    if lanes && let Some((beat, rows, over)) = &peak_marker {
                        let x = viewport.x_at(*beat);
                        if (0.0..width).contains(&x) {
                            for &(track, id) in rows {
                                if let Some(row) = layout.clip_row(track, id) {
                                    let left = bounds.left() + px(x as f32);
                                    let top = bounds.top()
                                        + px(row.top
                                            + super::clip_frame::title_height(&design)
                                            + 2.);
                                    let color = if *over { design.danger } else { design.warning };
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(left, top),
                                            size(
                                                px(1.),
                                                px((row.height
                                                    - super::clip_frame::title_height(&design)
                                                    - 5.)
                                                    .max(1.)),
                                            ),
                                        ),
                                        rgba((color << 8) | 0xaf),
                                    ));
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(left - px(2.), top),
                                            size(px(5.), px(3.)),
                                        ),
                                        rgb(color),
                                    ));
                                }
                            }
                        }
                    }
                    if let Some((pointer, label)) = pitch_label {
                        super::pitch_probe::paint_label(
                            pointer, label, bounds, &design, window, cx,
                        );
                    }
                });
            },
        )
        .size_full()
        .into_any_element()
    }
}
