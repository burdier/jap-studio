//! Datos editables: sin voces, muestras, dispositivos ni coordenadas de pantalla.
use super::time::Tick;
use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstrumentId(pub u32);
static NEXT_INSTRUMENT_ID: AtomicU32 = AtomicU32::new(1000);

impl InstrumentId {
    pub fn reserve_restored(id: Self) -> Result<(), String> {
        let next = id.0.checked_add(1).ok_or("ID de instrumento agotado")?;
        NEXT_INSTRUMENT_ID.fetch_max(next, Ordering::Relaxed);
        Ok(())
    }

    pub fn fresh() -> Self {
        Self(
            NEXT_INSTRUMENT_ID
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .expect("IDs de instrumento agotados"),
        )
    }
}
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
    pub expression: super::note_expression::NoteExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    InvalidLength,
    NotesBeyondEnd,
    InvalidNote,
    UnknownNote,
    IdsExhausted,
}
impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotesBeyondEnd => "La longitud dejaría notas o controles fuera del clip; mueve o elimina esos eventos primero",
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub(super) automation: super::automation::Automation,
    length: Tick,
    pub(super) notes: Vec<NoteEvent>,
    next_id: u64,
}

impl Pattern {
    /// Restaura notas con sus IDs originales; conserva el siguiente ID libre.
    pub fn restore(length: Tick, notes: Vec<NoteEvent>) -> Result<Self, EditError> {
        let next_id = notes
            .iter()
            .map(|note| note.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(EditError::IdsExhausted)?;
        Self::restore_with_next_id(length, notes, next_id)
    }
    pub fn restore_with_next_id(
        length: Tick,
        mut notes: Vec<NoteEvent>,
        next_id: u64,
    ) -> Result<Self, EditError> {
        if length.0 <= 0 || next_id == 0 {
            return Err(EditError::InvalidLength);
        }
        let mut ids = HashSet::new();
        for note in &notes {
            let end = note
                .start
                .0
                .checked_add(note.duration.0)
                .ok_or(EditError::InvalidNote)?;
            if note.id.0 == 0
                || note.id.0 >= next_id
                || !ids.insert(note.id)
                || note.start.0 < 0
                || note.duration.0 <= 0
                || end > length.0
                || !note.expression.valid()
                || note.pitch > 127
                || !(1..=127).contains(&note.velocity)
            {
                return Err(EditError::InvalidNote);
            }
        }
        notes.sort_by_key(|note| note.start);
        Ok(Self {
            automation: Default::default(),
            length,
            notes,
            next_id,
        })
    }
    pub fn next_id(&self) -> u64 {
        self.next_id
    }
    pub fn new(length: Tick) -> Result<Self, EditError> {
        if length.0 <= 0 {
            return Err(EditError::InvalidLength);
        }
        Ok(Self {
            automation: Default::default(),
            length,
            notes: Vec::new(),
            next_id: 1,
        })
    }
    /// Cambia el final sin eliminar ni recortar notas o reutilizar sus IDs.
    pub fn resize(&mut self, length: Tick) -> Result<(), EditError> {
        if length.0 <= 0 {
            return Err(EditError::InvalidLength);
        }
        if self
            .notes
            .iter()
            .any(|note| note.start.0 + note.duration.0 > length.0)
            || self
                .controls()
                .points()
                .iter()
                .any(|point| point.at >= length)
        {
            return Err(EditError::NotesBeyondEnd);
        }
        self.length = length;
        Ok(())
    }

    pub fn length(&self) -> Tick {
        self.length
    }
    pub fn notes(&self) -> &[NoteEvent] {
        &self.notes
    }

    /// Snapshot de reproducción con tonos sustituidos; conserva IDs, tiempos,
    /// expresión, automatización y el siguiente ID libre del patrón editable.
    pub fn with_note_pitches(&self, pitches: &[(NoteId, u8)]) -> Result<Self, EditError> {
        let mut copy = self.clone();
        for &(id, pitch) in pitches {
            if pitch > 127 {
                return Err(EditError::InvalidNote);
            }
            let note = copy
                .notes
                .iter_mut()
                .find(|note| note.id == id)
                .ok_or(EditError::UnknownNote)?;
            note.pitch = pitch;
        }
        Ok(copy)
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
                expression: Default::default(),
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
