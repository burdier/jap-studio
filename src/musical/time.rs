//! Tiempo de tempo constante del prototipo. La resolución visual no altera PPQ.
use std::fmt;

pub const PPQ: i64 = 960;
pub const DEFAULT_BPM: f64 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    InvalidTempo,
    InvalidSampleRate,
    OutOfRange,
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidTempo => "El tempo debe estar entre 1 y 1000 BPM",
            Self::InvalidSampleRate => "La frecuencia de muestreo debe ser positiva",
            Self::OutOfRange => "La posición temporal está fuera del rango del prototipo",
        })
    }
}
impl std::error::Error for TimeError {}

#[derive(Debug, Clone, Copy)]
pub struct TimeBase {
    sample_rate: u32,
    bpm: f64,
}

impl TimeBase {
    pub fn new(sample_rate: u32, bpm: f64) -> Result<Self, TimeError> {
        if sample_rate == 0 {
            return Err(TimeError::InvalidSampleRate);
        }
        if !bpm.is_finite() || !(1.0..=1000.0).contains(&bpm) {
            return Err(TimeError::InvalidTempo);
        }
        Ok(Self { sample_rate, bpm })
    }
    pub fn sample_rate(self) -> u32 {
        self.sample_rate
    }
    pub fn bpm(self) -> f64 {
        self.bpm
    }

    /// Redondea la posición absoluta, nunca una duración por paso: evita deriva acumulada.
    pub fn frame_at(self, tick: Tick) -> Result<u64, TimeError> {
        const EXACT_INTEGER_LIMIT: f64 = 9_007_199_254_740_992.0;
        let frame = tick.0 as f64 * f64::from(self.sample_rate) * 60.0 / (self.bpm * PPQ as f64);
        if tick.0 < 0 || tick.0 as f64 >= EXACT_INTEGER_LIMIT || frame >= EXACT_INTEGER_LIMIT {
            return Err(TimeError::OutOfRange);
        }
        Ok(frame.round() as u64)
    }
}

/// Intervalo semiabierto [start, end). Sus eventos tienen offset < frame_count.
#[derive(Debug, Clone, Copy)]
pub struct FrameWindow {
    start: u64,
    end: u64,
}

impl FrameWindow {
    pub fn new(start: u64, frame_count: u32) -> Result<Self, TimeError> {
        Ok(Self {
            start,
            end: start
                .checked_add(u64::from(frame_count))
                .ok_or(TimeError::OutOfRange)?,
        })
    }
    pub fn start(self) -> u64 {
        self.start
    }
    pub fn end(self) -> u64 {
        self.end
    }
}

/// Reloj avanzado exclusivamente por frames procesados. No usa tiempo de pared.
/// Stop conserva la posición; seek es explícito. El host libera voces al discontinuar.
#[derive(Debug, Default)]
pub struct Transport {
    frame: u64,
    playing: bool,
}

impl Transport {
    pub fn play(&mut self) {
        self.playing = true;
    }
    pub fn stop(&mut self) {
        self.playing = false;
    }
    pub fn seek(&mut self, frame: u64) {
        self.frame = frame;
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn advance(&mut self, frame_count: u32) -> Result<Option<FrameWindow>, TimeError> {
        if !self.playing {
            return Ok(None);
        }
        let window = FrameWindow::new(self.frame, frame_count)?;
        self.frame = window.end();
        Ok(Some(window))
    }
}
