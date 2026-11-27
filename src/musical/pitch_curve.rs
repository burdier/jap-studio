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

    /// Rampa lineal continua desde `from_cents` (ataque) hasta `to_cents` (final).
    pub fn glide(from_cents: i16, to_cents: i16) -> Self {
        let from = from_cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS);
        let to = to_cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS);
        Self::new(vec![
            PitchPoint { at: 0, cents: from },
            PitchPoint {
                at: u16::MAX,
                cents: to,
            },
        ])
        .unwrap_or_default()
    }

    /// Scoop expresivo: ataca desplazado (`from_cents`) y resuelve a 0 en el primer 25%.
    pub fn scoop(from_cents: i16) -> Self {
        let from = from_cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS);
        Self::new(vec![
            PitchPoint { at: 0, cents: from },
            PitchPoint {
                at: u16::MAX / 4,
                cents: 0,
            },
            PitchPoint {
                at: u16::MAX,
                cents: 0,
            },
        ])
        .unwrap_or_default()
    }

    /// Caída al final de la nota: sostiene afinación hasta el 65% y cae a `to_cents`.
    pub fn fall(to_cents: i16) -> Self {
        let to = to_cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS);
        Self::new(vec![
            PitchPoint { at: 0, cents: 0 },
            PitchPoint {
                at: (u32::from(u16::MAX) * 65 / 100) as u16,
                cents: 0,
            },
            PitchPoint {
                at: u16::MAX,
                cents: to,
            },
        ])
        .unwrap_or_default()
    }

    /// Vibrato senoidal expresivo con retardo suave de entrada (15% inicial).
    pub fn vibrato(depth_cents: i16, cycles: u8) -> Self {
        let depth = depth_cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS) as f64;
        let cycles = cycles.clamp(1, 12);
        let steps = (usize::from(cycles) * 4 + 2).min(MAX_PITCH_POINTS - 2);
        let mut points = Vec::with_capacity(steps + 2);
        points.push(PitchPoint { at: 0, cents: 0 });
        for i in 1..=steps {
            let t = i as f64 / (steps + 1) as f64;
            let at = (t * f64::from(u16::MAX))
                .round()
                .clamp(1.0, f64::from(u16::MAX - 1)) as u16;
            if points.last().is_some_and(|p| p.at >= at) {
                continue;
            }
            let onset = ((t - 0.12) / 0.22).clamp(0.0, 1.0);
            let env = onset * onset * (3.0 - 2.0 * onset);
            let wave = (t * f64::from(cycles) * std::f64::consts::TAU).sin();
            let cents = (depth * env * wave).round() as i16;
            points.push(PitchPoint {
                at,
                cents: cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS),
            });
        }
        points.push(PitchPoint {
            at: u16::MAX,
            cents: 0,
        });
        Self::new(points).unwrap_or_default()
    }

    /// Desliza el tramo final de la nota (último 35%) hacia `delta_cents` (legato hacia siguiente nota).
    pub fn tail_slide(delta_cents: i16) -> Self {
        let delta = delta_cents.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS);
        if delta == 0 {
            return Self::default();
        }
        Self::new(vec![
            PitchPoint { at: 0, cents: 0 },
            PitchPoint {
                at: (u32::from(u16::MAX) * 65 / 100) as u16,
                cents: 0,
            },
            PitchPoint {
                at: u16::MAX,
                cents: delta,
            },
        ])
        .unwrap_or_default()
    }

    /// Ajusta cada punto de la curva al semitono entero más cercano (múltiplo de 100 cents).
    pub fn quantize_semitones(&self) -> Self {
        if self.0.is_empty() {
            return Self::default();
        }
        let points = self
            .0
            .iter()
            .map(|p| {
                let snapped = ((f64::from(p.cents) / 100.0).round() * 100.0) as i16;
                PitchPoint {
                    at: p.at,
                    cents: snapped.clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS),
                }
            })
            .collect();
        Self::new(points).unwrap_or_else(|_| self.clone())
    }

    /// Invierte verticalmente la dirección de todos los bends (`-cents`).
    pub fn invert(&self) -> Self {
        if self.0.is_empty() {
            return Self::default();
        }
        let points = self
            .0
            .iter()
            .map(|p| PitchPoint {
                at: p.at,
                cents: (-p.cents).clamp(-PITCH_RANGE_CENTS, PITCH_RANGE_CENTS),
            })
            .collect();
        Self::new(points).unwrap_or_else(|_| self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpe_curve_generators_respect_bounds_and_order() {
        let g = PitchCurve::glide(0, 200);
        assert_eq!(g.cents_at(0), 0);
        assert_eq!(g.cents_at(u16::MAX), 200);
        assert_eq!(g.invert().cents_at(u16::MAX), -200);

        let scoop = PitchCurve::scoop(-200);
        assert_eq!(scoop.cents_at(0), -200);
        assert_eq!(scoop.cents_at(u16::MAX / 2), 0);

        let fall = PitchCurve::fall(-1200);
        assert_eq!(fall.cents_at(0), 0);
        assert_eq!(fall.cents_at(u16::MAX), -1200);

        let vib = PitchCurve::vibrato(50, 4);
        assert!(vib.points().len() >= 10 && vib.points().len() <= MAX_PITCH_POINTS);
        assert!(vib.points().iter().any(|p| p.cents.abs() >= 25));

        let q = PitchCurve::glide(40, 165).quantize_semitones();
        assert_eq!(q.cents_at(0), 0);
        assert_eq!(q.cents_at(u16::MAX), 200);
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
