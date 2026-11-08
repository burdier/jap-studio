//! Datos editables: sin voces, muestras, dispositivos ni coordenadas de pantalla.
use super::time::Tick;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstrumentId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteEvent {
    pub id: NoteId,
    pub instrument: InstrumentId,
    pub start: Tick,
    pub duration: Tick,
    pub pitch: u8,
    pub velocity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    InvalidLength,
    InvalidNote,
    UnknownNote,
    IdsExhausted,
}
impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLength => "El patrón debe tener longitud positiva",
            Self::InvalidNote => "Nota inválida: revisar posición, duración, pitch y velocity",
            Self::UnknownNote => "La nota no existe",
            Self::IdsExhausted => "No quedan identificadores de nota",
        })
    }
}
impl std::error::Error for EditError {}

/// Un patrón admite varias notas en el mismo tick. Los IDs son locales y no se reutilizan.
/// La edición puede reservar memoria; realizarla fuera del callback.
#[derive(Debug, Clone)]
pub struct Pattern {
    length: Tick,
    notes: Vec<NoteEvent>,
    next_id: u64,
}

impl Pattern {
    pub fn new(length: Tick) -> Result<Self, EditError> {
        if length.0 <= 0 {
            return Err(EditError::InvalidLength);
        }
        Ok(Self {
            length,
            notes: Vec::new(),
            next_id: 1,
        })
    }
    pub fn length(&self) -> Tick {
        self.length
    }
    pub fn notes(&self) -> &[NoteEvent] {
        &self.notes
    }

    pub fn add_note(
        &mut self,
        instrument: InstrumentId,
        start: Tick,
        duration: Tick,
        pitch: u8,
        velocity: u8,
    ) -> Result<NoteId, EditError> {
        let end = start
            .0
            .checked_add(duration.0)
            .ok_or(EditError::InvalidNote)?;
        if start.0 < 0
            || duration.0 <= 0
            || end > self.length.0
            || pitch > 127
            || !(1..=127).contains(&velocity)
        {
            return Err(EditError::InvalidNote);
        }
        let next = self.next_id.checked_add(1).ok_or(EditError::IdsExhausted)?;
        let id = NoteId(self.next_id);
        let index = self.notes.partition_point(|note| note.start <= start);
        self.notes.insert(
            index,
            NoteEvent {
                id,
                instrument,
                start,
                duration,
                pitch,
                velocity,
            },
        );
        self.next_id = next;
        Ok(id)
    }
    pub fn remove_note(&mut self, id: NoteId) -> Result<(), EditError> {
        let index = self
            .notes
            .iter()
            .position(|note| note.id == id)
            .ok_or(EditError::UnknownNote)?;
        self.notes.remove(index);
        Ok(())
    }
    pub fn set_velocity(&mut self, id: NoteId, velocity: u8) -> Result<(), EditError> {
        if !(1..=127).contains(&velocity) {
            return Err(EditError::InvalidNote);
        }
        let note = self
            .notes
            .iter_mut()
            .find(|note| note.id == id)
            .ok_or(EditError::UnknownNote)?;
        note.velocity = velocity;
        Ok(())
    }
}
