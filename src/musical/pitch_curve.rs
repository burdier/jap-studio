//! Afinación relativa al inicio y final de una nota, en centésimas de semitono.
use super::model::{EditError, NoteId, Pattern};
use std::sync::Arc;

pub const PITCH_RANGE_CENTS: i16 = 4800;
pub const MAX_PITCH_POINTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitchPoint {
    pub at: u16,
    pub cents: i16,
}

/// 0 es el ataque y 65535 es el final de la nota. El ataque implícito vale 0.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PitchCurve(Arc<[PitchPoint]>);

impl PitchCurve {
    pub fn new(mut points: Vec<PitchPoint>) -> Result<Self, EditError> {
        if points.len() > MAX_PITCH_POINTS
            || points.windows(2).any(|pair| pair[0].at >= pair[1].at)
            || points
                .iter()
                .any(|point| !(-PITCH_RANGE_CENTS..=PITCH_RANGE_CENTS).contains(&point.cents))
        {
            return Err(EditError::InvalidNote);
        }
        if let Some(last) = points.last().copied() {
            if last.at != u16::MAX {
                points.push(PitchPoint {
                    at: u16::MAX,
                    cents: last.cents,
                });
            }
            if points[0].at != 0 {
                points.insert(0, PitchPoint { at: 0, cents: 0 });
            }
            if points.len() > MAX_PITCH_POINTS {
                return Err(EditError::InvalidNote);
            }
        }
        Ok(Self(points.into()))
    }

    pub fn points(&self) -> &[PitchPoint] {
        &self.0
    }

    pub fn cents_at(&self, at: u16) -> i16 {
        let right = self.0.partition_point(|point| point.at <= at);
        let left = if right == 0 {
            PitchPoint { at: 0, cents: 0 }
        } else {
            self.0[right - 1]
        };
        let Some(next) = self.0.get(right) else {
            return left.cents;
        };
        let fraction = f64::from(at - left.at) / f64::from(next.at - left.at);
        (f64::from(left.cents) + fraction * f64::from(next.cents - left.cents)).round() as i16
    }

    /// Recortar una nota mantiene la afinación del instante que pasa a ser su ataque.
    pub fn window(&self, offset: i64, length: i64, duration: i64) -> Self {
        if self.0.is_empty() || (offset == 0 && length == duration) {
            return self.clone();
        }
        let first = pitch_fraction(offset, duration);
        let last = pitch_fraction(offset + length, duration);
        let mut points = vec![PitchPoint {
            at: 0,
            cents: self.cents_at(first),
        }];
        points.extend(
            self.0
                .iter()
                .filter(|point| point.at > first && point.at < last)
                .map(|point| PitchPoint {
                    at: pitch_fraction(i64::from(point.at - first), i64::from(last - first)),
                    cents: point.cents,
                }),
        );
        points.push(PitchPoint {
            at: u16::MAX,
            cents: self.cents_at(last),
        });
        Self(points.into())
    }
}

pub fn pitch_fraction(offset: i64, duration: i64) -> u16 {
    if duration <= 0 {
        return 0;
    }
    let offset = i128::from(offset.clamp(0, duration));
    ((offset * i128::from(u16::MAX) + i128::from(duration) / 2) / i128::from(duration)) as u16
}

impl Pattern {
    pub fn set_pitch_curve(&mut self, id: NoteId, curve: PitchCurve) -> Result<(), EditError> {
        let note = self
            .notes
            .iter_mut()
            .find(|note| note.id == id)
            .ok_or(EditError::UnknownNote)?;
        note.pitch_curve = curve;
        Ok(())
    }
}
