//! Ganancia de instancia y pico estéreo de los clips en un tramo común, antes del mezclador.
use super::{DesignSystem, Timeline, clip_frame};
use crate::{
    audio::{AudioClip, AudioFile, export::resample},
    ui::DawApp,
};
use elevate::musical::clip::ClipId;
use gpui::{AppContext, Context, Pixels, Point, Task, Window};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy)]
struct GainDrag {
    id: ClipId,
    y: Pixels,
    db: f32,
    beat: f64,
}

#[derive(Clone)]
struct Source {
    file: Arc<AudioFile>,
    id: ClipId,
    track: usize,
    start: usize,
    offset: usize,
    gain: f32,
    fades: Vec<crate::audio::crossfade::Ramp>,
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.track == other.track
            && Arc::ptr_eq(&self.file, &other.file)
            && self.start == other.start
            && self.offset == other.offset
            && self.gain == other.gain
            && self.fades == other.fades
    }
}

#[derive(Clone, PartialEq)]
struct Region {
    frames: Range<usize>,
    sources: Vec<Source>,
    rate: u32,
}

struct Converted {
    file: Arc<AudioFile>,
    rate: u32,
    frames: Vec<[f32; 2]>,
}

#[derive(Default)]
pub(in crate::ui) struct Overlap {
    drag: Option<GainDrag>,
    pub(super) selected: Vec<ClipId>,
    region: Option<Region>,
    peak: Option<(usize, f32)>,
    error: Option<String>,
    worker: Option<Task<()>>,
    cancel: Option<Arc<AtomicBool>>,
    cache: Vec<Converted>,
}

impl Overlap {
    pub(super) fn finish_gain(&mut self) -> bool {
        self.drag.take().is_some()
    }
    pub(super) fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub(in crate::ui) fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub(in crate::ui) fn readout(&self) -> Option<(String, String, bool)> {
        let Some(region) = &self.region else {
            return match self.selected.len() {
                0 => None,
                1 => Some(("Medir suma".into(), "Ctrl+clic en otro clip".into(), false)),
                _ => Some(("Sin tramo común".into(), "Selección de audio".into(), false)),
            };
        };
        if let Some(error) = &self.error {
            return Some(("Pico no disponible".into(), error.clone(), true));
        }
        let Some((frame, peak)) = self.peak else {
            return Some(("Midiendo suma…".into(), "Antes de FX".into(), false));
        };
        let db = if peak > 0. {
            format!("{:+.1}", 20. * peak.log10())
        } else {
            "−∞".into()
        };
        let ms = (frame as u64 * 1000 + u64::from(region.rate) / 2) / u64::from(region.rate);
        Some((
            format!("Pico suma  {db} dBFS"),
            format!(
                "{}:{:02}.{:03} · antes de FX",
                ms / 60000,
                ms / 1000 % 60,
                ms % 1000
            ),
            peak > 1.,
        ))
    }

    pub(super) fn marker(&self, bpm: f64) -> Option<(f64, Vec<(usize, ClipId)>, bool)> {
        let region = self.region.as_ref()?;
        let (frame, peak) = self.peak?;
        Some((
            frame as f64 / f64::from(region.rate) * bpm / 60.,
            region.sources.iter().map(|s| (s.track, s.id)).collect(),
            peak > 1.,
        ))
    }
}

impl Timeline {
    fn wave_clips(&self) -> impl Iterator<Item = &AudioClip> + Clone {
        self.clip.iter().chain(&self.other_audio_clips)
    }

    fn gain_at(&self, at: Point<Pixels>, ds: &DesignSystem) -> Option<GainDrag> {
        let hit = self.clip_hover_at(at, ds.track_header_height)?;
        let clip = self.wave_clips().find(|c| c.id == hit.id)?;
        if hit.edge.is_some() || clip.props.locked {
            return None;
        }
        let row = self.layout.as_ref()?.clip_row(clip.track, clip.id)?;
        let bounds = self.lanes_bounds.get();
        let y = f32::from(at.y - bounds.top()) - row.top;
        if y <= clip_frame::title_height(ds) || y >= row.height - 3. {
            return None;
        }
        let beat = self.viewport.beat_at(f64::from(at.x - bounds.left()));
        Some(GainDrag {
            id: hit.id,
            y: at.y,
            db: clip.props.gain_db,
            beat,
        })
    }

    pub(in crate::ui) fn begin_wave_gain(&mut self, at: Point<Pixels>, ds: &DesignSystem) -> bool {
        self.overlap.drag = self.gain_at(at, ds);
        self.overlap.drag.is_some()
    }

    pub(in crate::ui) fn toggle_overlap_clip(
        &mut self,
        at: Point<Pixels>,
        ds: &DesignSystem,
    ) -> bool {
        let Some(hit) = self.clip_hover_at(at, ds.track_header_height) else {
            return false;
        };
        if !self.wave_clips().any(|c| c.id == hit.id) {
            return false;
        }
        let selected = &mut self.overlap.selected;
        if let Some(index) = selected.iter().position(|id| *id == hit.id) {
            selected.remove(index);
        } else {
            selected.push(hit.id);
        }
        true
    }

    /// En empates exactos, el orden de la selección define quién entra.
    pub(in crate::ui) fn crossfade_pair(&self) -> Option<(&AudioClip, &AudioClip)> {
        let &[a, b] = self.overlap.selected.as_slice() else {
            return None;
        };
        let a = self.wave_clips().find(|c| c.id == a)?;
        let b = self.wave_clips().find(|c| c.id == b)?;
        let bpm = self.tempo.bpm();
        if a.track != b.track
            || a.crossfade_root() == b.crossfade_root()
            || a.props.locked
            || b.props.locked
            || a.props.muted
            || b.props.muted
            || a.start_beat.max(b.start_beat) >= a.end_beat(bpm).min(b.end_beat(bpm))
        {
            return None;
        }
        if a.processing
            .edit
            .crossfade
            .is_some_and(|x| x.from == b.crossfade_root())
        {
            return Some((b, a));
        }
        if b.processing
            .edit
            .crossfade
            .is_some_and(|x| x.from == a.crossfade_root())
        {
            return Some((a, b));
        }
        if (a.start_beat, a.end_beat(bpm)) > (b.start_beat, b.end_beat(bpm)) {
            Some((b, a))
        } else {
            Some((a, b))
        }
    }

    fn overlap_region(&self, track: usize, beat: f64, rate: u32) -> Option<Region> {
        let selected = &self.overlap.selected;
        if selected.len() == 1 {
            return None;
        }
        let per_beat = 60. / self.tempo.bpm() * f64::from(rate);
        let at = (beat * per_beat).floor() as usize;
        let mut first = 0;
        let mut last = usize::MAX;
        let mut sources = Vec::new();
        for clip in self.wave_clips() {
            if clip.props.muted
                || if selected.is_empty() {
                    clip.track != track
                } else {
                    !selected.contains(&clip.id)
                }
            {
                continue;
            }
            let start = (clip.start_beat * per_beat).round() as usize;
            let end =
                start.saturating_add((clip.length_seconds() * f64::from(rate)).round() as usize);
            if start >= end {
                continue;
            }
            if selected.is_empty() {
                if end <= at {
                    first = first.max(end);
                    continue;
                }
                if start > at {
                    last = last.min(start);
                    continue;
                }
            }
            first = first.max(start);
            last = last.min(end);
            sources.push(Source {
                id: clip.id,
                track: clip.track,
                file: clip.file.clone(),
                gain: clip.props.gain_linear(),
                fades: Vec::new(),
                start,
                offset: (clip.trim_start * f64::from(rate)).round() as usize,
            });
        }
        if sources.len() < 2 || first >= last {
            return None;
        }
        let mut fades = crate::audio::crossfade::prepare(self.wave_clips(), rate, self.tempo.bpm());
        for source in &mut sources {
            source.fades = fades.remove(&source.id).unwrap_or_default();
        }
        Some(Region {
            sources,
            frames: first..last,
            rate,
        })
    }
}

impl Region {
    fn measure(
        &self,
        cache: &mut Vec<Converted>,
        cancel: &AtomicBool,
    ) -> Result<Option<(usize, f32)>, String> {
        cache.retain(|entry| {
            entry.rate == self.rate
                && self
                    .sources
                    .iter()
                    .any(|s| Arc::ptr_eq(&s.file, &entry.file))
        });
        for s in &self.sources {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if s.file.sample_rate != self.rate
                && !cache.iter().any(|e| Arc::ptr_eq(&s.file, &e.file))
            {
                cache.push(Converted {
                    file: s.file.clone(),
                    rate: self.rate,
                    frames: resample(&s.file.frames, s.file.sample_rate, self.rate)?,
                });
            }
        }
        let inputs: Vec<_> = self
            .sources
            .iter()
            .map(|s| {
                let frames = cache
                    .iter()
                    .find(|e| Arc::ptr_eq(&s.file, &e.file))
                    .map_or(&s.file.frames, |e| &e.frames);
                (s, frames)
            })
            .collect();
        let mut peak = (self.frames.start, 0.0_f32);
        for first in self.frames.clone().step_by(1024) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let len = (self.frames.end - first).min(1024);
            let mut block = [[0.0_f32; 2]; 1024];
            for &(s, frames) in &inputs {
                let offset = s.offset.saturating_add(first - s.start);
                for (i, (out, input)) in block[..len]
                    .iter_mut()
                    .zip(frames.get(offset..).unwrap_or_default())
                    .enumerate()
                {
                    let gain = s
                        .fades
                        .iter()
                        .fold(s.gain, |gain, fade| gain * fade.gain((first + i) as u64));
                    out[0] += input[0] * gain;
                    out[1] += input[1] * gain;
                }
            }
            for (i, pair) in block[..len].iter().enumerate() {
                let amplitude = pair[0].abs().max(pair[1].abs());
                if amplitude > peak.1 {
                    peak = (first + i, amplitude);
                }
            }
        }
        Ok(Some(peak))
    }
}

impl DawApp {
    pub(in crate::ui) fn set_crossfade(&mut self, from: ClipId, to: ClipId, center: Option<u8>) {
        let Some((a, b)) = self.timeline.crossfade_pair() else {
            return;
        };
        if a.id != from || b.id != to {
            return;
        }
        let (source, family, track, previous) = (
            a.crossfade_root(),
            b.crossfade_root(),
            b.track,
            b.processing.edit.crossfade,
        );
        let next = center.map(|center| elevate::musical::sample_edit::Crossfade {
            from: source,
            center: center.clamp(1, 99),
        });
        if next == previous {
            return;
        }
        for piece in self
            .timeline
            .clip
            .iter_mut()
            .chain(&mut self.timeline.other_audio_clips)
        {
            if piece.track == track
                && piece.crossfade_root() == family
                && !piece.props.locked
                && piece.processing.edit.crossfade == previous
            {
                piece.processing.edit.crossfade = next;
            }
        }
        self.publish_clip_edit();
    }

    pub(in crate::ui) fn move_wave_gain(&mut self, at: Point<Pixels>) -> bool {
        let Some(drag) = self.timeline.overlap.drag else {
            return false;
        };
        let db = ((drag.db + f32::from(drag.y - at.y) * 0.2) * 10.)
            .round()
            .clamp(-240., 120.)
            / 10.;
        if let Some(clip) = self
            .timeline
            .clip
            .iter_mut()
            .chain(&mut self.timeline.other_audio_clips)
            .find(|c| c.id == drag.id && !c.props.locked && c.props.gain_db != db)
        {
            clip.props.gain_db = db;
            self.status_message = format!("{} · Ganancia {db:+.1} dB", clip.name);
            self.publish_clip_edit();
        }
        true
    }

    pub(in crate::ui) fn sync_overlap(&mut self, window: &Window, cx: &mut Context<Self>) {
        let tl = &self.timeline;
        let anchor = tl
            .overlap
            .drag
            .and_then(|d| {
                tl.wave_clips()
                    .find(|c| c.id == d.id)
                    .map(|c| (c.track, d.beat))
            })
            .or_else(|| {
                let at = tl
                    .probe
                    .pointer
                    .filter(|p| window.is_window_hovered() && *p == window.mouse_position())?;
                let hit = tl.clip_hover_at(at, self.ds.track_header_height)?;
                let clip = tl.wave_clips().find(|c| c.id == hit.id)?;
                Some((
                    clip.track,
                    tl.viewport
                        .beat_at(f64::from(at.x - tl.lanes_bounds.get().left())),
                ))
            })
            .unwrap_or((self.cursor_row, tl.playhead.unwrap_or(tl.edit_beat)));
        let rate = self
            .player
            .as_ref()
            .map(|p| p.sample_rate)
            .or(self.settings.audio.saved.sample_rate)
            .unwrap_or(48000);
        let next = self
            .arrangement_visible()
            .then(|| tl.overlap_region(anchor.0, anchor.1, rate))
            .flatten();
        let meter = &mut self.timeline.overlap;
        if next != meter.region {
            if let Some(cancel) = &meter.cancel {
                cancel.store(true, Ordering::Relaxed);
            }
            meter.region = next;
            meter.peak = None;
            meter.error = None;
            if meter.region.is_none() {
                meter.cache.clear();
            }
        }
        if meter.worker.is_some() || meter.peak.is_some() || meter.error.is_some() {
            return;
        }
        let Some(region) = meter.region.clone() else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        meter.cancel = Some(cancel.clone());
        let mut cache = std::mem::take(&mut meter.cache);
        let worker = cx.background_spawn(async move {
            let result = region.measure(&mut cache, &cancel);
            (region, result, cache)
        });
        meter.worker = Some(cx.spawn(async move |this, cx| {
            let (region, result, cache) = worker.await;
            let _ = this.update(cx, |this, cx| {
                let meter = &mut this.timeline.overlap;
                meter.worker = None;
                meter.cancel = None;
                if meter.region.is_some() {
                    meter.cache = cache;
                }
                if meter.region.as_ref() == Some(&region) {
                    match result {
                        Ok(peak) => meter.peak = peak,
                        Err(error) => meter.error = Some(error),
                    }
                }
                cx.notify();
            });
        }));
    }
}
