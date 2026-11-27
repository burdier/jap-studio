//! Propiedades por nota: datos musicales y decisiones reproducibles, sin DSP ni UI.
use super::model::{EditError, NoteEvent, NoteId, Pattern};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteExpression {
    pub release_velocity: u8,
    pub probability: u8,
    pub pan: i8,
    /// Eje Y MPE (CC 74 / Slide / Timbre por nota, 0..=127, neutro = 64).
    pub slide: u8,
    /// Eje Z MPE (Presión / Aftertouch por nota, 0..=127, neutro = 0).
    pub pressure: u8,
}
impl Default for NoteExpression {
    fn default() -> Self {
        Self {
            release_velocity: 64,
            probability: 100,
            pan: 0,
            slide: 64,
            pressure: 0,
        }
    }
}
impl NoteExpression {
    pub fn valid(self) -> bool {
        self.release_velocity <= 127
            && self.probability <= 100
            && (-64..=63).contains(&self.pan)
            && self.slide <= 127
            && self.pressure <= 127
    }
    /// La misma decisión para On y Off. Cada vuelta cambia el sorteo sin guardar voces.
    pub fn plays(self, note: NoteId, cycle: u64) -> bool {
        if self.probability == 100 {
            return true;
        }
        if self.probability == 0 {
            return false;
        }
        let mut value = note.0.wrapping_add(cycle.wrapping_mul(0x9e3779b97f4a7c15));
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^= value >> 31;
        value % 100 < u64::from(self.probability)
    }
}

#[derive(Clone, Copy)]
pub enum NoteProperty {
    Velocity,
    ReleaseVelocity,
    Probability,
    Pan,
    Slide,
    Pressure,
}
impl NoteProperty {
    pub fn value(self, note: &NoteEvent) -> i16 {
        match self {
            Self::Velocity => i16::from(note.velocity),
            Self::ReleaseVelocity => i16::from(note.expression.release_velocity),
            Self::Probability => i16::from(note.expression.probability),
            Self::Pan => i16::from(note.expression.pan),
            Self::Slide => i16::from(note.expression.slide),
            Self::Pressure => i16::from(note.expression.pressure),
        }
    }
    pub fn range(self) -> (i16, i16) {
        match self {
            Self::Velocity => (1, 127),
            Self::ReleaseVelocity | Self::Slide | Self::Pressure => (0, 127),
            Self::Probability => (0, 100),
            Self::Pan => (-64, 63),
        }
    }
    pub fn fraction(self, note: &NoteEvent) -> f32 {
        let value = self.value(note);
        if matches!(self, Self::Pan) {
            0.5 + f32::from(value) / if value < 0 { 128.0 } else { 126.0 }
        } else {
            f32::from(value) / f32::from(self.range().1)
        }
    }
    pub fn value_at_fraction(self, fraction: f32) -> i16 {
        let f = fraction.clamp(0.0, 1.0);
        let value = if matches!(self, Self::Pan) {
            (f - 0.5) * if f < 0.5 { 128.0 } else { 126.0 }
        } else {
            f * f32::from(self.range().1)
        };
        let (min, max) = self.range();
        (value.round() as i16).clamp(min, max)
    }
    pub fn set(self, note: &mut NoteEvent, value: i16) -> Result<(), EditError> {
        let (min, max) = self.range();
        if !(min..=max).contains(&value) {
            return Err(EditError::InvalidNote);
        }
        match self {
            Self::Velocity => note.velocity = value as u8,
            Self::ReleaseVelocity => note.expression.release_velocity = value as u8,
            Self::Probability => note.expression.probability = value as u8,
            Self::Pan => note.expression.pan = value as i8,
            Self::Slide => note.expression.slide = value as u8,
            Self::Pressure => note.expression.pressure = value as u8,
        }
        Ok(())
    }
}

impl Pattern {
    pub fn set_note_property(
        &mut self,
        id: NoteId,
        property: NoteProperty,
        value: i16,
    ) -> Result<(), EditError> {
        let note = self
            .notes
            .iter_mut()
            .find(|note| note.id == id)
            .ok_or(EditError::UnknownNote)?;
        property.set(note, value)
    }
    pub fn add_note_copy(
        &mut self,
        source: &NoteEvent,
        start: super::time::Tick,
        duration: super::time::Tick,
        instrument: super::model::InstrumentId,
    ) -> Result<NoteId, EditError> {
        if !source.expression.valid() {
            return Err(EditError::InvalidNote);
        }
        let id = self.add_note(instrument, start, duration, source.pitch, source.velocity)?;
        if let Some(note) = self.notes.iter_mut().find(|note| note.id == id) {
            note.expression = source.expression;
            note.pitch_curve = source.pitch_curve.clone();
        }
        Ok(id)
    }
}
