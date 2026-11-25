//! Formas matemáticas puras para contornos y moldeo 2D (0..1); sin GPUI ni estado.

/// Modos de perfilado entre dos puntos `(x0, y0)` y `(x1, y1)` o sobre una secuencia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ContourShape {
    /// Solo palitos individuales (sin línea conectora).
    #[default]
    StemsOnly,
    /// Polilínea recta que une las cabezas de los puntos o traza una rampa lineal.
    Linear,
    /// Curva suave (Catmull-Rom / potencia con tensión) que une las cabezas o arquea una rampa.
    Smooth,
    /// Onda senoidal superpuesta a la rampa base entre el inicio y el fin.
    SineWave,
}

impl ContourShape {
    pub fn next(self) -> Self {
        match self {
            Self::StemsOnly => Self::Linear,
            Self::Linear => Self::Smooth,
            Self::Smooth => Self::SineWave,
            Self::SineWave => Self::StemsOnly,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::StemsOnly => "Barras",
            Self::Linear => "Rampa",
            Self::Smooth => "Curva",
            Self::SineWave => "Onda",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "bars" | "stems" => Some(Self::StemsOnly),
            "linear" | "ramp" => Some(Self::Linear),
            "smooth" | "curve" => Some(Self::Smooth),
            "wave" | "sine" => Some(Self::SineWave),
            _ => None,
        }
    }

    /// Evalúa la altura `y` (0..1) en una posición `t` (0..1 entre `x0` y `x1`).
    /// `tension` en `-1.0..=1.0` curva la rampa en `Smooth` o ajusta ciclos/amplitud en `SineWave`.
    pub fn eval_span(self, y0: f32, y1: f32, t: f32, tension: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::StemsOnly | Self::Linear => y0 + (y1 - y0) * t,
            Self::Smooth => {
                let power = 2.0_f32.powf(-tension.clamp(-1.0, 1.0) * 2.0);
                let curved = t.powf(power);
                (y0 + (y1 - y0) * curved).clamp(0.0, 1.0)
            }
            Self::SineWave => {
                let base = y0 + (y1 - y0) * t;
                let cycles = 2.0 + (tension.abs() * 4.0).round();
                let amp = (0.18 + tension.abs() * 0.17).min(0.45);
                let sign = if tension >= 0.0 { 1.0 } else { -1.0 };
                let wave = (t * cycles * std::f32::consts::TAU).sin() * amp * sign;
                (base + wave).clamp(0.0, 1.0)
            }
        }
    }
}

/// Patrones listos para moldear un carril de propiedades de nota como una sola onda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanePreset {
    Flat,
    RampUp,
    RampDown,
    EaseIn,
    EaseOut,
    Triangle,
    Sine,
    SawUp,
    SawDown,
    Square,
    Random,
    Invert,
}

impl LanePreset {
    pub const ALL: [Self; 12] = [
        Self::Flat,
        Self::RampUp,
        Self::RampDown,
        Self::EaseIn,
        Self::EaseOut,
        Self::Triangle,
        Self::Sine,
        Self::SawUp,
        Self::SawDown,
        Self::Square,
        Self::Random,
        Self::Invert,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "Plano",
            Self::RampUp => "Rampa ↗ (crescendo)",
            Self::RampDown => "Rampa ↘ (decrescendo)",
            Self::EaseIn => "Curva lenta → rápida",
            Self::EaseOut => "Curva rápida → lenta",
            Self::Triangle => "Triángulo",
            Self::Sine => "Onda senoidal",
            Self::SawUp => "Diente de sierra ↗",
            Self::SawDown => "Diente de sierra ↘",
            Self::Square => "Cuadrada",
            Self::Random => "Aleatorio",
            Self::Invert => "Invertir valores",
        }
    }

    /// Altura `0..1` en la posición `t` (`0..1`) con `cycles` repeticiones; `index` solo alimenta `Random`.
    /// `Invert` devuelve `1 - current`.
    pub fn eval(self, t: f32, cycles: f32, index: usize, current: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let phase = (t * cycles).fract();
        let value = match self {
            Self::Flat => 0.75,
            Self::RampUp => 0.15 + 0.85 * t,
            Self::RampDown => 1.0 - 0.85 * t,
            Self::EaseIn => 0.15 + 0.85 * t * t,
            Self::EaseOut => 0.15 + 0.85 * (1.0 - (1.0 - t) * (1.0 - t)),
            Self::Triangle => 0.15 + 0.85 * (1.0 - (phase * 2.0 - 1.0).abs()),
            Self::Sine => 0.575 + 0.425 * (t * cycles * std::f32::consts::TAU).sin(),
            Self::SawUp => 0.15 + 0.85 * phase,
            Self::SawDown => 1.0 - 0.85 * phase,
            Self::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    0.4
                }
            }
            Self::Random => {
                let h = (index as u32 + 1).wrapping_mul(2_654_435_761) >> 8;
                0.3 + 0.7 * (h % 1000) as f32 / 999.0
            }
            Self::Invert => 1.0 - current,
        };
        value.clamp(0.0, 1.0)
    }
}

/// Genera una polilínea `(x, y)` que conecta los `anchors` ordenados por `x` según `shape`.
pub fn connect_anchors(
    anchors: &[(f32, f32)],
    shape: ContourShape,
    subdivisions: usize,
) -> Vec<(f32, f32)> {
    if anchors.is_empty() || shape == ContourShape::StemsOnly {
        return Vec::new();
    }
    if anchors.len() == 1 || shape == ContourShape::Linear {
        return anchors.to_vec();
    }
    let steps = subdivisions.max(4);
    let mut out = Vec::with_capacity((anchors.len() - 1) * steps + 1);
    for idx in 0..anchors.len() - 1 {
        let p0 = if idx > 0 {
            anchors[idx - 1]
        } else {
            anchors[idx]
        };
        let p1 = anchors[idx];
        let p2 = anchors[idx + 1];
        let p3 = if idx + 2 < anchors.len() {
            anchors[idx + 2]
        } else {
            anchors[idx + 1]
        };
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let x = p1.0 + (p2.0 - p1.0) * t;
            let y = match shape {
                ContourShape::Smooth => catmull_rom(p0.1, p1.1, p2.1, p3.1, t).clamp(0.0, 1.0),
                ContourShape::SineWave => {
                    let base = p1.1 + (p2.1 - p1.1) * t;
                    let amp = ((p2.1 - p1.1).abs() * 0.35).clamp(0.05, 0.25);
                    (base + (t * std::f32::consts::TAU).sin() * amp).clamp(0.0, 1.0)
                }
                _ => p1.1 + (p2.1 - p1.1) * t,
            };
            out.push((x, y));
        }
    }
    if let Some(&last) = anchors.last() {
        out.push(last);
    }
    out
}

/// Genera los puntos `(x, y)` de previsualización de un trazo entre `from` y `to`.
pub fn span_preview(
    from: (f32, f32),
    to: (f32, f32),
    shape: ContourShape,
    tension: f32,
    samples: usize,
) -> Vec<(f32, f32)> {
    let n = samples.max(2);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let x = from.0 + (to.0 - from.0) * t;
            let y = shape.eval_span(from.1, to.1, t, tension);
            (x, y)
        })
        .collect()
}

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_and_smooth_spans_respect_endpoints() {
        for shape in [
            ContourShape::Linear,
            ContourShape::Smooth,
            ContourShape::SineWave,
        ] {
            assert!((shape.eval_span(0.2, 0.8, 0.0, 0.4) - 0.2).abs() < 1e-4);
            assert!((shape.eval_span(0.2, 0.8, 1.0, 0.4) - 0.8).abs() < 1e-4);
        }
        let mid_convex = ContourShape::Smooth.eval_span(0.0, 1.0, 0.5, 0.5);
        let mid_concave = ContourShape::Smooth.eval_span(0.0, 1.0, 0.5, -0.5);
        assert!(mid_convex > 0.5 && mid_concave < 0.5);
    }

    #[test]
    fn presets_stay_in_range_and_ramps_move() {
        for preset in LanePreset::ALL {
            for i in 0..=10 {
                let v = preset.eval(i as f32 / 10.0, 1.0, i, 0.3);
                assert!((0.0..=1.0).contains(&v));
            }
        }
        assert!(
            LanePreset::RampUp.eval(1.0, 1.0, 0, 0.0) > LanePreset::RampUp.eval(0.0, 1.0, 0, 0.0)
        );
        assert!((LanePreset::Invert.eval(0.5, 1.0, 0, 0.25) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn connect_anchors_builds_continuous_curve() {
        let pts = [(0.1, 0.2), (0.5, 0.9), (0.9, 0.4)];
        let curve = connect_anchors(&pts, ContourShape::Smooth, 8);
        assert!(curve.len() > pts.len());
        assert_eq!(curve.first().copied(), Some(pts[0]));
        assert_eq!(curve.last().copied(), Some(pts[2]));
    }
}
