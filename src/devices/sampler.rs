//! Contrato headless del sampler. Unidades musicales; sin UI ni motor nativo.
use crate::{ParameterId, ParameterSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Classic,
    OneShot,
}

#[derive(Clone, Debug)]
pub struct SamplerParams {
    values: [f32; 14],
    pub mode: PlayMode,
    pub looped: bool,
    pub filter: bool,
}

pub const START: usize = 0;
pub const END: usize = 1;
pub const LOOP_START: usize = 2;
pub const TUNE: usize = 3;
pub const ROOT: usize = 4;
pub const VOICES: usize = 5;
pub const CUTOFF: usize = 6;
pub const RESONANCE: usize = 7;
pub const ATTACK: usize = 8;
pub const DECAY: usize = 9;
pub const SUSTAIN: usize = 10;
pub const RELEASE: usize = 11;
pub const GAIN: usize = 12;
pub const PAN: usize = 13;

const fn spec(
    id: usize,
    key: &'static str,
    name: &'static str,
    unit: &'static str,
    min: f32,
    max: f32,
    default: f32,
) -> ParameterSpec {
    ParameterSpec {
        id: ParameterId(id as u32),
        key,
        name,
        unit,
        min,
        max,
        default,
    }
}

pub const PARAMS: [ParameterSpec; 14] = [
    spec(START, "start", "Inicio", "%", 0., 99.9, 0.),
    spec(END, "end", "Final", "%", 0.1, 100., 100.),
    spec(LOOP_START, "loop_start", "Loop", "%", 0., 99.9, 0.),
    spec(TUNE, "tune", "Afinación", "st", -24., 24., 0.),
    spec(ROOT, "root", "Nota raíz", "MIDI", 0., 127., 60.),
    spec(VOICES, "voices", "Voces", "", 1., 128., 32.),
    spec(CUTOFF, "cutoff", "Frecuencia", "Hz", 20., 20000., 20000.),
    spec(RESONANCE, "resonance", "Resonancia", "dB", 0., 24., 0.),
    spec(ATTACK, "attack", "Attack", "ms", 0., 10000., 5.),
    spec(DECAY, "decay", "Decay", "ms", 0., 10000., 600.),
    spec(SUSTAIN, "sustain", "Sustain", "%", 0., 100., 100.),
    spec(RELEASE, "release", "Release", "ms", 1., 10000., 50.),
    spec(GAIN, "gain", "Volumen", "dB", -60., 6., -12.),
    spec(PAN, "pan", "Pan", "%", -100., 100., 0.),
];

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            values: PARAMS.map(|s| s.default),
            mode: PlayMode::Classic,
            looped: false,
            filter: false,
        }
    }
}
impl SamplerParams {
    pub fn get(&self, id: usize) -> f32 {
        self.values[id]
    }
    pub fn set(&mut self, id: usize, value: f32) {
        if !value.is_finite() || id >= PARAMS.len() {
            return;
        }
        let s = PARAMS[id];
        let value = value.clamp(s.min, s.max);
        self.values[id] = if matches!(id, ROOT | VOICES) {
            value.round()
        } else {
            value
        };
        match id {
            START => {
                self.values[START] = self.values[START].min(self.values[END] - 0.1);
                self.values[LOOP_START] = self.values[LOOP_START].max(self.values[START]);
            }
            END => {
                self.values[END] = self.values[END].max(self.values[START] + 0.1);
                self.values[LOOP_START] = self.values[LOOP_START].min(self.values[END] - 0.1);
            }
            LOOP_START => self.values[id] = value.clamp(self.values[START], self.values[END] - 0.1),
            _ => {}
        }
    }
    pub fn fraction(&self, id: usize) -> f32 {
        let s = PARAMS[id];
        if id == CUTOFF {
            (self.get(id) / s.min).ln() / (s.max / s.min).ln()
        } else {
            (self.get(id) - s.min) / (s.max - s.min)
        }
    }
    pub fn value_at(id: usize, fraction: f32) -> f32 {
        let s = PARAMS[id];
        let f = fraction.clamp(0., 1.);
        if id == CUTOFF {
            s.min * (s.max / s.min).powf(f)
        } else {
            s.min + f * (s.max - s.min)
        }
    }
}

/// CC de alta resolución privados del instrumento SFZ. Los cambios no reconstruyen voces.
pub fn live_control(params: &SamplerParams, id: usize) -> Option<(u8, f32)> {
    if matches!(
        id,
        TUNE | CUTOFF | RESONANCE | ATTACK | DECAY | SUSTAIN | RELEASE | GAIN | PAN
    ) {
        Some((80 + id as u8, params.fraction(id)))
    } else {
        None
    }
}
