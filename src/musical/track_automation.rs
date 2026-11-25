//! Curvas de automatización por pista en el Arrangement (headless: sin GPUI ni CPAL).
//! Los puntos se almacenan ordenados por tiempo musical (`beat`) con valores normalizados `0.0..=1.0`.
use super::track::{TrackId, UNITY_LEVEL};
use std::collections::BTreeMap;

pub const MAX_AUTOMATION_POINTS: usize = 4096;
const POINT_MERGE_EPS: f64 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum AutomationParam {
    #[default]
    Volume,
    Pan,
}

impl AutomationParam {
    pub const ALL: [Self; 2] = [Self::Volume, Self::Pan];

    pub fn label(self) -> &'static str {
        match self {
            Self::Volume => "Volumen",
            Self::Pan => "Pan",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Volume => "Vol",
            Self::Pan => "Pan",
        }
    }

    pub fn key_suffix(self) -> &'static str {
        match self {
            Self::Volume => "vol",
            Self::Pan => "pan",
        }
    }

    pub fn from_suffix(suffix: &str) -> Option<Self> {
        match suffix {
            "vol" => Some(Self::Volume),
            "pan" => Some(Self::Pan),
            _ => None,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Volume => Self::Pan,
            Self::Pan => Self::Volume,
        }
    }

    pub fn default_value(self) -> f32 {
        match self {
            Self::Volume => UNITY_LEVEL as f32 / 100.0,
            Self::Pan => 0.5,
        }
    }

    /// Convierte un valor normalizado `0.0..=1.0` a ganancia lineal (misma escala que `track_levels`).
    pub fn volume_to_gain(value: f32) -> f32 {
        let clamped = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            Self::Volume.default_value()
        };
        (clamped * 100.0 / UNITY_LEVEL as f32).clamp(0.0, 2.0)
    }

    /// Convierte un valor normalizado `0.0..=1.0` a posición estéreo `-1.0..=1.0`.
    pub fn pan_to_position(value: f32) -> f32 {
        let clamped = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.5
        };
        (clamped * 2.0 - 1.0).clamp(-1.0, 1.0)
    }

    pub fn format_value(self, value: f32) -> String {
        let v = value.clamp(0.0, 1.0);
        match self {
            Self::Volume => {
                if v <= 0.0005 {
                    "−∞ dB".into()
                } else {
                    let gain = Self::volume_to_gain(v);
                    format!("{:+.1} dB", 20.0 * gain.log10())
                }
            }
            Self::Pan => {
                let centered = ((v - 0.5) * 200.0).round() as i32;
                if centered == 0 {
                    "C".into()
                } else if centered < 0 {
                    format!("L {}", -centered)
                } else {
                    format!("R {centered}")
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutomationPoint {
    pub beat: f64,
    pub value: f32,
}

impl AutomationPoint {
    pub fn new(beat: f64, value: f32) -> Self {
        Self {
            beat: if beat.is_finite() { beat.max(0.0) } else { 0.0 },
            value: if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.5
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AutomationLane {
    points: Vec<AutomationPoint>,
}

impl AutomationLane {
    pub fn points(&self) -> &[AutomationPoint] {
        &self.points
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    pub fn restore(mut points: Vec<AutomationPoint>) -> Result<Self, String> {
        if points.len() > MAX_AUTOMATION_POINTS {
            return Err("Demasiados puntos de automatización en una pista".into());
        }
        for point in &mut points {
            if !point.beat.is_finite()
                || point.beat < 0.0
                || !point.value.is_finite()
                || !(0.0..=1.0).contains(&point.value)
            {
                return Err("Punto de automatización inválido".into());
            }
        }
        points.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        let mut deduped: Vec<AutomationPoint> = Vec::with_capacity(points.len());
        for point in points {
            if let Some(last) = deduped.last_mut()
                && (point.beat - last.beat).abs() < POINT_MERGE_EPS
            {
                last.value = point.value;
            } else {
                deduped.push(point);
            }
        }
        Ok(Self { points: deduped })
    }

    /// Evaluación determinista y sin reservas (segura para el callback de audio RT).
    /// Devuelve `None` si el carril no tiene puntos (se usa el valor estático de la pista).
    pub fn evaluate(&self, beat: f64) -> Option<f32> {
        let first = self.points.first()?;
        if !beat.is_finite() || beat <= first.beat {
            return Some(first.value);
        }
        let last = self.points.last().unwrap();
        if beat >= last.beat {
            return Some(last.value);
        }
        let idx = self.points.partition_point(|p| p.beat <= beat);
        let right = self.points[idx];
        let left = self.points[idx - 1];
        let span = right.beat - left.beat;
        if span <= 1e-9 {
            return Some(right.value);
        }
        let t = ((beat - left.beat) / span).clamp(0.0, 1.0) as f32;
        Some(left.value + (right.value - left.value) * t)
    }

    /// Inserta un punto o actualiza uno existente si cae en el mismo instante (`POINT_MERGE_EPS`).
    pub fn upsert(&mut self, beat: f64, value: f32) -> usize {
        let point = AutomationPoint::new(beat, value);
        if let Some(existing) = self
            .points
            .iter()
            .position(|p| (p.beat - point.beat).abs() < POINT_MERGE_EPS)
        {
            self.points[existing] = point;
            return existing;
        }
        if self.points.len() >= MAX_AUTOMATION_POINTS {
            return self.points.len().saturating_sub(1);
        }
        let idx = self.points.partition_point(|p| p.beat < point.beat);
        self.points.insert(idx, point);
        idx
    }

    /// Mueve un punto preservando el orden cronológico y devolviendo su nuevo índice.
    pub fn move_point(&mut self, index: usize, beat: f64, value: f32) -> Option<usize> {
        if index >= self.points.len() {
            return None;
        }
        let point = AutomationPoint::new(beat, value);
        self.points.remove(index);
        let idx = self.points.partition_point(|p| p.beat < point.beat);
        self.points.insert(idx, point);
        Some(idx)
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.points.len() {
            self.points.remove(index);
            true
        } else {
            false
        }
    }

    /// Desplaza puntos a partir de `from_beat` (útil al insertar/duplicar/eliminar tiempo).
    pub fn shift_from(&mut self, from_beat: f64, delta_beats: f64) {
        for point in &mut self.points {
            if point.beat >= from_beat {
                point.beat = (point.beat + delta_beats).max(0.0);
            }
        }
        self.points.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackAutomation {
    pub volume: AutomationLane,
    pub pan: AutomationLane,
}

impl TrackAutomation {
    pub fn lane(&self, param: AutomationParam) -> &AutomationLane {
        match param {
            AutomationParam::Volume => &self.volume,
            AutomationParam::Pan => &self.pan,
        }
    }

    pub fn lane_mut(&mut self, param: AutomationParam) -> &mut AutomationLane {
        match param {
            AutomationParam::Volume => &mut self.volume,
            AutomationParam::Pan => &mut self.pan,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.volume.is_empty() && self.pan.is_empty()
    }

    pub fn evaluate_gain(&self, beat: f64, fallback_gain: f32, audible: bool) -> f32 {
        if !audible {
            return 0.0;
        }
        self.volume
            .evaluate(beat)
            .map_or(fallback_gain, AutomationParam::volume_to_gain)
    }

    pub fn evaluate_pan(&self, beat: f64, fallback_pan: f32) -> f32 {
        self.pan
            .evaluate(beat)
            .map_or(fallback_pan, AutomationParam::pan_to_position)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArrangementAutomation {
    pub tracks: BTreeMap<TrackId, TrackAutomation>,
}

impl ArrangementAutomation {
    pub fn track(&self, id: TrackId) -> Option<&TrackAutomation> {
        self.tracks.get(&id)
    }

    pub fn track_or_default(&self, id: TrackId) -> TrackAutomation {
        self.tracks.get(&id).cloned().unwrap_or_default()
    }

    pub fn lane(&self, id: TrackId, param: AutomationParam) -> Option<&AutomationLane> {
        self.tracks.get(&id).map(|t| t.lane(param))
    }

    pub fn lane_mut(&mut self, id: TrackId, param: AutomationParam) -> &mut AutomationLane {
        self.tracks.entry(id).or_default().lane_mut(param)
    }

    pub fn has_points(&self, id: TrackId, param: AutomationParam) -> bool {
        self.lane(id, param).is_some_and(|l| !l.is_empty())
    }

    pub fn has_any_points(&self, id: TrackId) -> bool {
        self.tracks.get(&id).is_some_and(|t| !t.is_empty())
    }

    pub fn clear_lane(&mut self, id: TrackId, param: AutomationParam) {
        if let Some(track) = self.tracks.get_mut(&id) {
            track.lane_mut(param).clear();
            if track.is_empty() {
                self.tracks.remove(&id);
            }
        }
    }

    pub fn cleanup_empty(&mut self) {
        self.tracks.retain(|_, track| !track.is_empty());
    }

    pub fn retain_tracks(&mut self, valid: &[TrackId]) {
        self.tracks
            .retain(|id, track| valid.contains(id) && !track.is_empty());
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.values().all(TrackAutomation::is_empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation_and_boundary_clamping_are_exact() {
        let mut lane = AutomationLane::default();
        assert_eq!(lane.evaluate(0.0), None);
        lane.upsert(2.0, 0.25);
        lane.upsert(6.0, 0.75);

        assert!((lane.evaluate(0.0).unwrap() - 0.25).abs() < 1e-6);
        assert!((lane.evaluate(2.0).unwrap() - 0.25).abs() < 1e-6);
        assert!((lane.evaluate(4.0).unwrap() - 0.50).abs() < 1e-6);
        assert!((lane.evaluate(6.0).unwrap() - 0.75).abs() < 1e-6);
        assert!((lane.evaluate(12.0).unwrap() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn move_and_remove_keep_points_sorted_and_mute_overrides_volume() {
        let mut auto = TrackAutomation::default();
        let i0 = auto.volume.upsert(4.0, 0.75);
        let _i1 = auto.volume.upsert(1.0, 0.0);
        // Al insertar en beat 1.0, el punto en 4.0 ahora está en índice 1.
        assert_eq!(i0, 0);
        assert_eq!(auto.volume.points()[0].beat, 1.0);
        assert_eq!(auto.volume.points()[1].beat, 4.0);

        let moved = auto.volume.move_point(1, 0.5, 1.0).unwrap();
        assert_eq!(moved, 0);
        assert_eq!(auto.volume.points()[0].beat, 0.5);

        // Con audible = true evalúa la curva; con audible = false (mute) devuelve 0.0.
        assert!(auto.evaluate_gain(0.5, 0.5, true) > 1.3);
        assert_eq!(auto.evaluate_gain(0.5, 0.5, false), 0.0);

        assert!(auto.volume.remove(0));
        assert_eq!(auto.volume.len(), 1);
    }
}
