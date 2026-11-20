//! Adaptador entre celdas visuales y notas. El patrón conserva ticks, no columnas.
mod action;
mod inspector;
mod roll;

use super::{CenterView, DawApp, workspace_focus::Panel};
use elevate::musical::{
    drums::{DrumPlaybackSource, DrumSession},
    model::{NoteEvent, Pattern},
    time::{Meter, PPQ},
};
use gpui::Task;

pub(super) use action::PatternAction;

pub(in crate::ui) const TICKS_PER_STEP: i64 = PPQ / 4;
pub(in crate::ui) const SONG_BARS_PER_PAGE: i64 = 8;

#[derive(Default)]
pub(super) struct PatternEditor {
    pub session: DrumSession,
    pub cursor_lane: usize,
    pub cursor_step: usize,
    pub song_page: i64,
    pub workspace_ratio: Option<f32>,
    pub inspector_open: bool,
    pub mixer_open: bool,
    pub roll: roll::DrumRoll,
    sample_picker: Option<Task<()>>,
}

impl PatternEditor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pattern(&self) -> &Pattern {
        &self.session.active_pattern().notes
    }

    pub fn show_roll(&mut self, lane: usize) {
        if let Some(piece) = self.session.kit.pieces.get(lane) {
            self.cursor_lane = lane;
            self.roll.focus(piece, &self.session.active_pattern().notes);
        }
    }

    pub fn song_page_start(&self, meter: Meter) -> i64 {
        self.song_page
            .saturating_mul(SONG_BARS_PER_PAGE * meter.ticks_per_bar())
    }

    pub fn step_count(&self) -> usize {
        ((self.pattern().length().0 - 1) / TICKS_PER_STEP + 1) as usize
    }

    pub fn beats_per_step(&self) -> f64 {
        TICKS_PER_STEP as f64 / PPQ as f64
    }

    pub fn note_at(&self, lane: usize, step: usize) -> Option<&NoteEvent> {
        let piece = self.session.kit.pieces.get(lane)?;
        if step >= self.step_count() {
            return None;
        }
        let start = step as i64 * TICKS_PER_STEP;
        // Una celda representa un intervalo; no borra microtiming al cambiar resolución.
        self.pattern().notes().iter().find(|note| {
            note.instrument == piece.id
                && note.start.0 >= start
                && note.start.0 < start + TICKS_PER_STEP
        })
    }
}

impl DawApp {
    pub(super) fn apply_pattern_action(&mut self, action: PatternAction) {
        let selected_pattern = self.pattern_editor.session.active;
        match self.pattern_editor.apply(action, self.timeline.meter) {
            Ok(message) => {
                let editor = &mut self.pattern_editor;
                editor.cursor_step = editor.cursor_step.min(editor.step_count() - 1);
                if editor.session.active != selected_pattern {
                    editor.roll.reset_selection();
                }
                self.status_message =
                    format!("{message} · {} notas", editor.pattern().notes().len());
                self.publish_drum_edit();
            }
            Err(error) => self.status_message = error,
        }
    }

    pub(in crate::ui) fn drums_panel_active(&self) -> bool {
        self.center_view == CenterView::Session && self.workspace_focus.active == Panel::Center
    }

    pub(super) fn handle_drum_key(&mut self, key: &str) -> bool {
        if self.pattern_editor.mixer_open {
            return if key == "escape" {
                self.pattern_editor.mixer_open = false;
                self.mixer_drag = None;
                true
            } else {
                false
            };
        }
        if self.pattern_editor.roll.piece.is_some() {
            return self.handle_drum_roll_key(key);
        }
        let editor = &mut self.pattern_editor;
        match key {
            "enter" => {
                let lane = editor.cursor_lane;
                let step = editor.cursor_step;
                self.apply_pattern_action(PatternAction::Toggle { lane, step });
            }
            "h" | "left" => editor.cursor_step = editor.cursor_step.saturating_sub(1),
            "l" | "right" => {
                editor.cursor_step = (editor.cursor_step + 1).min(editor.step_count() - 1)
            }
            "k" | "up" => editor.cursor_lane = editor.cursor_lane.saturating_sub(1),
            "j" | "down" => {
                editor.cursor_lane =
                    (editor.cursor_lane + 1).min(editor.session.kit.pieces.len() - 1);
            }
            _ => return false,
        }
        true
    }

    pub(super) fn drum_playback_pattern(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, Pattern>>, String> {
        if self.pattern_loop_beats.is_some() {
            self.pattern_editor.session.playback_pattern()
        } else {
            Ok(None)
        }
    }

    /// Posición del patrón activo leída del motor. Canción usa su ocurrencia vigente.
    pub(in crate::ui) fn drum_pattern_playhead(&self) -> Option<f64> {
        let length = self
            .pattern_loop_beats
            .filter(|length| length.is_finite() && *length > 0.0)?;
        let beat = self.player.as_ref()?.beat().rem_euclid(length);
        let session = &self.pattern_editor.session;
        match session.source {
            DrumPlaybackSource::Pattern => Some(beat),
            DrumPlaybackSource::Song => {
                let tick = beat * PPQ as f64;
                let length = self.pattern_editor.pattern().length().0 as f64;
                session.song.iter().rev().find_map(|cell| {
                    let local = tick - cell.at.0 as f64;
                    (cell.pattern == session.active && local >= 0.0 && local < length)
                        .then_some(local / PPQ as f64)
                })
            }
            DrumPlaybackSource::Arrangement => None,
        }
    }

    pub(super) fn pattern_play_step(&self) -> Option<usize> {
        self.drum_pattern_playhead()
            .map(|beat| (beat / self.pattern_editor.beats_per_step()).floor() as usize)
    }
}
