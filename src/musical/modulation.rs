//! Modulación LFO headless (sin GPUI ni CPAL) según `docs/VISUAL_MODULATION.md`.
//!
//! Cada fuente LFO tiene identidad estable (`ModSourceId`) y se evalúa de forma pura y
//! determinista a partir del tiempo musical (`beat`) y el tempo (`bpm`), garantizando
//! resultados idénticos en reproducción RT, seek, loop y exportación offline sin asignaciones.
use super::{track::TrackId, track_automation::AutomationParam};
use std::f64::consts::TAU;

pub const MAX_LFOS: usize = 64;
pub const MAX_MOD_ROUTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModSourceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
    SampleAndHold,
}

impl LfoShape {
    pub const ALL: [Self; 6] = [
        Self::Sine,
        Self::Triangle,
        Self::SawUp,
        Self::SawDown,
        Self::Square,
        Self::SampleAndHold,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::SawUp => "saw_up",
            Self::SawDown => "saw_down",
            Self::Square => "square",
            Self::SampleAndHold => "sh",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sine => "Seno",
            Self::Triangle => "Triángulo",
            Self::SawUp => "Sierra ↑",
            Self::SawDown => "Sierra ↓",
            Self::Square => "Cuadrada",
            Self::SampleAndHold => "S&H",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "sine" | "seno" | "sin" => Some(Self::Sine),
            "tri" | "triangle" | "triangulo" | "triángulo" => Some(Self::Triangle),
            "saw" | "saw_up" | "sawup" | "ramp" | "sierra" => Some(Self::SawUp),
            "saw_down" | "sawdown" | "ramp_down" => Some(Self::SawDown),
            "square" | "sq" | "cuadrada" | "pulse" => Some(Self::Square),
            "sh" | "s&h" | "sample_hold" | "sampleandhold" | "random" | "rand" => {
                Some(Self::SampleAndHold)
            }
            _ => None,
        }
    }

    /// Evalúa la forma de onda en el rango bipolar `[-1.0, 1.0]` para el número de ciclo `cycles`.
    #[inline]
    pub fn sample(self, cycles: f64) -> f32 {
        let cycle_index = cycles.floor() as i64;
        let phase = cycles.rem_euclid(1.0);
        match self {
            Self::Sine => (phase * TAU).sin() as f32,
            Self::Triangle => {
                let v = if phase < 0.25 {
                    phase * 4.0
                } else if phase < 0.75 {
                    2.0 - phase * 4.0
                } else {
                    phase * 4.0 - 4.0
                };
                v as f32
            }
            Self::SawUp => (phase * 2.0 - 1.0) as f32,
            Self::SawDown => (1.0 - phase * 2.0) as f32,
            Self::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Self::SampleAndHold => {
                // Hash determinista (SplitMix64) por índice de ciclo: idéntico en Play, Seek y Export.
                let mut x = (cycle_index as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
                x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                x ^= x >> 31;
                let unit = (x >> 40) as f64 / ((1u64 << 24) - 1) as f64;
                (unit * 2.0 - 1.0) as f32
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfoDivision {
    Bars4,
    Bars2,
    Bar1,
    Half,
    #[default]
    Quarter,
    QuarterDot,
    Eighth,
    EighthTriplet,
    Sixteenth,
    ThirtySecond,
}

impl LfoDivision {
    pub const ALL: [Self; 10] = [
        Self::Bars4,
        Self::Bars2,
        Self::Bar1,
        Self::Half,
        Self::Quarter,
        Self::QuarterDot,
        Self::Eighth,
        Self::EighthTriplet,
        Self::Sixteenth,
        Self::ThirtySecond,
    ];

    pub fn beats(self) -> f64 {
        match self {
            Self::Bars4 => 16.0,
            Self::Bars2 => 8.0,
            Self::Bar1 => 4.0,
            Self::Half => 2.0,
            Self::Quarter => 1.0,
            Self::QuarterDot => 1.5,
            Self::Eighth => 0.5,
            Self::EighthTriplet => 1.0 / 3.0,
            Self::Sixteenth => 0.25,
            Self::ThirtySecond => 0.125,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Bars4 => "4bars",
            Self::Bars2 => "2bars",
            Self::Bar1 => "1bar",
            Self::Half => "1/2",
            Self::Quarter => "1/4",
            Self::QuarterDot => "1/4d",
            Self::Eighth => "1/8",
            Self::EighthTriplet => "1/8t",
            Self::Sixteenth => "1/16",
            Self::ThirtySecond => "1/32",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "4bars" | "4bar" | "4c" | "4/1" => Some(Self::Bars4),
            "2bars" | "2bar" | "2c" | "2/1" => Some(Self::Bars2),
            "1bar" | "1c" | "1/1" | "bar" => Some(Self::Bar1),
            "1/2" | "half" => Some(Self::Half),
            "1/4" | "quarter" | "beat" => Some(Self::Quarter),
            "1/4d" | "1/4." => Some(Self::QuarterDot),
            "1/8" | "eighth" => Some(Self::Eighth),
            "1/8t" => Some(Self::EighthTriplet),
            "1/16" | "sixteenth" => Some(Self::Sixteenth),
            "1/32" => Some(Self::ThirtySecond),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LfoRate {
    Hz(f32),
    Sync(LfoDivision),
}

impl Default for LfoRate {
    fn default() -> Self {
        Self::Sync(LfoDivision::Bar1)
    }
}

impl LfoRate {
    pub const MIN_HZ: f32 = 0.02;
    pub const MAX_HZ: f32 = 40.0;

    pub fn hz(value: f32) -> Self {
        let clamped = if value.is_finite() {
            value.clamp(Self::MIN_HZ, Self::MAX_HZ)
        } else {
            1.0
        };
        Self::Hz(clamped)
    }

    pub fn parse(token: &str) -> Option<Self> {
        let clean = token.trim().to_ascii_lowercase();
        if let Some(div) = LfoDivision::parse(&clean) {
            return Some(Self::Sync(div));
        }
        let numeric = clean.strip_suffix("hz").unwrap_or(&clean).trim();
        let val = numeric.parse::<f32>().ok()?;
        if val.is_finite() && val > 0.0 {
            Some(Self::hz(val))
        } else {
            None
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Hz(hz) => {
                if hz >= 10.0 {
                    format!("{hz:.1} Hz")
                } else {
                    format!("{hz:.2} Hz")
                }
            }
            Self::Sync(div) => div.key().to_string(),
        }
    }

    #[inline]
    pub fn cycles_at(self, beat: f64, bpm: f64) -> f64 {
        let b = if beat.is_finite() { beat.max(0.0) } else { 0.0 };
        match self {
            Self::Sync(div) => b / div.beats(),
            Self::Hz(hz) => {
                let safe_bpm = if bpm.is_finite() && bpm > 1.0 {
                    bpm
                } else {
                    120.0
                };
                let seconds = b * 60.0 / safe_bpm;
                seconds * f64::from(hz.clamp(Self::MIN_HZ, Self::MAX_HZ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfoPolarity {
    #[default]
    Bipolar,
    Unipolar,
}

impl LfoPolarity {
    pub fn key(self) -> &'static str {
        match self {
            Self::Bipolar => "bipolar",
            Self::Unipolar => "unipolar",
        }
    }

    pub fn parse(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "bipolar" | "bi" | "+-" => Some(Self::Bipolar),
            "unipolar" | "uni" | "+" => Some(Self::Unipolar),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LfoSpec {
    pub id: ModSourceId,
    pub name: String,
    pub shape: LfoShape,
    pub rate: LfoRate,
    /// Desfase inicial normalizado `0.0..1.0`.
    pub phase: f32,
    pub polarity: LfoPolarity,
    pub enabled: bool,
}

impl LfoSpec {
    pub fn new(id: ModSourceId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            shape: LfoShape::Sine,
            rate: LfoRate::default(),
            phase: 0.0,
            polarity: LfoPolarity::Bipolar,
            enabled: true,
        }
    }

    /// Evalúa la señal de control en `[-1.0, 1.0]` (bipolar) o `[0.0, 1.0]` (unipolar).
    #[inline]
    pub fn evaluate(&self, beat: f64, bpm: f64) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let phase_offset = if self.phase.is_finite() {
            f64::from(self.phase.rem_euclid(1.0))
        } else {
            0.0
        };
        let cycles = self.rate.cycles_at(beat, bpm) + phase_offset;
        let raw = self.shape.sample(cycles);
        match self.polarity {
            LfoPolarity::Bipolar => raw,
            LfoPolarity::Unipolar => (raw * 0.5 + 0.5).clamp(0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModRoute {
    pub id: u32,
    pub source: ModSourceId,
    pub track: TrackId,
    pub param: AutomationParam,
    /// Profundidad bipolar normalizada `-1.0..=1.0`.
    pub depth: f32,
    pub enabled: bool,
}

/// Ruta precompilada por pista para evaluación RT sin asignaciones ni búsquedas en tablas.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModRoute {
    pub shape: LfoShape,
    pub rate: LfoRate,
    pub phase: f32,
    pub polarity: LfoPolarity,
    pub param: AutomationParam,
    pub depth: f32,
}

impl CompiledModRoute {
    #[inline]
    pub fn evaluate(&self, beat: f64, bpm: f64) -> f32 {
        let phase_offset = if self.phase.is_finite() {
            f64::from(self.phase.rem_euclid(1.0))
        } else {
            0.0
        };
        let cycles = self.rate.cycles_at(beat, bpm) + phase_offset;
        let raw = self.shape.sample(cycles);
        let signal = match self.polarity {
            LfoPolarity::Bipolar => raw,
            LfoPolarity::Unipolar => (raw * 0.5 + 0.5).clamp(0.0, 1.0),
        };
        signal * self.depth
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackModulation {
    pub routes: Vec<CompiledModRoute>,
}

impl TrackModulation {
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn has_param(&self, param: AutomationParam) -> bool {
        self.routes.iter().any(|r| r.param == param)
    }

    /// Suma los aportes de todos los LFOs que apuntan a `param` en dominio normalizado `[-1.0, 1.0]`.
    #[inline]
    pub fn offset_for(&self, param: AutomationParam, beat: f64, bpm: f64) -> f32 {
        let mut sum = 0.0_f32;
        for route in &self.routes {
            if route.param == param {
                sum += route.evaluate(beat, bpm);
            }
        }
        sum.clamp(-1.0, 1.0)
    }

    /// Aplica modulación normalizada sobre un valor base `0.0..=1.0`.
    #[inline]
    pub fn modulate_normalized(
        &self,
        param: AutomationParam,
        base: f32,
        beat: f64,
        bpm: f64,
    ) -> f32 {
        let delta = self.offset_for(param, beat, bpm);
        apply_normalized(base, delta)
    }
}

/// Suma `delta` sobre un valor base normalizado `0.0..=1.0`, respetando límites.
#[inline]
pub fn apply_normalized(base: f32, delta: f32) -> f32 {
    let b = if base.is_finite() { base } else { 0.5 };
    let d = if delta.is_finite() { delta } else { 0.0 };
    (b + d).clamp(0.0, 1.0)
}

/// Convierte un valor normalizado modulado a escala logarítmica (ideal para cutoff / frecuencia).
#[inline]
pub fn apply_log_range(base_norm: f32, delta: f32, min_val: f32, max_val: f32) -> f32 {
    let norm = apply_normalized(base_norm, delta);
    let lo = min_val.max(1e-4);
    let hi = max_val.max(lo);
    lo * (hi / lo).powf(norm)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModulationMatrix {
    pub lfos: Vec<LfoSpec>,
    pub routes: Vec<ModRoute>,
}

impl ModulationMatrix {
    pub fn is_empty(&self) -> bool {
        self.lfos.is_empty() && self.routes.is_empty()
    }

    pub fn next_lfo_id(&self) -> ModSourceId {
        let max_id = self.lfos.iter().map(|l| l.id.0).max().unwrap_or(0);
        ModSourceId(max_id.saturating_add(1))
    }

    pub fn next_route_id(&self) -> u32 {
        self.routes
            .iter()
            .map(|r| r.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    pub fn add_lfo(
        &mut self,
        name: Option<String>,
        shape: LfoShape,
        rate: LfoRate,
    ) -> Result<ModSourceId, String> {
        if self.lfos.len() >= MAX_LFOS {
            return Err(format!("Límite de {MAX_LFOS} LFOs alcanzado"));
        }
        let id = self.next_lfo_id();
        let label = name.unwrap_or_else(|| format!("LFO {}", id.0));
        let mut spec = LfoSpec::new(id, label);
        spec.shape = shape;
        spec.rate = rate;
        self.lfos.push(spec);
        Ok(id)
    }

    pub fn lfo(&self, id: ModSourceId) -> Option<&LfoSpec> {
        self.lfos.iter().find(|l| l.id == id)
    }

    pub fn lfo_mut(&mut self, id: ModSourceId) -> Option<&mut LfoSpec> {
        self.lfos.iter_mut().find(|l| l.id == id)
    }

    pub fn remove_lfo(&mut self, id: ModSourceId) -> bool {
        let before = self.lfos.len();
        self.lfos.retain(|l| l.id != id);
        self.routes.retain(|r| r.source != id);
        self.lfos.len() != before
    }

    pub fn add_route(
        &mut self,
        source: ModSourceId,
        track: TrackId,
        param: AutomationParam,
        depth: f32,
    ) -> Result<u32, String> {
        if self.lfo(source).is_none() {
            return Err(format!("LFO #{} no existe", source.0));
        }
        let clamped_depth = if depth.is_finite() {
            depth.clamp(-1.0, 1.0)
        } else {
            return Err("Profundidad de modulación inválida".into());
        };
        if let Some(existing) = self
            .routes
            .iter_mut()
            .find(|r| r.source == source && r.track == track && r.param == param)
        {
            existing.depth = clamped_depth;
            existing.enabled = true;
            return Ok(existing.id);
        }
        if self.routes.len() >= MAX_MOD_ROUTES {
            return Err(format!(
                "Límite de {MAX_MOD_ROUTES} rutas de modulación alcanzado"
            ));
        }
        let id = self.next_route_id();
        self.routes.push(ModRoute {
            id,
            source,
            track,
            param,
            depth: clamped_depth,
            enabled: true,
        });
        Ok(id)
    }

    pub fn remove_route(&mut self, id: u32) -> bool {
        let before = self.routes.len();
        self.routes.retain(|r| r.id != id);
        self.routes.len() != before
    }

    pub fn remove_track(&mut self, track: TrackId) {
        self.routes.retain(|r| r.track != track);
    }

    pub fn track_route_count(&self, track: TrackId) -> usize {
        self.routes
            .iter()
            .filter(|r| {
                r.track == track
                    && r.enabled
                    && r.depth.abs() > 1e-6
                    && self.lfo(r.source).is_some_and(|l| l.enabled)
            })
            .count()
    }

    /// Compila el snapshot plano de rutas activas para `track` (publicación lock-free al motor RT).
    pub fn compile_for_track(&self, track: TrackId) -> TrackModulation {
        let mut routes = Vec::new();
        for route in &self.routes {
            if route.track != track || !route.enabled || route.depth.abs() <= 1e-6 {
                continue;
            }
            if let Some(lfo) = self.lfo(route.source)
                && lfo.enabled
            {
                routes.push(CompiledModRoute {
                    shape: lfo.shape,
                    rate: lfo.rate,
                    phase: lfo.phase,
                    polarity: lfo.polarity,
                    param: route.param,
                    depth: route.depth.clamp(-1.0, 1.0),
                });
            }
        }
        TrackModulation { routes }
    }

    /// Genera `count` muestras de un ciclo completo del LFO para previsualización en la UI.
    pub fn preview_cycle(&self, source: ModSourceId, count: usize) -> Vec<f32> {
        let Some(lfo) = self.lfo(source) else {
            return Vec::new();
        };
        let n = count.clamp(2, 256);
        let phase_offset = f64::from(lfo.phase.rem_euclid(1.0));
        (0..n)
            .map(|i| {
                let raw = lfo.shape.sample(i as f64 / n as f64 + phase_offset);
                match lfo.polarity {
                    LfoPolarity::Bipolar => raw,
                    LfoPolarity::Unipolar => (raw * 0.5 + 0.5).clamp(0.0, 1.0),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::fx::FxId;

    #[test]
    fn lfo_sine_sync_and_hz_are_deterministic() {
        let mut spec = LfoSpec::new(ModSourceId(1), "LFO 1");
        spec.shape = LfoShape::Sine;
        spec.rate = LfoRate::Sync(LfoDivision::Quarter);
        assert!(spec.evaluate(0.0, 120.0).abs() < 1e-5);
        assert!((spec.evaluate(0.25, 120.0) - 1.0).abs() < 1e-5);
        assert!((spec.evaluate(0.75, 120.0) + 1.0).abs() < 1e-5);

        // 2 Hz a 120 BPM (0.5 s por beat) = 1 ciclo completo por beat.
        spec.rate = LfoRate::hz(2.0);
        assert!((spec.evaluate(0.25, 120.0) - 1.0).abs() < 1e-5);
        assert!((spec.evaluate(0.75, 120.0) + 1.0).abs() < 1e-5);
    }

    #[test]
    fn sample_and_hold_is_repeatable_across_seeks_and_polarity() {
        let mut spec = LfoSpec::new(ModSourceId(1), "S&H");
        spec.shape = LfoShape::SampleAndHold;
        spec.rate = LfoRate::Sync(LfoDivision::Eighth);
        let a = spec.evaluate(0.1, 128.0);
        let b = spec.evaluate(0.4, 128.0);
        let next_cycle = spec.evaluate(0.6, 128.0);
        assert_eq!(a, b);
        assert_ne!(a, next_cycle);
        assert!((-1.0..=1.0).contains(&a));

        spec.polarity = LfoPolarity::Unipolar;
        let u = spec.evaluate(0.1, 128.0);
        assert!((0.0..=1.0).contains(&u));
        assert!((u - (a * 0.5 + 0.5)).abs() < 1e-5);
    }

    #[test]
    fn modulation_matrix_compiles_per_track_and_applies_log_cutoff() {
        let mut matrix = ModulationMatrix::default();
        let lfo1 = matrix
            .add_lfo(None, LfoShape::Square, LfoRate::Sync(LfoDivision::Quarter))
            .unwrap();
        let cutoff = AutomationParam::FxParam {
            fx: FxId(7),
            param: 0,
        };
        matrix.add_route(lfo1, TrackId(1), cutoff, 0.5).unwrap();
        matrix
            .add_route(lfo1, TrackId(2), AutomationParam::Pan, 0.4)
            .unwrap();

        let track1 = matrix.compile_for_track(TrackId(1));
        assert_eq!(track1.routes.len(), 1);
        assert!((track1.offset_for(cutoff, 0.1, 120.0) - 0.5).abs() < 1e-5);
        assert!((track1.offset_for(cutoff, 0.6, 120.0) + 0.5).abs() < 1e-5);

        let hz_mid = apply_log_range(0.5, 0.0, 20.0, 20_000.0);
        let hz_up = apply_log_range(0.5, 0.5, 20.0, 20_000.0);
        assert!((hz_mid - 632.4555).abs() < 1.0);
        assert!((hz_up - 20_000.0).abs() < 1.0);
    }
}
