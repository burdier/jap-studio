//! Resolución musical independiente del zoom, compás y presentación.
use super::time::{PPQ, Tick};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteGrid(u16);

impl Default for NoteGrid {
    fn default() -> Self {
        Self(16)
    }
}

impl NoteGrid {
    pub fn new(denominator: u16) -> Result<Self, String> {
        if denominator.is_power_of_two() && denominator <= 128 {
            Ok(Self(denominator))
        } else {
            Err("Rejilla: usa 1, 2, 4, 8, 16, 32, 64 o 128".into())
        }
    }

    pub fn denominator(self) -> u16 {
        self.0
    }

    pub fn ticks(self) -> Tick {
        Tick(4 * PPQ / i64::from(self.0))
    }

    pub fn finer(self) -> Self {
        Self((self.0 * 2).min(128))
    }

    pub fn coarser(self) -> Self {
        Self((self.0 / 2).max(1))
    }
}
