//! Selección por ClipId y cambios reversibles de clips MIDI.
use super::Timeline;
use elevate::musical::{
    clip::{ClipId, MidiClip},
    track::Track,
};
use gpui::{Bounds, Pixels, Point, point, px, size};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Default)]
pub(super) struct History {
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
}

struct Edit {
    before: EditDraft,
    after: EditDraft,
}

pub(in crate::ui) struct EditDraft {
    pub(super) clips: Vec<MidiClip>,
    selected: Vec<ClipId>,
    active: Option<ClipId>,
}

pub(super) struct SelectionBox {
    origin: Point<Pixels>,
    current: Point<Pixels>,
    original: Vec<ClipId>,
    original_audio: Vec<ClipId>,
    moved: bool,
    additive: bool,
}

impl Timeline {
    pub(super) fn select_midi_range(&mut self) {
        let Some(active) = &self.midi_clip else {
            return;
        };
        let anchor = self
            .selected_midi
            .last()
            .and_then(|id| self.midi_clips().find(|clip| clip.id == *id))
            .unwrap_or(active);
        let tracks = anchor.track.min(active.track)..=anchor.track.max(active.track);
        let first = anchor.start_beat.min(active.start_beat);
        let last = anchor.end_beat().max(active.end_beat());
        let mut ids: Vec<_> = self
            .midi_clips()
            .filter(|clip| {
                tracks.contains(&clip.track) && clip.start_beat < last && clip.end_beat() > first
            })
            .map(|clip| clip.id)
            .collect();
        ids.retain(|id| *id != anchor.id);
        ids.push(anchor.id);
        self.selected_midi = ids;
    }

    pub(in crate::ui) fn midi_clips(&self) -> impl Iterator<Item = &MidiClip> + Clone {
        self.other_midi_clips.iter().chain(self.midi_clip.iter())
    }

    pub(super) fn midi_clips_mut(&mut self) -> impl Iterator<Item = &mut MidiClip> {
        self.other_midi_clips
            .iter_mut()
            .chain(self.midi_clip.iter_mut())
    }

    pub(in crate::ui) fn midi_edit_targets(&self) -> Vec<ClipId> {
        let Some(active) = &self.midi_clip else {
            return Vec::new();
        };
        if self.selected_midi.contains(&active.id) {
            self.selected_midi.clone()
        } else {
            vec![active.id]
        }
    }

    pub(in crate::ui) fn midi_edit_draft(&self, ids: &[ClipId]) -> EditDraft {
        EditDraft {
            clips: self
                .midi_clips()
                .filter(|clip| ids.contains(&clip.id))
                .cloned()
                .collect(),
            selected: self.selected_midi.clone(),
            active: self.midi_clip.as_ref().map(|clip| clip.id),
        }
    }

    pub(in crate::ui) fn commit_midi_edit(&mut self, before: EditDraft, ids: &[ClipId]) {
        let after = self.midi_edit_draft(ids);
        if before.clips.len() == after.clips.len()
            && before.clips.iter().all(|clip| after.clips.contains(clip))
        {
            return;
        }
        const HISTORY_LIMIT: usize = 32;
        if self.midi_history.undo.len() == HISTORY_LIMIT {
            self.midi_history.undo.pop_front();
        }
        self.midi_history.undo.push_back(Edit { before, after });
        self.midi_history.redo.clear();
    }

    pub(in crate::ui) fn undo_midi_edit(&mut self, redo: bool) -> Result<(), &'static str> {
        let edit = if redo {
            self.midi_history.redo.pop()
        } else {
            self.midi_history.undo.pop_back()
        }
        .ok_or(if redo {
            "No hay edición MIDI para rehacer"
        } else {
            "No hay edición MIDI para deshacer"
        })?;

        let (current, target) = if redo {
            (&edit.before, &edit.after)
        } else {
            (&edit.after, &edit.before)
        };
        let states: HashMap<_, _> = self.midi_clips().map(|clip| (clip.id, clip)).collect();
        let valid = current
            .clips
            .iter()
            .all(|clip| states.get(&clip.id) == Some(&clip))
            && target.clips.iter().all(|clip| {
                !states.contains_key(&clip.id) || current.clips.iter().any(|old| old.id == clip.id)
            });
        if !valid {
            if redo {
                self.midi_history.redo.push(edit);
            } else {
                self.midi_history.undo.push_back(edit);
            }
            return Err("Los clips cambiaron después de esta edición");
        }
        let touched: Vec<_> = current.clips.iter().map(|clip| clip.id).collect();
        self.apply_midi_edit(&touched, target);
        if redo {
            self.midi_history.undo.push_back(edit);
        } else {
            self.midi_history.redo.push(edit);
        }
        Ok(())
    }

    fn apply_midi_edit(&mut self, touched: &[ClipId], target: &EditDraft) {
        let existing: HashSet<_> = self.midi_clips().map(|clip| clip.id).collect();
        let replacements: HashMap<_, _> = target.clips.iter().map(|clip| (clip.id, clip)).collect();
        let remove =
            |clip: &MidiClip| touched.contains(&clip.id) && !replacements.contains_key(&clip.id);
        self.other_midi_clips.retain(|clip| !remove(clip));
        if self.midi_clip.as_ref().is_some_and(remove) {
            self.midi_clip = None;
        }
        for clip in self.midi_clips_mut() {
            if let Some(replacement) = replacements.get(&clip.id) {
                *clip = (*replacement).clone();
            }
        }
        self.other_midi_clips.extend(
            target
                .clips
                .iter()
                .filter(|clip| !existing.contains(&clip.id))
                .cloned(),
        );
        if let Some(id) = target.active {
            self.focus_midi_clip(id);
        }
        if self.midi_clip.is_none() {
            self.midi_clip = self.other_midi_clips.pop();
        }
        self.selected_midi = target
            .selected
            .iter()
            .copied()
            .filter(|id| self.midi_clips().any(|clip| clip.id == *id))
            .collect();
    }

    pub(in crate::ui) fn change_selected_midi(&mut self, mutate: impl Fn(&mut MidiClip)) -> usize {
        let ids = self.midi_edit_targets();
        let before = self.midi_edit_draft(&ids);
        for clip in self.midi_clips_mut().filter(|clip| ids.contains(&clip.id)) {
            mutate(clip);
        }
        self.commit_midi_edit(before, &ids);
        ids.len()
    }

    pub(in crate::ui) fn delete_selected_midi(
        &mut self,
        ids: &[ClipId],
    ) -> Result<usize, &'static str> {
        let before = self.midi_edit_draft(ids);
        if before.clips.is_empty() {
            return Ok(0);
        }
        if before.clips.iter().any(|clip| clip.props.locked) {
            return Err("La selección contiene clips MIDI bloqueados");
        }
        let mut target = self.midi_edit_draft(&[]);
        target.selected.retain(|id| !ids.contains(id));
        self.apply_midi_edit(ids, &target);
        let count = before.clips.len();
        self.commit_midi_edit(before, &[]);
        Ok(count)
    }

    pub(in crate::ui) fn insert_midi_batch(&mut self, clips: Vec<MidiClip>) -> usize {
        let before = self.midi_edit_draft(&[]);
        let ids = clips.iter().map(|clip| clip.id).collect::<Vec<_>>();
        for clip in clips {
            self.insert_midi_clip(clip);
        }
        self.selected_midi = ids.clone();
        self.commit_midi_edit(before, &ids);
        ids.len()
    }

    pub(in crate::ui) fn duplicate_midi_selection(&mut self, ids: &[ClipId]) -> usize {
        let originals: Vec<_> = self
            .midi_clips()
            .filter(|clip| ids.contains(&clip.id))
            .collect();
        let first = originals
            .iter()
            .map(|clip| clip.start_beat)
            .reduce(f64::min);
        let last = originals
            .iter()
            .map(|clip| clip.end_beat())
            .reduce(f64::max);
        let (Some(first), Some(last)) = (first, last) else {
            return 0;
        };
        let copies = originals
            .into_iter()
            .map(|clip| {
                let mut copy = clip.duplicate();
                copy.start_beat = last + (clip.start_beat - first);
                copy
            })
            .collect();
        self.insert_midi_batch(copies)
    }

    pub(super) fn begin_midi_gesture(&mut self, ids: &[ClipId]) -> bool {
        let before = self.midi_edit_draft(ids);
        if before.clips.is_empty() || before.clips.iter().any(|clip| clip.props.locked) {
            return false;
        }
        self.midi_gesture = Some(before);
        true
    }

    pub(super) fn commit_midi_gesture(&mut self) -> bool {
        let Some(before) = self.midi_gesture.take() else {
            return false;
        };
        let ids = before.clips.iter().map(|clip| clip.id).collect::<Vec<_>>();
        self.commit_midi_edit(before, &ids);
        true
    }

    pub(in crate::ui) fn cancel_midi_gesture(&mut self) -> bool {
        if let Some(before) = self.midi_gesture.take() {
            let ids = before.clips.iter().map(|clip| clip.id).collect::<Vec<_>>();
            self.apply_midi_edit(&ids, &before);
            self.slip_drag = None;
            self.end_clip_drag();
            true
        } else if let Some(selection) = self.midi_box.take() {
            self.selected_midi = selection.original;
            self.selected_audio = selection.original_audio;
            true
        } else {
            false
        }
    }

    pub(super) fn move_midi_group(
        &mut self,
        raw: f64,
        track: usize,
        tracks: &[Track],
    ) -> Option<usize> {
        let active = self.midi_clip.as_ref()?.id;
        let originals = &self.midi_gesture.as_ref()?.clips;
        let anchor = originals.iter().find(|clip| clip.id == active)?;
        let anchor_start = anchor.start_beat;
        let track_delta = track as isize - anchor.track as isize;
        let earliest = originals
            .iter()
            .map(|clip| clip.start_beat)
            .reduce(f64::min)?;
        let minimum = anchor_start - earliest;
        let geometry: HashMap<_, _> = originals
            .iter()
            .map(|clip| {
                let index = clip.track.checked_add_signed(track_delta)?;
                if !tracks.get(index)?.accepts_clips() {
                    return None;
                }
                Some((clip.id, (index, clip.start_beat - anchor_start)))
            })
            .collect::<Option<_>>()?;
        let start = self.magnet_start(raw.max(minimum)).max(minimum);
        for clip in self.midi_clips_mut() {
            if let Some(&(index, offset)) = geometry.get(&clip.id) {
                clip.track = index;
                clip.start_beat = start + offset;
            }
        }
        self.edit_beat = start;
        Some(track)
    }

    pub(in crate::ui) fn begin_midi_box(&mut self, position: Point<Pixels>, additive: bool) {
        self.focused_audio = None;
        let original = self.selected_midi.clone();
        let original_audio = self.selected_audio.clone();
        if !additive {
            self.selected_midi.clear();
            self.selected_audio.clear();
        }
        self.midi_box = Some(SelectionBox {
            origin: position,
            current: position,
            original,
            original_audio,
            moved: false,
            additive,
        });
    }

    pub(super) fn update_midi_box(&mut self, position: Point<Pixels>, height: f32) {
        let Some(selection) = &mut self.midi_box else {
            return;
        };
        selection.current = position;
        selection.moved |= (selection.origin.x - position.x).abs() >= px(3.0)
            || (selection.origin.y - position.y).abs() >= px(3.0);
        let Some(rect) = self.midi_box_bounds() else {
            return;
        };
        let bounds = self.lanes_bounds.get();
        let first = self
            .viewport
            .beat_at(f64::from(rect.left() - bounds.left()));
        let last = self
            .viewport
            .beat_at(f64::from(rect.right() - bounds.left()));
        let top = f32::from(rect.top() - bounds.top());
        let bottom = f32::from(rect.bottom() - bounds.top());
        let selection = self.midi_box.as_ref().unwrap();
        let mut ids = if selection.additive {
            selection.original.clone()
        } else {
            Vec::new()
        };
        for clip in self.midi_clips() {
            let row = match &self.layout {
                Some(layout) => layout.clip_row(clip.track, clip.id),
                None => self.row_at_y(clip.track as f32 * height, height),
            };
            if row.is_some_and(|row| row.top <= bottom && row.top + row.height >= top)
                && clip.end_beat() >= first
                && clip.start_beat <= last
                && !ids.contains(&clip.id)
            {
                ids.push(clip.id);
            }
        }
        self.selected_midi = ids;
        let mut audio_ids = if selection.additive {
            selection.original_audio.clone()
        } else {
            Vec::new()
        };
        let bpm = self.tempo.bpm();
        for clip in self.clip.iter().chain(&self.other_audio_clips) {
            let row = match &self.layout {
                Some(layout) => layout.clip_row(clip.track, clip.id),
                None => self.row_at_y(clip.track as f32 * height, height),
            };
            if row.is_some_and(|row| row.top <= bottom && row.top + row.height >= top)
                && clip.end_beat(bpm) >= first
                && clip.start_beat <= last
                && !audio_ids.contains(&clip.id)
            {
                audio_ids.push(clip.id);
            }
        }
        self.selected_audio = audio_ids;
    }

    pub(super) fn midi_box_bounds(&self) -> Option<Bounds<Pixels>> {
        let selection = self.midi_box.as_ref().filter(|selection| selection.moved)?;
        let a = selection.origin;
        let b = selection.current;
        Some(Bounds::new(
            point(a.x.min(b.x), a.y.min(b.y)),
            size((a.x - b.x).abs(), (a.y - b.y).abs()),
        ))
    }

    pub(super) fn finish_midi_box(&mut self) -> bool {
        let Some(selection) = self.midi_box.take() else {
            return false;
        };
        if selection.moved
            && let Some(id) = self.selected_midi.first().copied()
            && !self
                .midi_clip
                .as_ref()
                .is_some_and(|clip| self.selected_midi.contains(&clip.id))
        {
            self.focus_midi_clip(id);
        }
        selection.moved
    }

    pub(super) fn reset_midi_history(&mut self) {
        self.midi_history = History::default();
    }

    pub(in crate::ui) fn shift_midi_selection(
        &mut self,
        delta: isize,
        tracks: &[Track],
    ) -> Result<(), &'static str> {
        let ids = self.midi_edit_targets();
        if self
            .midi_clips()
            .filter(|clip| ids.contains(&clip.id))
            .any(|clip| {
                clip.props.locked
                    || clip
                        .track
                        .checked_add_signed(delta)
                        .and_then(|index| tracks.get(index))
                        .is_none_or(|track| !track.accepts_clips())
            })
        {
            return Err("Los clips están bloqueados o la selección no cabe en esas pistas");
        }
        self.change_selected_midi(|clip| clip.track = clip.track.saturating_add_signed(delta));
        Ok(())
    }
}
