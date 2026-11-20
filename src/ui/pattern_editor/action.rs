//! Acciones de la superficie de batería. Pasos, Canción y roll editan la sesión compartida.
use super::{PatternEditor, TICKS_PER_STEP};
use elevate::musical::{
    drums::{DrumPatternId, DrumPlaybackSource, DrumSongCell},
    model::{EditError, InstrumentId, NoteEvent, NoteId},
    time::{Meter, Tick},
};

#[derive(Clone)]
pub(in crate::ui) enum PatternAction {
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
    SongCell(DrumSongCell),
    InsertNote {
        piece: InstrumentId,
        at: Tick,
        duration: Tick,
        pitch: u8,
    },
    MoveNote(NoteEvent),
    DeleteNote(NoteId),
    NoteVelocity {
        id: NoteId,
        value: u8,
    },
    SampleParam {
        piece: InstrumentId,
        param: usize,
        value: f32,
    },
    Filter(InstrumentId),
    Choke {
        piece: InstrumentId,
        delta: i32,
    },
    AddSample {
        piece: InstrumentId,
        path: String,
    },
    RemoveSample {
        piece: InstrumentId,
        layer: usize,
    },
    LayerVelocity {
        piece: InstrumentId,
        layer: usize,
        upper: bool,
        delta: i16,
    },
    UseSource(DrumPlaybackSource),
}

impl PatternEditor {
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

    pub(super) fn apply(
        &mut self,
        action: PatternAction,
        meter: Meter,
    ) -> Result<&'static str, String> {
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
            PatternAction::SongCell(cell) => {
                let present = !self.session.song.contains(&cell);
                self.session.set_song_cell(cell, present)?;
                if present {
                    self.session.select(cell.pattern)?;
                }
                Ok(if present {
                    "Patrón añadido a Canción"
                } else {
                    "Patrón retirado de Canción"
                })
            }
            PatternAction::InsertNote {
                piece,
                at,
                duration,
                pitch,
            } => {
                let notes = &mut self.session.active_pattern_mut().notes;
                let id = notes
                    .add_note(piece, at, duration, pitch, 100)
                    .map_err(|error| error.to_string())?;
                self.roll.selected = Some(id);
                Ok("Nota insertada")
            }
            PatternAction::MoveNote(note) => {
                let pattern = &mut self.session.active_pattern_mut().notes;
                let mut notes = pattern.notes().to_vec();
                let current = notes
                    .iter_mut()
                    .find(|current| current.id == note.id)
                    .ok_or("La nota ya no existe")?;
                *current = note;
                *pattern = pattern
                    .with_notes(notes, pattern.next_id())
                    .map_err(|error| error.to_string())?;
                Ok("Nota editada")
            }
            PatternAction::DeleteNote(id) => {
                self.session
                    .active_pattern_mut()
                    .notes
                    .remove_note(id)
                    .map_err(|error| error.to_string())?;
                self.roll.selected = None;
                Ok("Nota eliminada")
            }
            PatternAction::NoteVelocity { id, value } => {
                self.session
                    .active_pattern_mut()
                    .notes
                    .set_velocity(id, value)
                    .map_err(|error| error.to_string())?;
                Ok("Velocity actualizada")
            }
            PatternAction::UseSource(source) => {
                self.session.source = source;
                Ok("Fuente de batería seleccionada")
            }
            action => self.apply_inspector(action),
        }
    }
}
