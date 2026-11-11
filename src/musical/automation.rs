//! Eventos de control MIDI discretos en tiempo musical, independientes de las notas.
use super::{model::Pattern, time::Tick};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlLane {
    Cc(u8),
    PitchBend,
    Pressure,
}

impl ControlLane {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "pitch" => Some(Self::PitchBend),
            "pressure" => Some(Self::Pressure),
            word => word
                .strip_prefix("cc")
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|v| *v < 128)
                .map(Self::Cc),
        }
    }
    pub fn key(self) -> String {
        match self {
            Self::Cc(cc) => format!("cc{cc}"),
            Self::PitchBend => "pitch".into(),
            Self::Pressure => "pressure".into(),
        }
    }
    pub fn valid(self, value: i16) -> bool {
        match self {
            Self::Cc(cc) => cc < 128 && (0..=127).contains(&value),
            Self::PitchBend => (-8192..=8191).contains(&value),
            Self::Pressure => (0..=127).contains(&value),
        }
    }
    pub fn midi(self, value: i16) -> (u8, u8, u8) {
        match self {
            Self::Cc(cc) => (0xb0, cc, value as u8),
            Self::Pressure => (0xd0, value as u8, 0),
            Self::PitchBend => {
                let raw = (value + 8192) as u16;
                (0xe0, (raw & 127) as u8, (raw >> 7) as u8)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPoint {
    pub id: u64,
    pub at: Tick,
    pub lane: ControlLane,
    pub value: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Automation {
    points: Vec<ControlPoint>,
    next_id: u64,
}
impl Default for Automation {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            next_id: 1,
        }
    }
}

impl Automation {
    pub fn points(&self) -> &[ControlPoint] {
        &self.points
    }
    pub fn next_id(&self) -> u64 {
        self.next_id
    }
    pub fn restore(
        mut points: Vec<ControlPoint>,
        next_id: u64,
        length: Tick,
    ) -> Result<Self, String> {
        let mut ids = HashSet::new();
        let mut locations = HashSet::new();
        if next_id == 0 || next_id > i64::MAX as u64 || points.len() > 65_536 {
            return Err("Demasiados controles o IDs agotados".into());
        }
        for point in &points {
            if point.id == 0
                || point.id >= next_id
                || !ids.insert(point.id)
                || !locations.insert((point.at.0, point.lane))
                || point.at.0 < 0
                || point.at >= length
                || !point.lane.valid(point.value)
            {
                return Err("Evento de control MIDI inválido".into());
            }
        }
        points.sort_by_key(|point| (point.at, point.lane));
        Ok(Self { points, next_id })
    }
    pub fn set(
        &mut self,
        lane: ControlLane,
        at: Tick,
        value: i16,
        length: Tick,
    ) -> Result<u64, String> {
        if at.0 < 0 || at >= length || !lane.valid(value) {
            return Err("Control MIDI fuera de rango".into());
        }
        let index = match self
            .points
            .binary_search_by_key(&(at, lane), |p| (p.at, p.lane))
        {
            Ok(index) => {
                self.points[index].value = value;
                return Ok(self.points[index].id);
            }
            Err(index) => index,
        };
        if self.points.len() >= 65_536 || self.next_id >= i64::MAX as u64 {
            return Err("Límite de controles MIDI alcanzado".into());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.points.insert(
            index,
            ControlPoint {
                id,
                at,
                lane,
                value,
            },
        );
        Ok(id)
    }
    pub fn remove(&mut self, id: u64) -> bool {
        let len = self.points.len();
        self.points.retain(|point| point.id != id);
        self.points.len() != len
    }
    /// Copia una ventana y persigue el último valor anterior de cada controlador.
    pub fn window(&self, start: Tick, length: Tick) -> Self {
        let mut result = Self::default();
        for point in &self.points {
            let at = Tick((point.at.0 - start.0).max(0));
            if at < length {
                let _ = result.set(point.lane, at, point.value, length);
            }
        }
        result
    }
}

impl Pattern {
    pub fn controls(&self) -> &Automation {
        &self.automation
    }
    pub fn set_control(&mut self, lane: ControlLane, at: Tick, value: i16) -> Result<u64, String> {
        self.automation.set(lane, at, value, self.length())
    }
    pub fn remove_control(&mut self, id: u64) -> bool {
        self.automation.remove(id)
    }
    pub fn restore_controls(
        &mut self,
        points: Vec<ControlPoint>,
        next_id: u64,
    ) -> Result<(), String> {
        self.automation = Automation::restore(points, next_id, self.length())?;
        Ok(())
    }
    /// Reconstruye notas conservando los controles, usados también por drag y Lua.
    pub fn with_notes(
        &self,
        notes: Vec<super::model::NoteEvent>,
        next_id: u64,
    ) -> Result<Self, super::model::EditError> {
        let mut result = Self::restore_with_next_id(self.length(), notes, next_id)?;
        result.automation = self.automation.clone();
        Ok(result)
    }
}
