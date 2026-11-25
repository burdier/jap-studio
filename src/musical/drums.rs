//! Kit y sesión de percusión editables. Las notas y los tiempos son los del secuenciador común.
mod arrangement;
mod kit;
pub use arrangement::{DrumClipTarget, arrangement_pattern, piece_pattern};
pub use kit::{
    DrumKit, DrumKitId, DrumLayer, DrumMix, DrumPiece, LAST_LEGACY_DRUM_ID, MAX_DRUM_PIECES,
    OffsetMode,
};

use super::{
    grid::NoteGrid,
    model::{NoteId, Pattern},
    time::{Meter, PPQ, Tick},
};
use std::{borrow::Cow, collections::HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrumPatternId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumPattern {
    pub id: DrumPatternId,
    pub name: String,
    pub notes: Pattern,
    pub meter: Meter,
    pub grid: NoteGrid,
}

impl DrumPattern {
    pub fn bar_count(&self) -> i64 {
        (self.notes.length().0 - 1) / self.meter.ticks_per_bar() + 1
    }

    pub fn step_count(&self) -> usize {
        ((self.notes.length().0 - 1) / self.grid.ticks().0 + 1) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrumSongCell {
    pub at: Tick,
    pub pattern: DrumPatternId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrumPlaybackSource {
    #[default]
    Pattern,
    Song,
    Arrangement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrumSession {
    pub kit: DrumKit,
    pub patterns: Vec<DrumPattern>,
    pub active: DrumPatternId,
    pub next_pattern_id: u32,
    pub song: Vec<DrumSongCell>,
    pub source: DrumPlaybackSource,
}

// La vista de pasos instancia sus celdas. Limita el tamaño antes de dibujar.
const MAX_PATTERN_TICKS: i64 = 128 * 4 * PPQ;

impl Default for DrumSession {
    fn default() -> Self {
        let id = DrumPatternId(1);
        Self {
            kit: DrumKit::default(),
            patterns: vec![DrumPattern {
                id,
                name: "Patrón 1".into(),
                notes: Pattern::new(Tick(Meter::default().ticks_per_bar())).unwrap(),
                meter: Meter::default(),
                grid: NoteGrid::default(),
            }],
            active: id,
            next_pattern_id: 2,
            song: Vec::new(),
            source: DrumPlaybackSource::default(),
        }
    }
}

impl DrumSession {
    pub fn pattern(&self, id: DrumPatternId) -> Option<&DrumPattern> {
        self.patterns.iter().find(|pattern| pattern.id == id)
    }

    pub fn active_pattern(&self) -> &DrumPattern {
        self.pattern(self.active).expect("patrón activo de batería")
    }

    pub fn active_pattern_mut(&mut self) -> &mut DrumPattern {
        self.patterns
            .iter_mut()
            .find(|pattern| pattern.id == self.active)
            .expect("patrón activo de batería")
    }

    pub fn select(&mut self, id: DrumPatternId) -> Result<(), String> {
        self.pattern(id).ok_or("Patrón de batería inexistente")?;
        self.active = id;
        Ok(())
    }

    pub fn resize_active(&mut self, length: Tick) -> Result<(), String> {
        if length.0 > MAX_PATTERN_TICKS {
            return Err("El patrón supera la capacidad de la vista de pasos".into());
        }
        if self
            .song
            .iter()
            .any(|cell| cell.pattern == self.active && cell.at.0.checked_add(length.0).is_none())
        {
            return Err("El patrón supera el final permitido de Canción".into());
        }
        self.active_pattern_mut()
            .notes
            .resize(length)
            .map_err(|error| error.to_string())
    }

    /// Reagrupa la regla del patrón conservando sus notas y su duración en ticks.
    pub fn set_active_meter(&mut self, meter: Meter) {
        self.active_pattern_mut().meter = meter;
    }

    pub fn new_pattern(&mut self) -> Result<DrumPatternId, String> {
        let meter = self.active_pattern().meter;
        let notes = Pattern::new(Tick(meter.ticks_per_bar())).map_err(|error| error.to_string())?;
        self.insert_pattern(notes)
    }

    pub fn duplicate_active(&mut self) -> Result<DrumPatternId, String> {
        self.insert_pattern(self.active_pattern().notes.clone())
    }

    fn insert_pattern(&mut self, notes: Pattern) -> Result<DrumPatternId, String> {
        let id = DrumPatternId(self.next_pattern_id);
        let next = self
            .next_pattern_id
            .checked_add(1)
            .ok_or("IDs de patrón agotados")?;
        let DrumPattern { meter, grid, .. } = *self.active_pattern();
        self.patterns.push(DrumPattern {
            id,
            name: format!("Patrón {}", id.0),
            notes,
            meter,
            grid,
        });
        self.next_pattern_id = next;
        self.active = id;
        Ok(id)
    }

    pub fn set_song_cell(&mut self, cell: DrumSongCell, present: bool) -> Result<(), String> {
        self.song_cell_end(cell)?;
        let sort_key = |entry: &DrumSongCell| (entry.at, entry.pattern);
        match (
            self.song.binary_search_by_key(&sort_key(&cell), sort_key),
            present,
        ) {
            (Ok(index), false) => {
                self.song.remove(index);
            }
            (Err(index), true) => self.song.insert(index, cell),
            _ => {}
        }
        Ok(())
    }

    pub fn song_end(&self) -> Result<Tick, String> {
        self.song
            .iter()
            .try_fold(Tick(0), |end, &cell| Ok(end.max(self.song_cell_end(cell)?)))
    }

    fn song_cell_end(&self, cell: DrumSongCell) -> Result<Tick, String> {
        let pattern = self
            .pattern(cell.pattern)
            .ok_or("Patrón de Canción inexistente")?;
        cell.at
            .0
            .checked_add(pattern.notes.length().0)
            .filter(|_| cell.at.0 >= 0)
            .map(Tick)
            .ok_or_else(|| "Posición de Canción inválida".into())
    }

    pub fn playback_length(&self) -> Result<Option<Tick>, String> {
        match self.source {
            DrumPlaybackSource::Pattern => Ok(Some(self.active_pattern().notes.length())),
            DrumPlaybackSource::Arrangement => Ok(None),
            DrumPlaybackSource::Song if self.song.is_empty() => {
                Err("Añade un patrón a Canción antes de reproducirla".into())
            }
            DrumPlaybackSource::Song => self.song_end().map(Some),
        }
    }

    /// Patrón temporal para la audición. Las ocurrencias mantienen su vínculo editable.
    pub fn playback_pattern(&self) -> Result<Option<Cow<'_, Pattern>>, String> {
        match self.source {
            DrumPlaybackSource::Pattern => Ok(Some(Cow::Borrowed(&self.active_pattern().notes))),
            DrumPlaybackSource::Arrangement => Ok(None),
            DrumPlaybackSource::Song => {
                if self.song.is_empty() {
                    return Err("Añade un patrón a Canción antes de reproducirla".into());
                }
                let length = self.song_end()?;
                let mut notes = Vec::new();
                for cell in &self.song {
                    let pattern = self
                        .pattern(cell.pattern)
                        .ok_or("Patrón de Canción inexistente")?;
                    for note in pattern.notes.notes() {
                        let mut copy = note.clone();
                        copy.id = NoteId(notes.len() as u64 + 1);
                        copy.start.0 += cell.at.0;
                        notes.push(copy);
                    }
                }
                Pattern::restore(length, notes)
                    .map(|pattern| Some(Cow::Owned(pattern)))
                    .map_err(|error| error.to_string())
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let pieces = self.kit.validate()?;
        let mut ids = HashSet::new();
        for pattern in &self.patterns {
            if pattern.id.0 == 0
                || pattern.id.0 >= self.next_pattern_id
                || !ids.insert(pattern.id)
                || pattern.name.trim().is_empty()
                || pattern.notes.length().0 > MAX_PATTERN_TICKS
                || pattern.notes.next_id() > i64::MAX as u64
                || pattern
                    .notes
                    .notes()
                    .iter()
                    .any(|note| !pieces.contains(&note.instrument))
            {
                return Err("Patrón de batería inválido".into());
            }
        }
        self.pattern(self.active)
            .ok_or("Patrón de batería activo inexistente")?;
        self.song_end()?;
        for pair in self.song.windows(2) {
            if (pair[0].at, pair[0].pattern) >= (pair[1].at, pair[1].pattern) {
                return Err("Ocurrencias de batería duplicadas o desordenadas".into());
            }
        }
        Ok(())
    }
}
