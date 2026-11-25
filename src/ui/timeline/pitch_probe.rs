//! Lectura local de afinación bajo el puntero, fuera del render y del motor de audio.
use super::{DesignSystem, Timeline, clip_frame};
use crate::{
    audio::{AudioFile, fundamental},
    ui::DawApp,
};
use gpui::{
    App, AppContext, Bounds, Context, Pixels, Point, SharedString, Task, TextAlign, TextRun,
    Window, fill, point, px, rgb, size,
};
use std::{ops::Range, sync::Arc};

#[derive(Clone)]
struct Segment {
    file: Arc<AudioFile>,
    frames: Range<usize>,
}

impl PartialEq for Segment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.file, &other.file) && self.frames == other.frames
    }
}

#[derive(Default)]
pub(super) struct PitchProbe {
    pub(super) pointer: Option<Point<Pixels>>,
    pub(super) label: Option<SharedString>,
    segment: Option<Segment>,
    worker: Option<Task<()>>,
}

impl Timeline {
    fn pitch_segment(&self, design: &DesignSystem) -> Option<Segment> {
        let position = self.probe.pointer?;
        let hit = self.clip_hover_at(position, design.track_header_height)?;
        let clip = self
            .clip
            .iter()
            .chain(&self.other_audio_clips)
            .find(|c| c.id == hit.id)?;
        let bounds = self.lanes_bounds.get();
        let y = f32::from(position.y - bounds.top());
        let row = self.row_at_y(y, design.track_header_height)?;
        if y - Self::lane_top(row, y) < clip_frame::title_height(design) {
            return None;
        }
        let rate = f64::from(clip.file.sample_rate);
        let beat = self.viewport.beat_at(f64::from(position.x - bounds.left()));
        let first = (clip.trim_start * rate).round() as usize;
        let end = (((clip.trim_start + clip.length_seconds()) * rate).round() as usize)
            .min(clip.file.frames.len());
        if first >= end {
            return None;
        }
        let len = ((rate * 0.14).round() as usize)
            .clamp(1, 32768)
            .min(end - first);
        let offset = ((beat - clip.start_beat).max(0.) * 60. / self.tempo.bpm() * rate) as usize;
        // Cada paso de 16 ms reutiliza su estimación al mover el puntero o durante Play.
        let hop = ((rate * 0.016).round() as usize).max(1);
        let middle = first.saturating_add(offset / hop * hop);
        let start = middle.saturating_sub(len / 2).clamp(first, end - len);
        Some(Segment {
            file: clip.file.clone(),
            frames: start..start + len,
        })
    }
}

impl DawApp {
    pub(in crate::ui) fn sync_wave_pitch(&mut self, window: &Window, cx: &mut Context<Self>) {
        let next = (self.arrangement_visible()
            && window.is_window_hovered()
            && self.split_drag.is_none()
            && !self.timeline.holds_clip_layout()
            && !cx.has_active_drag()
            && !self.timeline.hover_dragging()
            && self.track_reorder_drag.is_none()
            && self.timeline.probe.pointer == Some(window.mouse_position()))
        .then(|| self.timeline.pitch_segment(&self.ds))
        .flatten();
        let probe = &mut self.timeline.probe;
        if next != probe.segment {
            probe.segment = next;
            probe.label = None;
        }
        let Some(segment) = probe.segment.as_ref() else {
            return;
        };
        if probe.worker.is_some() || probe.label.is_some() {
            return;
        }
        let segment = segment.clone();
        let worker = cx.background_spawn(async move {
            let hz = fundamental::estimate(
                &segment.file.frames[segment.frames.clone()],
                segment.file.sample_rate,
            );
            (segment, hz)
        });
        probe.worker = Some(cx.spawn(async move |this, cx| {
            let (segment, hz) = worker.await;
            let _ = this.update(cx, |this, cx| {
                let probe = &mut this.timeline.probe;
                probe.worker = None;
                if probe.segment.as_ref() == Some(&segment) {
                    probe.label = Some(match hz {
                        Some(hz) => {
                            let midi = 69. + 12. * (hz / 440.).log2();
                            let note = elevate::musical::pitch::note_name(midi.round() as u8);
                            let cents = ((midi - midi.round()) * 100.).round() as i32;
                            format!("Fund. ≈ {hz:.1} Hz · {note} {cents:+} ¢").into()
                        }
                        None => "Sin tono estable".into(),
                    });
                }
                cx.notify();
            });
        }));
    }
}

pub(super) fn paint_label(
    pointer: Point<Pixels>,
    text: SharedString,
    bounds: Bounds<Pixels>,
    design: &DesignSystem,
    window: &mut Window,
    cx: &mut App,
) {
    if !bounds.contains(&pointer) {
        return;
    }
    let style = TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color: rgb(design.text).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text, px(design.font_size_xs), &[style], None);
    let width = line.width + px(12.);
    let height = px(design.font_size_xs + 10.);
    let x = (pointer.x + px(9.))
        .min(bounds.right() - width)
        .max(bounds.left());
    let y = if pointer.y - height - px(6.) >= bounds.top() {
        pointer.y - height - px(6.)
    } else {
        (pointer.y + px(12.)).min(bounds.bottom() - height)
    };
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(width, height)),
        rgb(design.panel_dark),
    ));
    window.paint_quad(fill(
        Bounds::new(point(x, y), size(px(2.), height)),
        rgb(design.cyan),
    ));
    let _ = line.paint(
        point(x + px(6.), y),
        height,
        TextAlign::Left,
        None,
        window,
        cx,
    );
}
