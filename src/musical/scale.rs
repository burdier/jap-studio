//! Pertenencia de un tono MIDI a una escala, sin depender de la interfaz.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitchScale {
    pub root: u8,
    pub intervals: u16,
}

impl PitchScale {
    pub const C_NATURAL_MINOR: Self = Self {
        root: 0,
        intervals: (1 << 0) | (1 << 2) | (1 << 3) | (1 << 5) | (1 << 7) | (1 << 8) | (1 << 10),
    };

    pub fn contains(self, pitch: u8) -> bool {
        let interval = (pitch % 12 + 12 - self.root % 12) % 12;
        self.intervals & (1 << interval) != 0
    }
}
