//! Adaptador entre celdas visuales y notas. El patrón conserva ticks, no columnas.
use super::DawApp;
use elevate::musical::{
    model::{EditError, InstrumentId, NoteEvent, Pattern},
    time::{PPQ, Tick},
};

pub(super) struct Lane {
    pub name: &'static str,
    pub instrument: InstrumentId,
    pub pitch: u8,
}
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
}
pub(super) struct PatternEditor {
    pub pattern: Pattern,
    pub lanes: Vec<Lane>,
    ticks_per_step: i64,
}

impl PatternEditor {
    pub fn new() -> Self {
        Self {
            pattern: Pattern::new(Tick(PPQ * 4)).expect("preset de un compás válido"),
            ticks_per_step: PPQ / 4,
            lanes: vec![
                Lane {
                    name: "Kick",
                    instrument: InstrumentId(1),
                    pitch: 36,
                },
                Lane {
                    name: "Snare",
                    instrument: InstrumentId(2),
                    pitch: 38,
                },
                Lane {
                    name: "Hi-Hat",
                    instrument: InstrumentId(3),
                    pitch: 42,
                },
                Lane {
                    name: "Clap",
                    instrument: InstrumentId(4),
                    pitch: 39,
                },
            ],
        }
    }
    pub fn step_count(&self) -> usize {
        ((self.pattern.length().0 - 1) / self.ticks_per_step + 1) as usize
    }
    pub fn length_beats(&self) -> f64 {
        self.pattern.length().0 as f64 / PPQ as f64
    }
    pub fn beats_per_step(&self) -> f64 {
        self.ticks_per_step as f64 / PPQ as f64
    }
    pub fn note_at(&self, lane: usize, step: usize) -> Option<&NoteEvent> {
        let lane = self.lanes.get(lane)?;
        if step >= self.step_count() {
            return None;
        }
        let start = step as i64 * self.ticks_per_step;
        // Una celda representa un intervalo; no borra microtiming al cambiar resolución.
        self.pattern.notes().iter().find(|note| {
            note.instrument == lane.instrument
                && note.start.0 >= start
                && note.start.0 < start + self.ticks_per_step
        })
    }
    fn apply(&mut self, action: PatternAction) -> Result<&'static str, EditError> {
        match action {
            PatternAction::Toggle { lane, step } => {
                let instrument = self.lanes.get(lane).ok_or(EditError::InvalidNote)?;
                if step >= self.step_count() {
                    return Err(EditError::InvalidNote);
                }
                if let Some(note) = self.note_at(lane, step).copied() {
                    self.pattern.remove_note(note.id)?;
                    Ok("Nota eliminada")
                } else {
                    let start = Tick(step as i64 * self.ticks_per_step);
                    let duration = Tick(self.ticks_per_step.min(self.pattern.length().0 - start.0));
                    self.pattern.add_note(
                        instrument.instrument,
                        start,
                        duration,
                        instrument.pitch,
                        100,
                    )?;
                    Ok("Nota añadida · velocity 100")
                }
            }
            PatternAction::Velocity { lane, step, delta } => {
                let note = self
                    .note_at(lane, step)
                    .copied()
                    .ok_or(EditError::UnknownNote)?;
                let velocity = (i32::from(note.velocity) + i32::from(delta)).clamp(1, 127) as u8;
                self.pattern.set_velocity(note.id, velocity)?;
                Ok("Velocity actualizada")
            }
        }
    }
}

impl DawApp {
    pub(super) fn apply_pattern_action(&mut self, action: PatternAction) {
        let mut status = match self.pattern_editor.apply(action) {
            Ok(message) => format!(
                "{message} · {} notas",
                self.pattern_editor.pattern.notes().len()
            ),
            Err(error) => error.to_string(),
        };
        if self.pattern_loop_beats.is_some() {
            status.push_str(" · Space: reiniciar audición");
        }
        self.status_message = status;
    }

    /// Paso que suena ahora. Es solo presentación: lo calcula el frame del motor.
    pub(super) fn pattern_play_step(&self) -> Option<usize> {
        let player = self.player.as_ref()?;
        let loop_beats = self.pattern_loop_beats?;
        let local = player.beat().rem_euclid(loop_beats);
        Some((local / self.pattern_editor.beats_per_step()).floor() as usize)
    }
}
