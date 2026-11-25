//! El rango y el zoom pertenecen a la ventana. Solo el procesamiento confirmado entra en Loro.
use super::{DawApp, timeline::DragKind};
use crate::audio::{
    AudioClip,
    sample_edit::Processing,
    sample_preview::{PreparedSamplePreview, SamplePreview},
};
use elevate::musical::{
    clip::ClipId,
    sample_edit::{Bell, Process, RangeEdit, SampleEdit},
};
use gpui::{AppContext, Bounds, Context, Pixels, Task};
use std::{cell::Cell, rc::Rc, sync::Arc};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum SampleTool {
    #[default]
    Select,
    Scissors,
}

pub(super) struct ChordDraft {
    id: ClipId,
    file: Arc<crate::audio::AudioFile>,
    visible: (usize, usize),
    pub cuts: Vec<usize>,
    pub dragging: Option<usize>,
}
impl ChordDraft {
    fn append(&self, edit: &mut SampleEdit) {
        for (i, pair) in self.cuts.windows(2).enumerate() {
            edit.ranges.push(RangeEdit {
                first: pair[0],
                end: pair[1],
                op: if i == 0 {
                    Process::ChordStart
                } else {
                    Process::ChordNext
                },
            });
        }
    }
}
#[derive(Default)]
pub(super) struct SampleEditor {
    pub id: Option<ClipId>,
    pub pending: Option<DragKind>,
    pub tab: u8,
    pub tool: SampleTool,
    pub hover_frame: Option<usize>,
    pub selection: Option<(usize, usize)>,
    pub zoom: Option<(usize, usize)>,
    pub anchor: Option<usize>,
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
    pub draft: Option<Bell>,
    pub chords: Option<ChordDraft>,
    pub tuning: Option<(i8, i8)>,
    pub preview_range: Option<(usize, usize)>,
    pub job: Option<Task<()>>,
    pub audition: Option<SamplePreview>,
    pub listening: Option<Task<()>>,
    pub looping: bool,
}
impl SampleEditor {
    pub fn chord_draft(&self, c: &AudioClip) -> Option<&ChordDraft> {
        self.chords.as_ref().filter(|d| {
            d.id == c.id
                && Arc::ptr_eq(&d.file, &c.file)
                && d.visible == self.visible(c)
                && !c.props.locked
        })
    }
    pub fn visible(&self, c: &AudioClip) -> (usize, usize) {
        let rate = f64::from(c.file.sample_rate);
        let len = c.file.frames.len();
        let first = ((c.trim_start * rate).round() as usize).min(len);
        let end = (((c.trim_start + c.length_seconds()) * rate).round() as usize).clamp(first, len);
        (first, end)
    }
    pub fn span(&self, c: &AudioClip, range: Option<(usize, usize)>) -> (usize, usize) {
        let (lo, hi) = self.visible(c);
        range
            .map(|(a, b)| (a.clamp(lo, hi), b.clamp(lo, hi)))
            .filter(|(a, b)| a < b)
            .unwrap_or((lo, hi))
    }
    pub fn silence(&mut self) {
        self.preview_range = None;
        self.audition = None;
        self.listening = None;
    }
}
impl DawApp {
    pub(in crate::ui) fn shown_sample(&self) -> Option<&AudioClip> {
        self.timeline.clip.as_ref().filter(|c| {
            Some(c.id) == self.timeline.sample_editor.id
                && Some(c.id) == self.timeline.focused_audio
        })
    }
    pub(in crate::ui) fn show_selected_editor(&mut self, kind: DragKind) {
        self.fx.editor_track = None;
        self.fx.tejer = None;
        self.fx.cut_draft = None;
        match kind {
            DragKind::Midi => {
                self.timeline.sample_editor.silence();
                self.timeline.sample_editor.draft = None;
                if !self.piano_roll.open
                    || self.piano_roll.opened_clip.as_ref().map(|c| c.id)
                        != self.timeline.midi_clip.as_ref().map(|c| c.id)
                {
                    self.open_piano_roll();
                }
            }
            DragKind::Audio => {
                let Some(id) = self.timeline.clip.as_ref().map(|c| c.id) else {
                    return;
                };
                let e = &mut self.timeline.sample_editor;
                if e.id != Some(id) {
                    e.chords = None;
                    e.tuning = None;
                    e.selection = None;
                    e.zoom = None;
                    e.anchor = None;
                    e.silence();
                }
                e.id = Some(id);
                e.draft = None;
                if self.piano_roll_window.is_none() {
                    self.piano_roll.open = false
                }
            }
        }
    }
    pub(in crate::ui) fn sample_position(&self, x: Pixels) -> Option<usize> {
        let c = self.shown_sample()?;
        let e = &self.timeline.sample_editor;
        let (a, b) = e.span(c, e.zoom);
        let bounds = e.bounds.get();
        let frac =
            (f32::from(x - bounds.left()) / f32::from(bounds.size.width).max(1.)).clamp(0., 1.);
        Some(a + ((b - a) as f64 * f64::from(frac)).round() as usize)
    }

    /// Divide el clip de audio en el Arrangement en el frame exacto pulsado con Tijeras.
    pub(in crate::ui) fn split_sample_at_frame(&mut self, frame: usize) {
        let Some(c) = self.shown_sample() else {
            return;
        };
        if c.props.locked {
            self.status_message = "Clip bloqueado".into();
            return;
        }
        let rate = f64::from(c.file.sample_rate).max(1.0);
        let bps = self.timeline.tempo.beats_per_second();
        let sec_in_file = frame as f64 / rate;
        let offset_sec = sec_in_file - c.trim_start;
        if offset_sec <= 0.005 || offset_sec >= c.length_seconds() - 0.005 {
            self.status_message = "Coloca las tijeras dentro del cuerpo de la onda".into();
            return;
        }
        let beat = c.start_beat + offset_sec * bps;
        self.timeline.sample_editor.selection = None;
        self.timeline.sample_editor.zoom = None;
        self.timeline.sample_editor.anchor = None;
        self.split_arrangement_clip(DragKind::Audio, beat);
    }

    /// Recorta (Crop/Trim) el clip de audio en el Arrangement a la selección actual `[a, b]`.
    pub(in crate::ui) fn crop_sample_to_selection(&mut self) {
        let Some((a, b)) = self.timeline.sample_editor.selection else {
            self.status_message = "Selecciona un rango sobre la onda para recortar".into();
            return;
        };
        let Some(c) = self.timeline.clip.as_mut().filter(|c| {
            Some(c.id) == self.timeline.sample_editor.id
                && Some(c.id) == self.timeline.focused_audio
        }) else {
            return;
        };
        if c.props.locked {
            self.status_message = "Clip bloqueado".into();
            return;
        }
        let rate = f64::from(c.file.sample_rate).max(1.0);
        let bps = self.timeline.tempo.beats_per_second();
        let new_start_sec = a as f64 / rate;
        let new_len_sec = (b.saturating_sub(a)) as f64 / rate;
        if new_len_sec < 0.01 {
            return;
        }
        let delta_sec = (new_start_sec - c.trim_start).max(0.0);
        c.start_beat += delta_sec * bps;
        c.trim_start = new_start_sec;
        c.trim_len = Some(new_len_sec);
        let e = &mut self.timeline.sample_editor;
        e.selection = None;
        e.zoom = None;
        e.anchor = None;
        e.silence();
        self.publish_clip_edit();
        self.status_message = format!("Clip recortado a {:.0} ms", new_len_sec * 1000.0);
    }

    /// Zoom in (`factor < 1.0`) o zoom out (`factor > 1.0`) centrado en `anchor_frame`.
    pub(in crate::ui) fn zoom_sample_by(&mut self, factor: f64, anchor_frame: Option<usize>) {
        let Some(c) = self.shown_sample() else {
            return;
        };
        let (lo, hi) = self.timeline.sample_editor.visible(c);
        let full = hi.saturating_sub(lo);
        if full <= 64 {
            return;
        }
        let (va, vb) = self
            .timeline
            .sample_editor
            .span(c, self.timeline.sample_editor.zoom);
        let cur_len = vb.saturating_sub(va).max(1);
        let target_len = ((cur_len as f64 * factor).round() as usize).clamp(64, full);
        if target_len >= full {
            self.timeline.sample_editor.zoom = None;
            return;
        }
        let anchor = anchor_frame.unwrap_or((va + vb) / 2).clamp(va, vb);
        let ratio = (anchor.saturating_sub(va)) as f64 / cur_len as f64;
        let left_span = (target_len as f64 * ratio).round() as usize;
        let new_a = anchor
            .saturating_sub(left_span)
            .clamp(lo, hi.saturating_sub(target_len));
        let new_b = (new_a + target_len).min(hi);
        self.timeline.sample_editor.zoom = Some((new_a, new_b));
    }
    pub(in crate::ui) fn analyze_chords(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.shown_sample() else { return };
        let e = &self.timeline.sample_editor;
        if c.props.locked || e.job.is_some() {
            return;
        }
        let id = c.id;
        let file = c.file.clone();
        let source = file.clone();
        let visible = e.visible(c);
        let (a, b) = e.span(c, e.selection);
        let job = cx.background_spawn(async move {
            crate::audio::chord_slices::boundaries(&source.frames[a..b], source.sample_rate)
        });
        let e = &mut self.timeline.sample_editor;
        e.chords = None;
        e.silence();
        self.status_message = "Buscando cambios de acorde…".into();
        e.job = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.timeline.sample_editor.job = None;
                if let Some(c) = this.shown_sample()
                    && c.id == id
                    && Arc::ptr_eq(&c.file, &file)
                    && !c.props.locked
                    && this.timeline.sample_editor.visible(c) == visible
                {
                    match result {
                        Ok(cuts)
                            if cuts.len() - 1 + c.processing.edit.ranges.len()
                                <= SampleEdit::LIMIT =>
                        {
                            let e = &mut this.timeline.sample_editor;
                            e.chords = Some(ChordDraft {
                                id,
                                file,
                                visible,
                                cuts: cuts.into_iter().map(|i| i + a).collect(),
                                dragging: None,
                            });
                            e.zoom = Some((a, b));
                            e.anchor = None;
                            this.status_message =
                                "Arrastra las marcas para ajustar los cortes".into();
                        }
                        Ok(_) => {
                            this.status_message =
                                "Demasiados cortes. Selecciona un rango más corto".into()
                        }
                        Err(error) => this.status_message = error,
                    }
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn apply_chords(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.shown_sample() else { return };
        let Some(draft) = self.timeline.sample_editor.chord_draft(c) else {
            return;
        };
        let mut edit = c.processing.edit.clone();
        let pitch = c.props.pitch_semitones;
        draft.append(&mut edit);
        self.timeline.sample_editor.silence();
        self.timeline.sample_editor.chords = None;
        self.prepare_sample_edit(edit, pitch, cx);
    }
    pub(in crate::ui) fn sample_operation(&mut self, op: Option<Process>, cx: &mut Context<Self>) {
        let Some(c) = self.shown_sample() else { return };
        let e = &self.timeline.sample_editor;
        let mut edit = c.processing.edit.clone();
        if let Some(op) = op {
            let (first, end) = e.span(c, e.selection);
            if first >= end {
                return;
            }
            edit.ranges.push(RangeEdit { first, end, op });
        } else {
            while edit
                .ranges
                .last()
                .is_some_and(|r| r.op == Process::ChordNext)
            {
                edit.ranges.pop();
            }
            if edit.ranges.pop().is_none() {
                return;
            }
        }
        self.prepare_sample_edit(edit, c.props.pitch_semitones, cx);
    }
    pub(in crate::ui) fn commit_bell(&mut self, bell: Bell, cx: &mut Context<Self>) {
        let Some(c) = self.shown_sample() else { return };
        let mut edit = c.processing.edit.clone();
        edit.bell = bell;
        self.prepare_sample_edit(edit, c.props.pitch_semitones, cx);
    }
    pub(in crate::ui) fn apply_sample_tuning(&mut self, cx: &mut Context<Self>) {
        let Some((semitones, fine)) = self.timeline.sample_editor.tuning.take() else {
            return;
        };
        let Some(c) = self.shown_sample() else { return };
        let mut edit = c.processing.edit.clone();
        edit.fine_cents = fine;
        self.prepare_sample_edit(edit, semitones, cx);
    }
    pub(in crate::ui) fn apply_sample_warp(
        &mut self,
        warp: elevate::musical::sample_edit::WarpSettings,
        cx: &mut Context<Self>,
    ) {
        let Some(c) = self.shown_sample() else { return };
        let mut edit = c.processing.edit.clone();
        edit.warp = warp;
        self.prepare_sample_edit(edit, c.props.pitch_semitones, cx);
    }
    pub(in crate::ui) fn rewarp_audio_clips_for_tempo(&mut self, bpm: f64) {
        use elevate::musical::sample_edit::WarpMode;
        let mut changed = false;
        for clip in self
            .timeline
            .clip
            .iter_mut()
            .chain(&mut self.timeline.other_audio_clips)
        {
            if clip.processing.edit.warp.mode == WarpMode::Off {
                continue;
            }
            let original = clip.original_file().clone();
            let edit = clip.processing.edit.clone();
            if let Ok((file, processing)) =
                Processing::prepare_for_bpm(original, edit, clip.props.pitch_semitones, bpm)
            {
                clip.file = file;
                clip.processing = processing;
                changed = true;
            }
        }
        if changed {
            self.publish_clip_edit();
        }
    }
    fn prepare_sample_edit(&mut self, edit: SampleEdit, semitones: i8, cx: &mut Context<Self>) {
        let Some(c) = self.shown_sample() else { return };
        if c.props.locked || self.timeline.sample_editor.job.is_some() {
            self.status_message = "Clip bloqueado o procesamiento en curso".into();
            return;
        }
        if c.processing.edit == edit && c.props.pitch_semitones == semitones {
            return;
        }
        let id = c.id;
        let original = c.original_file().clone();
        let previous_pitch = c.props.pitch_semitones;
        let expected = c.processing.edit.clone();
        let work_file = original.clone();
        let project_bpm = self.timeline.tempo.bpm();
        let job = cx.background_spawn(async move {
            Processing::prepare_for_bpm(work_file, edit, semitones, project_bpm)
        });
        self.timeline.sample_editor.job = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.timeline.sample_editor.job = None;
                let found = this
                    .timeline
                    .clip
                    .iter_mut()
                    .chain(&mut this.timeline.other_audio_clips)
                    .find(|c| c.id == id);
                match (found, result) {
                    (Some(c), Ok((file, processing)))
                        if !c.props.locked
                            && c.props.pitch_semitones == previous_pitch
                            && c.processing.edit == expected
                            && Arc::ptr_eq(c.original_file(), &original) =>
                    {
                        c.props.pitch_semitones = semitones;
                        c.processing = processing;
                        c.file = file;
                        let resume = this.shown_sample().is_some_and(|c| c.id == id)
                            && this
                                .timeline
                                .sample_editor
                                .audition
                                .as_ref()
                                .is_some_and(|a| a.finished() == Ok(false) && !a.paused());
                        this.timeline.sample_editor.silence();
                        this.publish_clip_edit();
                        if resume {
                            this.listen_sample(cx)
                        }
                    }
                    (_, Err(error)) => this.status_message = error,
                    _ => {
                        this.status_message =
                            "El clip cambió durante la preparación; vuelve a aplicar la edición"
                                .into()
                    }
                }
                cx.notify();
            });
        }));
    }
    pub(in crate::ui) fn listen_sample(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.shown_sample() else { return };
        let editor = &self.timeline.sample_editor;
        let draft = editor.chord_draft(c);
        let (a, b) = draft
            .map(|d| (d.cuts[0], *d.cuts.last().unwrap()))
            .unwrap_or_else(|| editor.span(c, editor.selection));
        let preview_edit = draft.map(|d| {
            let mut edit = c.processing.edit.clone();
            d.append(&mut edit);
            (c.original_file().clone(), edit, c.props.pitch_semitones)
        });
        let e = &mut self.timeline.sample_editor;
        if e.listening.take().is_some() {
            return;
        }
        if let Some(play) = &e.audition
            && play.finished() == Ok(false)
            && e.preview_range == Some((a, b))
        {
            play.set_paused(!play.paused());
            return;
        }
        e.silence();
        let Some(c) = self.shown_sample() else { return };
        let e = &self.timeline.sample_editor;
        let file = c.file.clone();
        let id = c.id;
        let looping = e.looping;
        let gain = c.props.gain_linear();
        let config = self.settings.audio.saved.clone();
        let project_bpm = self.timeline.tempo.bpm();
        let job = cx.background_spawn(async move {
            let file = match preview_edit {
                Some((original, edit, pitch)) => {
                    Processing::prepare_for_bpm(original, edit, pitch, project_bpm)?.0
                }
                None => file,
            };
            let mut frames = file.frames[a..b].to_vec();
            for v in frames.iter_mut().flatten() {
                *v *= gain
            }
            PreparedSamplePreview::from_frames(frames, file.sample_rate, &config, looping)
        });
        self.timeline.sample_editor.listening = Some(cx.spawn(async move |this, cx| {
            let stream = job.await;
            let _ = this.update(cx, |this, cx| {
                this.timeline.sample_editor.listening = None;
                if this.shown_sample().is_some_and(|c| c.id == id) {
                    match stream.and_then(|s| s.start(90)) {
                        Ok(play) => {
                            play.set_looping(this.timeline.sample_editor.looping);
                            this.timeline.sample_editor.preview_range = Some((a, b));
                            this.timeline.sample_editor.audition = Some(play)
                        }
                        Err(e) => this.status_message = e,
                    }
                }
                cx.notify();
            });
        }));
    }
}
