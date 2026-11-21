//! Contrato headless del sampler / Simpler para drums. Unidades musicales; sin UI ni motor nativo.
use crate::{ParameterId, ParameterSpec, devices::filter::FilterMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Classic,
    OneShot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SamplerParams {
    values: [f32; 19],
    pub mode: PlayMode,
    pub looped: bool,
    pub filter: bool,
    pub filter_mode: FilterMode,
    /// En `false` (modo Drum) cualquier nota MIDI dispara el golpe a su tono original.
    pub keytrack: bool,
    /// Invierte la reproducción del sample (`direction=reverse` en SFZ).
    pub reverse: bool,
    /// Grupo de corte mutuo dentro de la instancia (choke de hi-hat / golpe único).
    pub choke: bool,
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
pub const HOLD: usize = 14;
pub const PITCH_ENV: usize = 15;
pub const PITCH_DECAY: usize = 16;
pub const VEL_VOLUME: usize = 17;
pub const VEL_FILTER: usize = 18;

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

pub const PARAMS: [ParameterSpec; 19] = [
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
    spec(HOLD, "hold", "Hold", "ms", 0., 5000., 40.),
    spec(PITCH_ENV, "pitch_env", "Pitch Env", "st", -24., 24., 0.),
    spec(
        PITCH_DECAY,
        "pitch_decay",
        "Pitch Decay",
        "ms",
        1.,
        2000.,
        80.,
    ),
    spec(VEL_VOLUME, "vel_volume", "Vel → Vol", "%", 0., 100., 80.),
    spec(
        VEL_FILTER,
        "vel_filter",
        "Vel → Cut",
        "ct",
        -4800.,
        4800.,
        0.,
    ),
];

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            values: PARAMS.map(|s| s.default),
            mode: PlayMode::Classic,
            looped: false,
            filter: true,
            filter_mode: FilterMode::LowPass,
            keytrack: true,
            reverse: false,
            choke: false,
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

    pub fn play_full_sample(&mut self) {
        self.mode = PlayMode::OneShot;
        self.looped = false;
        self.set(SUSTAIN, 100.);
    }

    /// Configura el Simpler para programar baterías de un toque (1-Shot + AHD + Pitch fijo + Choke).
    pub fn apply_drum_preset(&mut self) {
        self.mode = PlayMode::OneShot;
        self.looped = false;
        self.keytrack = false;
        self.choke = true;
        self.filter = true;
        self.set(VOICES, 4.);
        self.set(ATTACK, 0.5);
        self.set(HOLD, 35.);
        self.set(DECAY, 320.);
        self.set(SUSTAIN, 0.);
        self.set(RELEASE, 40.);
        self.set(VEL_VOLUME, 85.);
    }
}

/// CC de alta resolución privados del instrumento SFZ. Los cambios no reconstruyen voces.
pub fn live_control(params: &SamplerParams, id: usize) -> Option<(u8, f32)> {
    if matches!(
        id,
        TUNE | CUTOFF
            | RESONANCE
            | ATTACK
            | DECAY
            | SUSTAIN
            | RELEASE
            | GAIN
            | PAN
            | HOLD
            | PITCH_ENV
            | PITCH_DECAY
    ) {
        Some((80 + id as u8, params.fraction(id)))
    } else {
        None
    }
}
