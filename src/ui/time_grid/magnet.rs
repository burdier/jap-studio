//! Imán de rejilla: mide en píxeles la distancia de un borde a su línea destino.
//! Sin estado ni GPUI; el controlador decide si usa el destino capturado.
use super::TimeGrid;

/// Radio en píxeles donde la línea destino empieza a anunciarse.
const APPROACH_PX: f64 = 28.0;
/// Radio en píxeles donde el borde queda pegado a la línea.
const CAPTURE_PX: f64 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct MagnetHit {
    /// Beat de la línea destino.
    pub target: f64,
    /// 0 al entrar en el radio de aproximación, 1 al quedar capturado.
    pub proximity: f32,
    pub captured: bool,
}

impl TimeGrid {
    /// Línea más cercana a `beat` en una rejilla de `step` beats.
    pub fn magnet(self, beat: f64, step: f64) -> Option<MagnetHit> {
        self.magnet_to(beat, self.snap(beat, step).max(0.0), step)
    }

    /// Destino ya calculado por el snap del gesto (p. ej. snap relativo de notas).
    /// Los radios se reducen con el zoom para no capturar siempre en rejillas densas.
    pub fn magnet_to(self, beat: f64, target: f64, step: f64) -> Option<MagnetHit> {
        if !(step > 0.0) || !beat.is_finite() || !target.is_finite() || self.pixels_per_beat <= 0.0
        {
            return None;
        }
        let step_px = step * self.pixels_per_beat;
        let approach = APPROACH_PX.min(step_px * 0.5);
        let capture = CAPTURE_PX.min(step_px * 0.3);
        let distance = (beat - target).abs() * self.pixels_per_beat;
        if distance > approach {
            return None;
        }
        let proximity = if approach > capture {
            1.0 - ((distance - capture).max(0.0) / (approach - capture))
        } else {
            1.0
        };
        Some(MagnetHit {
            target,
            proximity: proximity.clamp(0.0, 1.0) as f32,
            captured: distance <= capture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announces_before_capturing_and_ignores_far_positions() {
        let grid = TimeGrid {
            first_beat: 0.0,
            pixels_per_beat: 100.0,
        };
        let near = grid.magnet(1.05, 1.0).expect("a 5 px de la línea");
        assert!(near.captured && near.target == 1.0);
        let approaching = grid.magnet(1.2, 1.0).expect("a 20 px de la línea");
        assert!(!approaching.captured && approaching.proximity > 0.0);
        assert!(grid.magnet(1.45, 1.0).is_none());
    }
}
