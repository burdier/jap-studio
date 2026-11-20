//! Adaptador entre celdas visuales y notas. El patrón conserva ticks, no columnas.
use super::DawApp;
use elevate::musical::{
    drums::{DrumPatternId, DrumPlaybackSource, DrumSession, DrumSongCell},
    model::{EditError, NoteEvent, Pattern},
    time::{Meter, PPQ, Tick},
};

const TICKS_PER_STEP: i64 = PPQ / 4;

#[derive(Clone, Copy)]
pub(super) enum PatternAction {
    Toggle {
        lane: usize,
        step: usize,
    },
    Velocity {
        lane: usize,
        step: usize,
        delta: i16,
    },
    Select(DrumPatternId),
    ExtendBar,
    Duplicate,
    AppendToSong,
    UseSource(DrumPlaybackSource),
}

#[derive(Default)]
pub(super) struct PatternEditor {
    pub session: DrumSession,
}

impl PatternEditor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pattern(&self) -> &Pattern {
        &self.session.active_pattern().notes
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

    fn toggle(&mut self, lane: usize, step: usize) -> Result<&'static str, EditError> {
        let piece = self
            .session
            .kit
            .pieces
            .get(lane)
            .ok_or(EditError::InvalidNote)?;
        if step >= self.step_count() {
            return Err(EditError::InvalidNote);
        }
        let id = piece.id;
        let pitch = piece.base_note();
        let old_note = self.note_at(lane, step).copied();
        let pattern = &mut self.session.active_pattern_mut().notes;
        if let Some(note) = old_note {
            pattern.remove_note(note.id)?;
            Ok("Nota eliminada")
        } else {
            let start = Tick(step as i64 * TICKS_PER_STEP);
            let duration = Tick(TICKS_PER_STEP.min(pattern.length().0 - start.0));
            pattern.add_note(id, start, duration, pitch, 100)?;
            Ok("Nota añadida · velocity 100")
        }
    }

    fn apply(&mut self, action: PatternAction, meter: Meter) -> Result<&'static str, String> {
        match action {
            PatternAction::Toggle { lane, step } => {
                self.toggle(lane, step).map_err(|error| error.to_string())
            }
            PatternAction::Velocity { lane, step, delta } => {
                let note = self
                    .note_at(lane, step)
                    .copied()
                    .ok_or_else(|| EditError::UnknownNote.to_string())?;
                let velocity = (i32::from(note.velocity) + i32::from(delta)).clamp(1, 127) as u8;
                self.session
                    .active_pattern_mut()
                    .notes
                    .set_velocity(note.id, velocity)
                    .map_err(|error| error.to_string())?;
                Ok("Velocity actualizada")
            }
            PatternAction::Select(id) => {
                self.session.select(id)?;
                Ok("Patrón seleccionado")
            }
            PatternAction::ExtendBar => {
                self.session
                    .resize_active(Tick(self.pattern().length().0 + meter.ticks_per_bar()))?;
                Ok("Compás añadido")
            }
            PatternAction::Duplicate => {
                self.session.duplicate_active()?;
                Ok("Patrón duplicado")
            }
            PatternAction::AppendToSong => {
                self.session.set_song_cell(
                    DrumSongCell {
                        at: self.session.song_end()?,
                        pattern: self.session.active,
                    },
                    true,
                )?;
                Ok("Patrón añadido a Canción")
            }
            PatternAction::UseSource(source) => {
                self.session.source = source;
                Ok("Fuente de batería seleccionada")
            }
        }
    }
}

impl DawApp {
    pub(super) fn apply_pattern_action(&mut self, action: PatternAction) {
        let mut status = match self.pattern_editor.apply(action, self.timeline.meter) {
            Ok(message) => {
                self.cursor_col = self.cursor_col.min(self.pattern_editor.step_count() - 1);
                format!(
                    "{message} · {} notas",
                    self.pattern_editor.pattern().notes().len()
                )
            }
            Err(error) => error,
        };
        if self.pattern_loop_beats.is_some() {
            status.push_str(" · Space: reiniciar audición");
        }
        self.status_message = status;
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

    /// Paso que suena ahora. Es solo presentación: lo calcula el frame del motor.
    pub(super) fn pattern_play_step(&self) -> Option<usize> {
        if self.pattern_editor.session.source != DrumPlaybackSource::Pattern {
            return None;
        }
        let player = self.player.as_ref()?;
        let loop_beats = self.pattern_loop_beats?;
        let local = player.beat().rem_euclid(loop_beats);
        Some((local / self.pattern_editor.beats_per_step()).floor() as usize)
    }
}
