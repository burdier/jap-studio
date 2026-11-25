//! Marcadores, selección de región, Loop Brace y edición de tiempo estilo Ableton.
use super::Timeline;
use crate::audio::AudioClip;
use elevate::musical::{clip::MidiClip, time::PPQ};
use gpui::{Entity, Pixels, Point, component::input::InputState};

pub(in crate::ui) struct MarkerEdit {
    pub tick: i64,
    pub field: Entity<InputState>,
}

#[derive(Clone, Copy)]
pub(super) struct LoopDrag {
    origin: i64,
    initial: Option<(i64, i64)>,
    edge: Option<bool>,
    step: i64,
    range: Option<(i64, i64)>,
}

#[derive(Clone, Copy)]
pub(super) struct RegionDrag {
    origin: i64,
    step: i64,
    range: Option<(i64, i64)>,
}

pub(in crate::ui) fn loop_beats((start, end): (i64, i64)) -> (f64, f64) {
    (start as f64 / PPQ as f64, end as f64 / PPQ as f64)
}

pub(in crate::ui) fn beats_to_ticks(beat: f64) -> i64 {
    (beat.max(0.0) * PPQ as f64).round() as i64
}

impl Timeline {
    pub(in crate::ui) fn navigation_view(&self) -> (f64, f64, f64) {
        (
            self.viewport.first_beat,
            self.viewport.pixels_per_beat,
            f64::from(f32::from(self.ruler_bounds.get().size.width)),
        )
    }

    pub(in crate::ui) fn visible_loop(&self) -> Option<(i64, i64)> {
        self.loop_drag
            .and_then(|drag| drag.range)
            .or(self.navigation.loop_region)
    }

    pub(in crate::ui) fn visible_time_selection(&self) -> Option<(i64, i64)> {
        self.region_drag
            .and_then(|drag| drag.range)
            .or(self.time_selection)
    }

    /// Rango objetivo en ticks: primero la región de tiempo marcada, luego el rango
    /// de los clips seleccionados (audio + MIDI), y por último el Loop Brace.
    pub(in crate::ui) fn active_region_ticks(&self) -> Option<(i64, i64)> {
        if let Some((s, e)) = self.time_selection
            && e > s
        {
            return Some((s, e));
        }
        if let Some((start, end)) = self.selected_clips_beat_range() {
            let (s, e) = (beats_to_ticks(start), beats_to_ticks(end));
            if e > s {
                return Some((s, e));
            }
        }
        self.navigation.loop_region.filter(|(s, e)| e > s)
    }

    pub(in crate::ui) fn selected_clips_beat_range(&self) -> Option<(f64, f64)> {
        let bpm = self.tempo.bpm();
        let mut min_start = f64::INFINITY;
        let mut max_end = 0.0_f64;
        for clip in self
            .clip
            .iter()
            .chain(&self.other_audio_clips)
            .filter(|c| self.selected_audio.contains(&c.id))
        {
            min_start = min_start.min(clip.start_beat);
            max_end = max_end.max(clip.end_beat(bpm));
        }
        for clip in self
            .midi_clips()
            .filter(|c| self.selected_midi.contains(&c.id))
        {
            min_start = min_start.min(clip.start_beat);
            max_end = max_end.max(clip.end_beat());
        }
        if min_start.is_finite() && max_end > min_start {
            Some((min_start, max_end))
        } else {
            None
        }
    }

    pub(in crate::ui) fn begin_region_drag(&mut self, point: Point<Pixels>, fine: bool) {
        let x = self.ruler_x(point);
        let step = if fine {
            self.viewport.grid_step()
        } else {
            self.viewport.grid_step().max(1.0)
        };
        let step = ((step * PPQ as f64).round() as i64).max(1);
        let origin = self.loop_tick(x, step).max(0);
        self.edit_beat = origin as f64 / PPQ as f64;
        self.region_drag = Some(RegionDrag {
            origin,
            step,
            range: None,
        });
    }

    pub(in crate::ui) fn move_region_drag(&mut self, point: Point<Pixels>) -> bool {
        let Some(mut drag) = self.region_drag else {
            return false;
        };
        let tick = self.loop_tick(self.ruler_x(point), drag.step).max(0);
        drag.range = if tick == drag.origin {
            None
        } else {
            Some((drag.origin.min(tick), drag.origin.max(tick)))
        };
        self.region_drag = Some(drag);
        true
    }

    pub(in crate::ui) fn finish_region_drag(&mut self) -> Option<Option<(i64, i64)>> {
        let drag = self.region_drag.take()?;
        self.time_selection = drag.range;
        if let Some((start, _)) = drag.range {
            self.edit_beat = start as f64 / PPQ as f64;
        }
        Some(drag.range)
    }

    pub(in crate::ui) fn begin_loop_drag(&mut self, point: Point<Pixels>, fine: bool) {
        let x = self.ruler_x(point);
        let initial = self.navigation.loop_region;
        let edge = initial.and_then(|range| {
            let (start, end) = loop_beats(range);
            let left = (x - self.viewport.x_at(start)).abs();
            let right = (x - self.viewport.x_at(end)).abs();
            (left.min(right) <= 8.0).then_some(right < left)
        });
        let step = if fine {
            self.viewport.grid_step()
        } else {
            self.meter.beats_per_bar()
        };
        let step = ((step * PPQ as f64).round() as i64).max(1);
        let origin = self.loop_tick(x, step).max(0);
        let initial = initial.filter(|&range| {
            let (start, end) = loop_beats(range);
            edge.is_some() || (start..end).contains(&self.viewport.beat_at(x))
        });
        self.loop_drag = Some(LoopDrag {
            origin,
            initial,
            edge,
            step,
            range: None,
        });
    }

    pub(in crate::ui) fn move_loop_drag(&mut self, point: Point<Pixels>) -> bool {
        let Some(mut drag) = self.loop_drag else {
            return false;
        };
        let tick = self.loop_tick(self.ruler_x(point), drag.step).max(0);
        drag.range = if tick == drag.origin {
            None
        } else {
            let range = match (drag.initial, drag.edge) {
                (Some((start, _)), Some(true)) => (start, tick.max(start + 1)),
                (Some((_, end)), Some(false)) => (tick.min(end - 1), end),
                (Some((start, end)), None) => {
                    let delta = (tick - drag.origin).max(-start);
                    (start + delta, end.saturating_add(delta))
                }
                (None, _) => (drag.origin.min(tick), drag.origin.max(tick)),
            };
            Some(range)
        };
        self.loop_drag = Some(drag);
        true
    }

    pub(in crate::ui) fn finish_loop_drag(&mut self) -> Option<(i64, i64)> {
        self.loop_drag.take()?.range
    }

    /// Duplica el tiempo `[start, end]` estilo Ableton (Ctrl+Shift+D):
    /// desplaza todo lo posterior a `end` hacia `end + len` y clona el contenido
    /// de `[start, end]` en `[end, end + len]`.
    pub(in crate::ui) fn duplicate_time(&mut self, start_beat: f64, end_beat: f64) -> usize {
        let len = end_beat - start_beat;
        if !len.is_finite() || len <= 1e-6 {
            return 0;
        }
        let bpm = self.tempo.bpm();
        let bps = self.tempo.beats_per_second();
        let mut cloned = 0usize;

        // 1. Clips de audio
        let all_audio: Vec<AudioClip> = self
            .clip
            .take()
            .into_iter()
            .chain(self.other_audio_clips.drain(..))
            .collect();
        let mut next_audio = Vec::with_capacity(all_audio.len() * 2);
        for clip in all_audio {
            let c_start = clip.start_beat;
            let c_end = clip.end_beat(bpm);
            if c_start >= end_beat - 1e-6 {
                let mut shifted = clip;
                shifted.start_beat += len;
                next_audio.push(shifted);
            } else if c_end <= start_beat + 1e-6 {
                next_audio.push(clip);
            } else {
                // Parte anterior a end_beat (conserva lo que está antes de end_beat)
                if c_end > end_beat + 1e-6 {
                    let (left, mut right) = split_audio_clip(&clip, end_beat, bps);
                    right.start_beat += len;
                    // Clonar la parte dentro de [start_beat, end_beat]
                    if let Some(mut dup) = slice_audio_clip(&left, start_beat, end_beat, bpm, bps) {
                        dup.start_beat += len;
                        next_audio.push(dup);
                        cloned += 1;
                    }
                    next_audio.push(left);
                    next_audio.push(right);
                } else {
                    if let Some(mut dup) = slice_audio_clip(&clip, start_beat, end_beat, bpm, bps) {
                        dup.start_beat += len;
                        next_audio.push(dup);
                        cloned += 1;
                    }
                    next_audio.push(clip);
                }
            }
        }
        self.clip = next_audio.pop();
        self.other_audio_clips = next_audio;

        // 2. Clips MIDI
        let all_midi: Vec<MidiClip> = self
            .midi_clip
            .take()
            .into_iter()
            .chain(self.other_midi_clips.drain(..))
            .collect();
        let mut next_midi = Vec::with_capacity(all_midi.len() * 2);
        for clip in all_midi {
            let c_start = clip.start_beat;
            let c_end = clip.end_beat();
            if c_start >= end_beat - 1e-6 {
                let mut shifted = clip;
                shifted.start_beat += len;
                next_midi.push(shifted);
            } else if c_end <= start_beat + 1e-6 {
                next_midi.push(clip);
            } else if c_end > end_beat + 1e-6 {
                let (left, mut right) = split_midi_clip(&clip, end_beat);
                right.start_beat += len;
                if let Some(mut dup) = slice_midi_clip(&left, start_beat, end_beat) {
                    dup.start_beat += len;
                    next_midi.push(dup);
                    cloned += 1;
                }
                next_midi.push(left);
                next_midi.push(right);
            } else {
                if let Some(mut dup) = slice_midi_clip(&clip, start_beat, end_beat) {
                    dup.start_beat += len;
                    next_midi.push(dup);
                    cloned += 1;
                }
                next_midi.push(clip);
            }
        }
        self.midi_clip = next_midi.pop();
        self.other_midi_clips = next_midi;

        // 3. Marcadores
        let start_tick = beats_to_ticks(start_beat);
        let end_tick = beats_to_ticks(end_beat);
        let len_ticks = end_tick - start_tick;
        let old_markers = std::mem::take(&mut self.navigation.markers);
        for (tick, name) in old_markers {
            if tick >= end_tick {
                self.navigation.markers.insert(tick + len_ticks, name);
            } else if tick >= start_tick {
                self.navigation.markers.insert(tick, name.clone());
                self.navigation.markers.insert(tick + len_ticks, name);
            } else {
                self.navigation.markers.insert(tick, name);
            }
        }

        // 4. Avanzar la selección a la región recién duplicada (como Ableton)
        self.selected_audio.clear();
        self.selected_midi.clear();
        self.focused_audio = None;
        self.time_selection = Some((end_tick, end_tick + len_ticks));
        self.edit_beat = end_beat;
        cloned
    }

    /// Elimina el rango de tiempo `[start, end]` estilo Ableton (Ctrl+Shift+Delete):
    /// borra el contenido dentro del rango y contrae todo lo posterior hacia la izquierda.
    pub(in crate::ui) fn delete_time(&mut self, start_beat: f64, end_beat: f64) -> usize {
        let len = end_beat - start_beat;
        if !len.is_finite() || len <= 1e-6 {
            return 0;
        }
        let bpm = self.tempo.bpm();
        let bps = self.tempo.beats_per_second();
        let mut affected = 0usize;

        let all_audio: Vec<AudioClip> = self
            .clip
            .take()
            .into_iter()
            .chain(self.other_audio_clips.drain(..))
            .collect();
        let mut next_audio = Vec::with_capacity(all_audio.len());
        for clip in all_audio {
            let c_start = clip.start_beat;
            let c_end = clip.end_beat(bpm);
            if c_end <= start_beat + 1e-6 {
                next_audio.push(clip);
            } else if c_start >= end_beat - 1e-6 {
                let mut shifted = clip;
                shifted.start_beat = (shifted.start_beat - len).max(0.0);
                next_audio.push(shifted);
                affected += 1;
            } else {
                affected += 1;
                if c_start < start_beat - 1e-6 {
                    let (left, _) = split_audio_clip(&clip, start_beat, bps);
                    next_audio.push(left);
                }
                if c_end > end_beat + 1e-6 {
                    let (_, mut right) = split_audio_clip(&clip, end_beat, bps);
                    right.start_beat = start_beat;
                    next_audio.push(right);
                }
            }
        }
        self.clip = next_audio.pop();
        self.other_audio_clips = next_audio;

        let all_midi: Vec<MidiClip> = self
            .midi_clip
            .take()
            .into_iter()
            .chain(self.other_midi_clips.drain(..))
            .collect();
        let mut next_midi = Vec::with_capacity(all_midi.len());
        for clip in all_midi {
            let c_start = clip.start_beat;
            let c_end = clip.end_beat();
            if c_end <= start_beat + 1e-6 {
                next_midi.push(clip);
            } else if c_start >= end_beat - 1e-6 {
                let mut shifted = clip;
                shifted.start_beat = (shifted.start_beat - len).max(0.0);
                next_midi.push(shifted);
                affected += 1;
            } else {
                affected += 1;
                if c_start < start_beat - 1e-6 {
                    let (left, _) = split_midi_clip(&clip, start_beat);
                    next_midi.push(left);
                }
                if c_end > end_beat + 1e-6 {
                    let (_, mut right) = split_midi_clip(&clip, end_beat);
                    right.start_beat = start_beat;
                    next_midi.push(right);
                }
            }
        }
        self.midi_clip = next_midi.pop();
        self.other_midi_clips = next_midi;

        let start_tick = beats_to_ticks(start_beat);
        let end_tick = beats_to_ticks(end_beat);
        let len_ticks = end_tick - start_tick;
        let old_markers = std::mem::take(&mut self.navigation.markers);
        for (tick, name) in old_markers {
            if tick < start_tick {
                self.navigation.markers.insert(tick, name);
            } else if tick >= end_tick {
                self.navigation
                    .markers
                    .insert((tick - len_ticks).max(0), name);
            }
        }

        self.selected_audio.clear();
        self.selected_midi.clear();
        self.time_selection = None;
        self.edit_beat = start_beat;
        affected
    }

    /// Inserta tiempo vacío de duración `len_beat` en `start_beat` estilo Ableton (Ctrl+I).
    pub(in crate::ui) fn insert_silence(&mut self, start_beat: f64, len_beat: f64) {
        if !len_beat.is_finite() || len_beat <= 1e-6 {
            return;
        }
        let bpm = self.tempo.bpm();
        let bps = self.tempo.beats_per_second();

        let all_audio: Vec<AudioClip> = self
            .clip
            .take()
            .into_iter()
            .chain(self.other_audio_clips.drain(..))
            .collect();
        let mut next_audio = Vec::with_capacity(all_audio.len() + 2);
        for clip in all_audio {
            let c_start = clip.start_beat;
            let c_end = clip.end_beat(bpm);
            if c_start >= start_beat - 1e-6 {
                let mut shifted = clip;
                shifted.start_beat += len_beat;
                next_audio.push(shifted);
            } else if c_end > start_beat + 1e-6 {
                let (left, mut right) = split_audio_clip(&clip, start_beat, bps);
                right.start_beat += len_beat;
                next_audio.push(left);
                next_audio.push(right);
            } else {
                next_audio.push(clip);
            }
        }
        self.clip = next_audio.pop();
        self.other_audio_clips = next_audio;

        let all_midi: Vec<MidiClip> = self
            .midi_clip
            .take()
            .into_iter()
            .chain(self.other_midi_clips.drain(..))
            .collect();
        let mut next_midi = Vec::with_capacity(all_midi.len() + 2);
        for clip in all_midi {
            let c_start = clip.start_beat;
            let c_end = clip.end_beat();
            if c_start >= start_beat - 1e-6 {
                let mut shifted = clip;
                shifted.start_beat += len_beat;
                next_midi.push(shifted);
            } else if c_end > start_beat + 1e-6 {
                let (left, mut right) = split_midi_clip(&clip, start_beat);
                right.start_beat += len_beat;
                next_midi.push(left);
                next_midi.push(right);
            } else {
                next_midi.push(clip);
            }
        }
        self.midi_clip = next_midi.pop();
        self.other_midi_clips = next_midi;

        let start_tick = beats_to_ticks(start_beat);
        let len_ticks = beats_to_ticks(len_beat);
        let old_markers = std::mem::take(&mut self.navigation.markers);
        for (tick, name) in old_markers {
            if tick >= start_tick {
                self.navigation.markers.insert(tick + len_ticks, name);
            } else {
                self.navigation.markers.insert(tick, name);
            }
        }

        self.selected_audio.clear();
        self.selected_midi.clear();
        self.focused_audio = None;
        self.time_selection = Some((start_tick, start_tick + len_ticks));
        self.edit_beat = start_beat;
    }

    fn ruler_x(&self, point: Point<Pixels>) -> f64 {
        let bounds = self.ruler_bounds.get();
        f64::from(f32::from(point.x - bounds.left()))
            .clamp(0.0, f64::from(f32::from(bounds.size.width)))
    }

    pub(in crate::ui) fn ruler_beat(&self, point: Point<Pixels>) -> f64 {
        self.viewport.beat_at(self.ruler_x(point)).max(0.)
    }

    fn loop_tick(&self, x: f64, step: i64) -> i64 {
        let ticks = self.viewport.beat_at(x) * PPQ as f64;
        ((ticks / step as f64).round() as i64).saturating_mul(step)
    }

    pub(in crate::ui) fn change_wave_zoom(&mut self, factor: f32) {
        self.wave_zoom = (self.wave_zoom * factor).clamp(0.5, 8.0);
    }

    pub(in crate::ui) fn zoom_to_clip(&mut self) {
        let Some(clip) = &self.clip else { return };
        let width = f64::from(f32::from(self.ruler_bounds.get().size.width));
        let start = clip.start_beat;
        let span = clip.end_beat(self.tempo.bpm()) - start;
        if width <= 0.0 || span <= 0.0 {
            return;
        }
        self.viewport.pixels_per_beat =
            (width * 0.85 / span).clamp(8.0, super::viewport::MAX_SCALE);
        self.viewport.first_beat = (start - width * 0.05 / self.viewport.pixels_per_beat).max(0.0);
    }
}

fn split_audio_clip(clip: &AudioClip, beat: f64, bps: f64) -> (AudioClip, AudioClip) {
    let mut left = clip.clone();
    let mut right = clip.duplicate();
    let length_sec = (beat - left.start_beat) / bps;
    right.trim_start += length_sec;
    right.trim_len = Some((left.length_seconds() - length_sec).max(0.0));
    right.start_beat = beat;
    left.trim_len = Some(length_sec.max(0.0));
    (left, right)
}

fn slice_audio_clip(
    clip: &AudioClip,
    start_beat: f64,
    end_beat: f64,
    bpm: f64,
    bps: f64,
) -> Option<AudioClip> {
    let ov_start = clip.start_beat.max(start_beat);
    let ov_end = clip.end_beat(bpm).min(end_beat);
    if ov_end <= ov_start + 1e-6 {
        return None;
    }
    let mut dup = clip.duplicate();
    dup.trim_start += (ov_start - clip.start_beat) / bps;
    dup.trim_len = Some((ov_end - ov_start) / bps);
    dup.start_beat = ov_start;
    Some(dup)
}

fn split_midi_clip(clip: &MidiClip, beat: f64) -> (MidiClip, MidiClip) {
    let mut left = clip.clone();
    let mut right = clip.duplicate();
    let length = beat - left.start_beat;
    right.trim_start += length;
    right.trim_len = Some((left.end_beat() - beat).max(0.0));
    right.start_beat = beat;
    left.trim_len = Some(length.max(0.0));
    (left, right)
}

fn slice_midi_clip(clip: &MidiClip, start_beat: f64, end_beat: f64) -> Option<MidiClip> {
    let ov_start = clip.start_beat.max(start_beat);
    let ov_end = clip.end_beat().min(end_beat);
    if ov_end <= ov_start + 1e-6 {
        return None;
    }
    let mut dup = clip.duplicate();
    dup.trim_start += ov_start - clip.start_beat;
    dup.trim_len = Some(ov_end - ov_start);
    dup.start_beat = ov_start;
    Some(dup)
}
