//! Adaptador entre celdas visuales y notas. El patrón conserva ticks, no columnas.
mod action;
mod hydrogen;
mod hydrogen_xml;
mod inspector;
mod roll;

use super::{CenterView, DawApp, workspace_focus::Panel};
use elevate::musical::{
    drums::{DrumPlaybackSource, DrumSession},
    model::{NoteEvent, Pattern},
    time::{Meter, PPQ},
};
use gpui::{ScrollHandle, Task};

pub(super) use action::PatternAction;
pub(super) use inspector::DrumSampleTarget;

pub(in crate::ui) const SONG_BARS_PER_PAGE: i64 = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum PatternOptions {
    Grid,
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct DrumLaneMenu {
    pub lane: usize,
    pub step: Option<usize>,
    pub position: gpui::Point<gpui::Pixels>,
}

pub(super) struct CopiedLane {
    pub base_note: u8,
    pub notes: Vec<NoteEvent>,
}

#[derive(Default)]
pub(super) struct PatternEditor {
    pub session: DrumSession,
    pub cursor_lane: usize,
    pub steps_scroll: ScrollHandle,
    pub options: Option<PatternOptions>,
    pub lane_menu: Option<DrumLaneMenu>,
    pub copied_lane: Option<CopiedLane>,
    pub cursor_step: usize,
    pub song_page: i64,
    pub workspace_ratio: Option<f32>,
    pub inspector_open: bool,
    pub mixer_open: bool,
    pub roll: roll::DrumRoll,
    sample_task: Option<Task<()>>,
    /// Picos normalizados por muestra de percusión; `None` mientras decodifica o si falló.
    pub waveforms: std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<String, Option<std::sync::Arc<Vec<[f32; 2]>>>>,
        >,
    >,
    offset_drag: Option<inspector::DrumOffsetDrag>,
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
            let pattern = self.session.active_pattern();
            self.roll.snap = pattern.grid;
            self.roll.focus(piece, &pattern.notes);
        }
    }

    pub fn song_page_start(&self, meter: Meter) -> i64 {
        self.song_page
            .saturating_mul(SONG_BARS_PER_PAGE * meter.ticks_per_bar())
    }

    pub fn step_count(&self) -> usize {
        self.session.active_pattern().step_count()
    }

    pub fn ticks_per_step(&self) -> i64 {
        self.session.active_pattern().grid.ticks().0
    }

    #[cfg(feature = "p2p")]
    pub(in crate::ui) fn editing_project_drums(&self) -> bool {
        self.sample_task.is_some() || self.offset_drag.is_some() || self.roll.dragging()
    }

    #[cfg(feature = "p2p")]
    pub(in crate::ui) fn install_shared_drums(&mut self, session: DrumSession) {
        let previous = self.session.active;
        let cursor_tick = self.cursor_step as i64 * self.ticks_per_step();
        self.session = session;
        if self.session.active == previous {
            self.cursor_step = (cursor_tick / self.ticks_per_step()) as usize;
        }
        self.cursor_step = self.cursor_step.min(self.step_count() - 1);
        self.cursor_lane = self
            .cursor_lane
            .min(self.session.kit.pieces.len().saturating_sub(1));
        if self.session.active != previous
            || self
                .roll
                .selected
                .is_some_and(|id| !self.pattern().notes().iter().any(|note| note.id == id))
        {
            self.roll.reset_selection();
        }
        if self
            .roll
            .piece
            .is_some_and(|id| !self.session.kit.pieces.iter().any(|piece| piece.id == id))
        {
            self.roll.piece = None;
        }
        self.roll.snap = self.session.active_pattern().grid;
    }

    pub(in crate::ui) fn step_notes(
        &self,
        lane: usize,
        step: usize,
    ) -> impl Iterator<Item = &NoteEvent> {
        let piece = self.session.kit.pieces.get(lane).map(|piece| piece.id);
        let notes = self.pattern().notes();
        let count = self.step_count();
        let step_ticks = self.ticks_per_step();
        let start = step.min(count) as i64 * step_ticks;
        let first = notes.partition_point(|note| note.start.0 < start);
        let end = notes.partition_point(|note| note.start.0 < start + step_ticks);
        notes[first..end]
            .iter()
            .filter(move |note| Some(note.instrument) == piece)
    }
}

impl DawApp {
    pub(super) fn apply_pattern_action(&mut self, action: PatternAction) {
        self.pattern_editor.lane_menu = None;
        let selected_pattern = self.pattern_editor.session.active;
        let updates_notes = matches!(
            &action,
            PatternAction::Toggle { .. }
                | PatternAction::SplitStep { .. }
                | PatternAction::Velocity { .. }
                | PatternAction::Bars(_)
                | PatternAction::InsertNote { .. }
                | PatternAction::MoveNote(_)
                | PatternAction::DeleteNote(_)
                | PatternAction::NoteVelocity { .. }
                | PatternAction::FillLane { .. }
                | PatternAction::ClearLane(_)
                | PatternAction::InvertLane(_)
                | PatternAction::ShiftLane { .. }
                | PatternAction::PasteLaneMidi(_)
                | PatternAction::CloneLane {
                    copy_midi: true,
                    ..
                }
        );
        let updates_score = updates_notes
            || matches!(
                &action,
                PatternAction::Select(_)
                    | PatternAction::Duplicate
                    | PatternAction::NewPattern
                    | PatternAction::SongCell(_)
                    | PatternAction::UseSource(_)
                    | PatternAction::CloneLane { .. }
            );
        let updates_ruler = matches!(&action, PatternAction::Grid(_));
        if updates_ruler {
            self.pattern_editor.options = None;
        }
        #[cfg(feature = "p2p")]
        let touched_step = self.pattern_editor.action_step(&action);
        let meter = self.timeline.meter;
        match self.pattern_editor.apply(action, meter) {
            Ok(message) => {
                #[cfg(feature = "p2p")]
                if let Some(step) = touched_step {
                    self.mark_drum_step(step);
                }
                let editor = &mut self.pattern_editor;
                editor.cursor_step = editor.cursor_step.min(editor.step_count() - 1);
                if editor.session.active != selected_pattern {
                    editor.roll.reset_selection();
                    editor.roll.snap = editor.session.active_pattern().grid;
                }
                self.status_message =
                    format!("{message} · {} notas", editor.pattern().notes().len());
                if updates_notes
                    && let Err(error) = self
                        .timeline
                        .refresh_drum_links(&editor.session, selected_pattern)
                {
                    self.status_message = error;
                    return;
                }
                if updates_score {
                    self.publish_drum_edit();
                } else if !updates_ruler {
                    self.publish_drum_mix();
                }
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
        if key == "escape"
            && (self.pattern_editor.lane_menu.take().is_some()
                || self.pattern_editor.options.take().is_some())
        {
            return true;
        }
        if self.pattern_editor.roll.piece.is_some() {
            return self.handle_drum_roll_key(key);
        }
        let editor = &mut self.pattern_editor;
        if editor.session.kit.pieces.is_empty() {
            return matches!(
                key,
                "enter" | "h" | "left" | "l" | "right" | "k" | "up" | "j" | "down"
            );
        }
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
        self.drum_pattern_playhead().map(|beat| {
            (beat * PPQ as f64 / self.pattern_editor.ticks_per_step() as f64).floor() as usize
        })
    }
}
