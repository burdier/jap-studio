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
    /// Swing / Groove por patrón (`0..=75%`), retrasa los pasos impares de la rejilla.
    pub swing: u8,
}

impl DrumPattern {
    pub const MAX_SWING: u8 = 75;

    pub fn bar_count(&self) -> i64 {
        (self.notes.length().0 - 1) / self.meter.ticks_per_bar() + 1
    }

    pub fn step_count(&self) -> usize {
        ((self.notes.length().0 - 1) / self.grid.ticks().0 + 1) as usize
    }

    /// Devuelve el patrón con el swing aplicado sobre los pasos impares de `self.grid`.
    /// Si `swing == 0`, presta el patrón original sin copiar.
    pub fn swung_notes(&self) -> Result<Cow<'_, Pattern>, String> {
        if self.swing == 0 || self.notes.notes().is_empty() {
            return Ok(Cow::Borrowed(&self.notes));
        }
        let step_ticks = self.grid.ticks().0.max(1);
        let length = self.notes.length().0.max(1);
        let pct = i64::from(self.swing.min(Self::MAX_SWING));
        let shift = (step_ticks * pct + 50) / 100;
        if shift == 0 {
            return Ok(Cow::Borrowed(&self.notes));
        }
        let mut notes = self.notes.notes().to_vec();
        for note in &mut notes {
            let step_index = note.start.0 / step_ticks;
            if step_index % 2 == 1 {
                let shifted = (note.start.0 + shift).min(length - 1);
                note.start = Tick(shifted);
                note.duration = Tick(note.duration.0.min(length - shifted).max(1));
            }
        }
        self.notes
            .with_notes(notes, self.notes.next_id())
            .map(Cow::Owned)
            .map_err(|error| error.to_string())
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
                swing: 0,
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

    /// Elimina una pieza del kit y limpia sus notas en todos los patrones de la sesión.
    pub fn remove_piece(
        &mut self,
        lane: usize,
    ) -> Result<crate::musical::model::InstrumentId, String> {
        let removed_id = self.kit.remove_piece(lane)?;
        for pattern in &mut self.patterns {
            let had_piece = pattern
                .notes
                .notes()
                .iter()
                .any(|note| note.instrument == removed_id);
            if had_piece {
                let kept = pattern
                    .notes
                    .notes()
                    .iter()
                    .filter(|note| note.instrument != removed_id)
                    .cloned()
                    .collect();
                pattern.notes = pattern
                    .notes
                    .with_notes(kept, pattern.notes.next_id())
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(removed_id)
    }

    /// Elimina un patrón de la sesión y sus ocurrencias en Canción.
    /// Siempre conserva al menos un patrón activo.
    pub fn delete_pattern(&mut self, id: DrumPatternId) -> Result<(), String> {
        if self.patterns.len() <= 1 {
            return Err("Debe existir al menos un patrón".into());
        }
        let index = self
            .patterns
            .iter()
            .position(|pattern| pattern.id == id)
            .ok_or("Patrón de batería inexistente")?;
        self.patterns.remove(index);
        self.song.retain(|cell| cell.pattern != id);
        if self.active == id {
            let next_index = index.saturating_sub(1).min(self.patterns.len() - 1);
            self.active = self.patterns[next_index].id;
        }
        Ok(())
    }

    /// Vacía todas las notas del patrón activo conservando duración, rejilla y swing.
    pub fn clear_active_pattern(&mut self) -> Result<(), String> {
        let pattern = &mut self.active_pattern_mut().notes;
        *pattern = pattern
            .with_notes(Vec::new(), pattern.next_id())
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn insert_pattern(&mut self, notes: Pattern) -> Result<DrumPatternId, String> {
        let id = DrumPatternId(self.next_pattern_id);
        let next = self
            .next_pattern_id
            .checked_add(1)
            .ok_or("IDs de patrón agotados")?;
        let DrumPattern {
            meter, grid, swing, ..
        } = *self.active_pattern();
        self.patterns.push(DrumPattern {
            id,
            name: format!("Patrón {}", id.0),
            notes,
            meter,
            grid,
            swing,
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
            DrumPlaybackSource::Pattern => self.active_pattern().swung_notes().map(Some),
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
                    let swung = pattern.swung_notes()?;
                    for note in swung.notes() {
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
                || pattern.swing > DrumPattern::MAX_SWING
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_piece_cleans_all_patterns_and_allows_empty_kit() {
        let mut session = DrumSession::default();
        let kick = session.kit.add_sample("kick.wav".into()).unwrap();
        let snare = session.kit.add_sample("snare.wav".into()).unwrap();
        let p1 = session.active;
        session
            .active_pattern_mut()
            .notes
            .add_note(kick, Tick(0), Tick(120), 36, 100)
            .unwrap();
        session
            .active_pattern_mut()
            .notes
            .add_note(snare, Tick(480), Tick(120), 38, 100)
            .unwrap();
        let p2 = session.duplicate_active().unwrap();
        assert_eq!(session.pattern(p2).unwrap().notes.notes().len(), 2);

        let removed = session.remove_piece(0).unwrap();
        assert_eq!(removed, kick);
        assert_eq!(session.kit.pieces.len(), 1);
        assert_eq!(session.pattern(p1).unwrap().notes.notes().len(), 1);
        assert_eq!(session.pattern(p2).unwrap().notes.notes().len(), 1);
        assert!(
            session
                .pattern(p1)
                .unwrap()
                .notes
                .notes()
                .iter()
                .all(|n| n.instrument == snare)
        );
        session.validate().unwrap();

        let removed_last = session.remove_piece(0).unwrap();
        assert_eq!(removed_last, snare);
        assert!(session.kit.pieces.is_empty());
        assert!(session.pattern(p1).unwrap().notes.notes().is_empty());
        assert!(session.pattern(p2).unwrap().notes.notes().is_empty());
        session.validate().unwrap();
    }

    #[test]
    fn delete_pattern_removes_song_cells_and_selects_neighbor() {
        let mut session = DrumSession::default();
        let p1 = session.active;
        let p2 = session.new_pattern().unwrap();
        let p3 = session.new_pattern().unwrap();
        session
            .set_song_cell(
                DrumSongCell {
                    at: Tick(0),
                    pattern: p1,
                },
                true,
            )
            .unwrap();
        session
            .set_song_cell(
                DrumSongCell {
                    at: Tick(1920),
                    pattern: p2,
                },
                true,
            )
            .unwrap();
        session
            .set_song_cell(
                DrumSongCell {
                    at: Tick(3840),
                    pattern: p3,
                },
                true,
            )
            .unwrap();

        session.select(p2).unwrap();
        session.delete_pattern(p2).unwrap();
        assert_eq!(session.active, p1);
        assert_eq!(session.patterns.len(), 2);
        assert!(session.song.iter().all(|cell| cell.pattern != p2));
        session.validate().unwrap();

        session.delete_pattern(p3).unwrap();
        assert_eq!(session.patterns.len(), 1);
        assert!(session.delete_pattern(p1).is_err());
        session.validate().unwrap();
    }

    #[test]
    fn clear_active_pattern_only_clears_active() {
        let mut session = DrumSession::default();
        let kick = session.kit.add_sample("kick.wav".into()).unwrap();
        let p1 = session.active;
        session
            .active_pattern_mut()
            .notes
            .add_note(kick, Tick(0), Tick(120), 36, 100)
            .unwrap();
        let p2 = session.duplicate_active().unwrap();
        assert_eq!(session.active, p2);

        session.clear_active_pattern().unwrap();
        assert!(session.pattern(p2).unwrap().notes.notes().is_empty());
        assert_eq!(session.pattern(p1).unwrap().notes.notes().len(), 1);
        session.validate().unwrap();
    }

    #[test]
    fn add_sliced_sample_creates_choke_group_and_valid_ranges() {
        let mut session = DrumSession::default();
        let slices = vec![(0.0, 25.0), (25.0, 50.0), (50.0, 75.0), (75.0, 100.0)];
        let created = session
            .kit
            .add_sliced_sample("break.wav".into(), "Break", &slices, Some(1), true)
            .unwrap();
        assert_eq!(created.len(), 4);
        assert_eq!(session.kit.pieces.len(), 4);
        assert_eq!(session.kit.pieces[0].name, "Break 1");
        assert_eq!(session.kit.pieces[0].choke_group, Some(1));
        assert!(!session.kit.pieces[0].sampler.filter);
        assert_eq!(
            session.kit.pieces[2]
                .sampler
                .get(crate::devices::sampler::START),
            50.0
        );
        assert_eq!(
            session.kit.pieces[2]
                .sampler
                .get(crate::devices::sampler::END),
            75.0
        );
        session.validate().unwrap();
    }
}
