//! Acciones de la superficie de batería. Pasos, Canción y roll editan la sesión compartida.
use super::PatternEditor;
use elevate::musical::{
    drums::{DrumPatternId, DrumPlaybackSource, DrumSongCell},
    grid::NoteGrid,
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
    NewPattern,
    Meter(Meter),
    Grid(NoteGrid),
    Bars(i64),
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
    FullSample(InstrumentId),
    RetriggerChoke(InstrumentId),
    Choke {
        piece: InstrumentId,
        delta: i32,
    },
    AddPiece(String),
    AddSample {
        piece: InstrumentId,
        path: String,
    },
    ReplaceSample {
        piece: InstrumentId,
        layer: usize,
        previous: String,
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
        let step_ticks = self.ticks_per_step();
        let pattern = &mut self.session.active_pattern_mut().notes;
        if let Some(note) = old_note {
            pattern.remove_note(note.id)?;
            Ok("Nota eliminada")
        } else {
            let start = Tick(step as i64 * step_ticks);
            let duration = Tick(step_ticks.min(pattern.length().0 - start.0));
            pattern.add_note(id, start, duration, pitch, 100)?;
            Ok("Nota añadida · velocity 100")
        }
    }

    pub(super) fn apply(&mut self, action: PatternAction) -> Result<&'static str, String> {
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
            PatternAction::NewPattern => {
                self.session.new_pattern()?;
                self.cursor_step = 0;
                Ok("Patrón vacío creado")
            }
            PatternAction::Meter(meter) => {
                self.session.set_active_meter(meter)?;
                Ok("Compás actualizado")
            }
            PatternAction::Grid(grid) => {
                let cursor = self.cursor_step as i64 * self.ticks_per_step();
                self.session.active_pattern_mut().grid = grid;
                self.cursor_step = (cursor / grid.ticks().0) as usize;
                self.roll.snap = grid;
                Ok("Subdivisión actualizada")
            }
            PatternAction::Bars(delta) => {
                let pattern = self.session.active_pattern();
                let bars = pattern.bar_count() + delta;
                if bars < 1 {
                    return Err("El patrón necesita al menos un compás".into());
                }
                self.session
                    .resize_active(Tick(bars * pattern.meter.ticks_per_bar()))?;
                Ok("Duración del patrón actualizada")
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
            PatternAction::AddPiece(path) => {
                self.session.kit.add_sample(path)?;
                self.cursor_lane = self.session.kit.pieces.len() - 1;
                self.roll.piece = None;
                self.roll.reset_selection();
                self.mixer_open = false;
                self.steps_scroll.scroll_to_item(self.cursor_lane);
                Ok("Muestra añadida al kit")
            }
            action => self.apply_inspector(action),
        }
    }
}
