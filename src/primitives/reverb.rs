//! Reverb algorítmica estéreo estilo Ableton Live Reverb: Pre-Delay, difusión allpass,
//! red FDN Hadamard de 8 canales con modulación sutil, filtros Low/High Cut, Damping
//! de agudos y modo Freeze infinito. Cero reservas ni bloqueos en RT.
use crate::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};

pub const FREEZE: ParameterId = ParameterId(0);
pub const PREDELAY_MS: ParameterId = ParameterId(1);
pub const DECAY_S: ParameterId = ParameterId(2);
pub const SIZE_PCT: ParameterId = ParameterId(3);
pub const DIFFUSION_PCT: ParameterId = ParameterId(4);
pub const LOW_CUT_HZ: ParameterId = ParameterId(5);
pub const HIGH_CUT_HZ: ParameterId = ParameterId(6);
pub const DAMP_PCT: ParameterId = ParameterId(7);
pub const MIX_PCT: ParameterId = ParameterId(8);
pub const SYNC: ParameterId = ParameterId(9);
pub const PRE_DIV: ParameterId = ParameterId(10);
pub const DECAY_DIV: ParameterId = ParameterId(11);

/// Divisiones de Pre-Delay sincronizadas al BPM (`Off` .. `1/8`).
pub const REVERB_PRE_DIVISIONS: [(&str, f64); 5] = [
    ("Off", 0.0),
    ("1/64", 0.0625),
    ("1/32", 0.125),
    ("1/16", 0.25),
    ("1/8", 0.5),
];

/// Divisiones de Decay sincronizadas al BPM (`1/4` .. `8c`).
pub const REVERB_DECAY_DIVISIONS: [(&str, f64); 6] = [
    ("1/4", 1.0),
    ("1/2", 2.0),
    ("1c", 4.0),
    ("2c", 8.0),
    ("4c", 16.0),
    ("8c", 32.0),
];

pub fn predelay_sync_ms(index: u8, bpm: f64) -> f32 {
    let beats = REVERB_PRE_DIVISIONS
        .get(usize::from(index))
        .map_or(0.125, |&(_, b)| b);
    let safe_bpm = bpm.clamp(20.0, 360.0);
    ((beats * 60_000.0 / safe_bpm) as f32).clamp(0.0, 250.0)
}

pub fn decay_sync_s(index: u8, bpm: f64) -> f32 {
    let beats = REVERB_DECAY_DIVISIONS
        .get(usize::from(index))
        .map_or(4.0, |&(_, b)| b);
    let safe_bpm = bpm.clamp(20.0, 360.0);
    ((beats * 60.0 / safe_bpm) as f32).clamp(0.2, 20.0)
}

pub fn predelay_div_label(index: u8) -> &'static str {
    REVERB_PRE_DIVISIONS
        .get(usize::from(index))
        .map_or("1/32", |&(l, _)| l)
}

pub fn decay_div_label(index: u8) -> &'static str {
    REVERB_DECAY_DIVISIONS
        .get(usize::from(index))
        .map_or("1c", |&(l, _)| l)
}

const MAX_PREDELAY_SAMPLES: usize = 48_000; // 250 ms @ 192 kHz
const FDN_CHANNELS: usize = 8;
/// Longitudes primas base (para 48 kHz) escaladas por `Size` y `sample_rate`.
const BASE_FDN_LENGTHS_48K: [usize; FDN_CHANNELS] =
    [1499, 1789, 2111, 2437, 2791, 3163, 3557, 3967];
const MAX_FDN_LEN: usize = 20_000;
const DIFF_LEN_1: usize = 1024;
const DIFF_LEN_2: usize = 1536;

const PARAMETERS: &[ParameterSpec] = &[
    ParameterSpec {
        id: FREEZE,
        key: "freeze",
        name: "Freeze",
        unit: "tipo",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParameterSpec {
        id: PREDELAY_MS,
        key: "predelay",
        name: "Pre-Delay",
        unit: "ms",
        min: 0.0,
        max: 250.0,
        default: 18.0,
    },
    ParameterSpec {
        id: DECAY_S,
        key: "decay",
        name: "Decay",
        unit: "s",
        min: 0.2,
        max: 20.0,
        default: 2.4,
    },
    ParameterSpec {
        id: SIZE_PCT,
        key: "size",
        name: "Tamaño",
        unit: "%",
        min: 10.0,
        max: 100.0,
        default: 65.0,
    },
    ParameterSpec {
        id: DIFFUSION_PCT,
        key: "diffusion",
        name: "Difusión",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 75.0,
    },
    ParameterSpec {
        id: LOW_CUT_HZ,
        key: "low_cut",
        name: "Low Cut",
        unit: "Hz",
        min: 20.0,
        max: 1000.0,
        default: 120.0,
    },
    ParameterSpec {
        id: HIGH_CUT_HZ,
        key: "high_cut",
        name: "High Cut",
        unit: "Hz",
        min: 1000.0,
        max: 20000.0,
        default: 10000.0,
    },
    ParameterSpec {
        id: DAMP_PCT,
        key: "damp",
        name: "Damping",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 40.0,
    },
    ParameterSpec {
        id: MIX_PCT,
        key: "mix",
        name: "Dry/Wet",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 32.0,
    },
    ParameterSpec {
        id: SYNC,
        key: "sync",
        name: "Tempo",
        unit: "tipo",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParameterSpec {
        id: PRE_DIV,
        key: "pre_div",
        name: "Pre-Delay Sync",
        unit: "tipo",
        min: 0.0,
        max: 4.0,
        default: 2.0,
    },
    ParameterSpec {
        id: DECAY_DIV,
        key: "decay_div",
        name: "Decay Sync",
        unit: "tipo",
        min: 0.0,
        max: 5.0,
        default: 2.0,
    },
];

pub struct Reverb {
    sample_rate: f32,
    tempo_bpm: f64,
    bypassed: bool,
    freeze: bool,
    predelay_ms: f32,
    decay_s: f32,
    size_pct: f32,
    diffusion_pct: f32,
    low_cut_hz: f32,
    high_cut_hz: f32,
    damp_pct: f32,
    mix_pct: f32,
    sync: bool,
    pre_div: u8,
    decay_div: u8,

    // Pre-delay estéreo
    pre_l: Box<[f32]>,
    pre_r: Box<[f32]>,
    pre_pos: usize,

    // Filtros de entrada (Low Cut + High Cut)
    in_hp_prev_in: [f32; 2],
    in_hp_prev_out: [f32; 2],
    in_lp_state: [f32; 2],

    // 2 difusores allpass por canal
    diff1_l: Box<[f32]>,
    diff1_r: Box<[f32]>,
    diff2_l: Box<[f32]>,
    diff2_r: Box<[f32]>,
    diff1_pos: usize,
    diff2_pos: usize,

    // Red FDN de 8 líneas con damping individual
    fdn_bufs: Vec<Box<[f32]>>,
    fdn_pos: [usize; FDN_CHANNELS],
    fdn_damp: [f32; FDN_CHANNELS],
    lfo_phase: f32,
}

impl Reverb {
    pub const INFO: &'static PrimitiveInfo = &PrimitiveInfo {
        id: "elevate.space.reverb",
        name: "Hay Reverb",
        properties: &[SignalProperty::Spatial, SignalProperty::Time],
        parameters: PARAMETERS,
    };

    pub fn new(specification: ProcessSpec) -> Self {
        let sr = specification.sample_rate().max(8_000.0) as f32;
        let fdn_bufs = (0..FDN_CHANNELS)
            .map(|_| vec![0.0; MAX_FDN_LEN].into_boxed_slice())
            .collect();
        Self {
            sample_rate: sr,
            tempo_bpm: 120.0,
            bypassed: false,
            freeze: false,
            predelay_ms: 18.0,
            decay_s: 2.4,
            size_pct: 65.0,
            diffusion_pct: 75.0,
            low_cut_hz: 120.0,
            high_cut_hz: 10000.0,
            damp_pct: 40.0,
            mix_pct: 32.0,
            sync: false,
            pre_div: 2,
            decay_div: 2,
            pre_l: vec![0.0; MAX_PREDELAY_SAMPLES].into_boxed_slice(),
            pre_r: vec![0.0; MAX_PREDELAY_SAMPLES].into_boxed_slice(),
            pre_pos: 0,
            in_hp_prev_in: [0.0; 2],
            in_hp_prev_out: [0.0; 2],
            in_lp_state: [0.0; 2],
            diff1_l: vec![0.0; DIFF_LEN_1].into_boxed_slice(),
            diff1_r: vec![0.0; DIFF_LEN_1].into_boxed_slice(),
            diff2_l: vec![0.0; DIFF_LEN_2].into_boxed_slice(),
            diff2_r: vec![0.0; DIFF_LEN_2].into_boxed_slice(),
            diff1_pos: 0,
            diff2_pos: 0,
            fdn_bufs,
            fdn_pos: [0; FDN_CHANNELS],
            fdn_damp: [0.0; FDN_CHANNELS],
            lfo_phase: 0.0,
        }
    }

    /// Matriz ortogonal Hadamard 8x8 normalizada (factor 1/sqrt(8) ≈ 0.35355338).
    #[inline]
    fn hadamard8(x: &mut [f32; 8]) {
        for step in [1usize, 2, 4] {
            let mut i = 0;
            while i < 8 {
                for j in 0..step {
                    let a = x[i + j];
                    let b = x[i + j + step];
                    x[i + j] = a + b;
                    x[i + j + step] = a - b;
                }
                i += step * 2;
            }
        }
        const INV_SQRT8: f32 = 0.353_553_38;
        for v in x.iter_mut() {
            *v *= INV_SQRT8;
        }
    }

    #[inline]
    fn allpass(buf: &mut [f32], pos: usize, delay: usize, g: f32, input: f32) -> f32 {
        let len = buf.len();
        let read_idx = (pos + len - delay.min(len - 1)) % len;
        let delayed = buf[read_idx];
        let v = input - g * delayed;
        buf[pos] = v;
        delayed + g * v
    }
}

impl Primitive for Reverb {
    fn info(&self) -> &'static PrimitiveInfo {
        Self::INFO
    }

    fn parameter(&self, parameter: ParameterId) -> Result<f32, PrimitiveError> {
        Ok(match parameter {
            FREEZE => {
                if self.freeze {
                    1.0
                } else {
                    0.0
                }
            }
            PREDELAY_MS => self.predelay_ms,
            DECAY_S => self.decay_s,
            SIZE_PCT => self.size_pct,
            DIFFUSION_PCT => self.diffusion_pct,
            LOW_CUT_HZ => self.low_cut_hz,
            HIGH_CUT_HZ => self.high_cut_hz,
            DAMP_PCT => self.damp_pct,
            MIX_PCT => self.mix_pct,
            SYNC => {
                if self.sync {
                    1.0
                } else {
                    0.0
                }
            }
            PRE_DIV => f32::from(self.pre_div),
            DECAY_DIV => f32::from(self.decay_div),
            _ => return Err(PrimitiveError::UnknownParameter(parameter)),
        })
    }

    fn set_parameter(&mut self, parameter: ParameterId, value: f32) -> Result<(), PrimitiveError> {
        let spec = PARAMETERS
            .iter()
            .find(|p| p.id == parameter)
            .ok_or(PrimitiveError::UnknownParameter(parameter))?;
        spec.validate(value)?;
        match parameter {
            FREEZE => self.freeze = value >= 0.5,
            PREDELAY_MS => self.predelay_ms = value,
            DECAY_S => self.decay_s = value,
            SIZE_PCT => self.size_pct = value,
            DIFFUSION_PCT => self.diffusion_pct = value,
            LOW_CUT_HZ => self.low_cut_hz = value,
            HIGH_CUT_HZ => self.high_cut_hz = value,
            DAMP_PCT => self.damp_pct = value,
            MIX_PCT => self.mix_pct = value,
            SYNC => self.sync = value >= 0.5,
            PRE_DIV => self.pre_div = value.round().clamp(0.0, 4.0) as u8,
            DECAY_DIV => self.decay_div = value.round().clamp(0.0, 5.0) as u8,
            _ => unreachable!(),
        }
        Ok(())
    }

    fn prepare(&mut self, specification: ProcessSpec) {
        self.sample_rate = specification.sample_rate().max(8_000.0) as f32;
        self.reset();
    }

    fn set_tempo_bpm(&mut self, bpm: f64) {
        if bpm.is_finite() && bpm >= 20.0 {
            self.tempo_bpm = bpm.clamp(20.0, 360.0);
        }
    }

    fn process(&mut self, frames: &mut [StereoFrame]) {
        if self.bypassed || frames.is_empty() {
            return;
        }
        let sr = self.sample_rate.max(8_000.0);
        let rate_scale = sr / 48_000.0;
        let size_scale = 0.35 + (self.size_pct / 100.0).clamp(0.1, 1.0) * 0.95;

        let eff_predelay_ms = if self.sync {
            predelay_sync_ms(self.pre_div, self.tempo_bpm)
        } else {
            self.predelay_ms
        };
        let eff_decay_s = if self.sync {
            decay_sync_s(self.decay_div, self.tempo_bpm)
        } else {
            self.decay_s
        };

        let predelay_samples =
            ((eff_predelay_ms * 0.001 * sr).round() as usize).min(MAX_PREDELAY_SAMPLES - 2);
        let diff_g = (self.diffusion_pct / 100.0).clamp(0.0, 1.0) * 0.68;
        let diff_d1 = ((211.0 * rate_scale).round() as usize).clamp(16, DIFF_LEN_1 - 2);
        let diff_d2 = ((379.0 * rate_scale).round() as usize).clamp(16, DIFF_LEN_2 - 2);

        let dt = 1.0 / sr;
        let hp_rc = 1.0 / (std::f32::consts::TAU * self.low_cut_hz.clamp(20.0, sr * 0.45));
        let hp_alpha = hp_rc / (hp_rc + dt);
        let lp_rc = 1.0 / (std::f32::consts::TAU * self.high_cut_hz.clamp(200.0, sr * 0.45));
        let lp_alpha = dt / (lp_rc + dt);

        let damp_coeff = if self.freeze {
            0.0
        } else {
            (self.damp_pct / 100.0).clamp(0.0, 0.92) * 0.72
        };

        // Longitudes y ganancias RT60 exactas por cada canal de la FDN
        let mut fdn_delays = [0usize; FDN_CHANNELS];
        let mut fdn_gains = [0.0f32; FDN_CHANNELS];
        let decay = eff_decay_s.max(0.2);
        for ch in 0..FDN_CHANNELS {
            let d = ((BASE_FDN_LENGTHS_48K[ch] as f32 * rate_scale * size_scale).round() as usize)
                .clamp(64, MAX_FDN_LEN - 16);
            fdn_delays[ch] = d;
            fdn_gains[ch] = if self.freeze {
                1.0
            } else {
                // Ganancia por vuelta para alcanzar -60 dB en `decay` segundos.
                10.0_f32
                    .powf((-3.0 * d as f32) / (decay * sr))
                    .clamp(0.0, 0.996)
            };
        }

        let wet = (self.mix_pct / 100.0).clamp(0.0, 1.0);
        let dry = 1.0 - wet;
        let lfo_inc = std::f32::consts::TAU * 0.42 / sr;
        let freeze = self.freeze;

        for frame in frames.iter_mut() {
            let in_l = frame[0];
            let in_r = frame[1];

            // 1. Pre-delay
            self.pre_l[self.pre_pos] = in_l;
            self.pre_r[self.pre_pos] = in_r;
            let pre_read =
                (self.pre_pos + MAX_PREDELAY_SAMPLES - predelay_samples) % MAX_PREDELAY_SAMPLES;
            let mut sig = [self.pre_l[pre_read], self.pre_r[pre_read]];
            self.pre_pos = (self.pre_pos + 1) % MAX_PREDELAY_SAMPLES;

            // 2. Filtros de entrada (Low Cut + High Cut)
            for ch in 0..2 {
                let hp = hp_alpha * (self.in_hp_prev_out[ch] + sig[ch] - self.in_hp_prev_in[ch]);
                self.in_hp_prev_in[ch] = sig[ch];
                self.in_hp_prev_out[ch] = hp;
                let lp = self.in_lp_state[ch] + lp_alpha * (hp - self.in_lp_state[ch]);
                self.in_lp_state[ch] = lp;
                sig[ch] = if freeze { 0.0 } else { lp };
            }

            // 3. Difusión allpass en 2 etapas
            let d_l = Self::allpass(&mut self.diff1_l, self.diff1_pos, diff_d1, diff_g, sig[0]);
            let d_r = Self::allpass(
                &mut self.diff1_r,
                self.diff1_pos,
                diff_d1 + 19,
                diff_g,
                sig[1],
            );
            let d_l = Self::allpass(&mut self.diff2_l, self.diff2_pos, diff_d2, diff_g, d_l);
            let d_r = Self::allpass(&mut self.diff2_r, self.diff2_pos, diff_d2 + 23, diff_g, d_r);
            self.diff1_pos = (self.diff1_pos + 1) % DIFF_LEN_1;
            self.diff2_pos = (self.diff2_pos + 1) % DIFF_LEN_2;

            // 4. Lectura FDN con modulación sutil (evita timbres metálicos)
            self.lfo_phase += lfo_inc;
            if self.lfo_phase >= std::f32::consts::TAU {
                self.lfo_phase -= std::f32::consts::TAU;
            }
            let mod_a = self.lfo_phase.sin() * 3.5;
            let mod_b = (self.lfo_phase + 2.094).sin() * 3.5;

            let mut taps = [0.0_f32; FDN_CHANNELS];
            for ch in 0..FDN_CHANNELS {
                let jitter = if ch % 2 == 0 { mod_a } else { mod_b };
                let d_f = (fdn_delays[ch] as f32 + if freeze { 0.0 } else { jitter })
                    .clamp(8.0, (MAX_FDN_LEN - 4) as f32);
                let int_d = d_f.floor() as usize;
                let frac = d_f - int_d as f32;
                let pos = self.fdn_pos[ch];
                let idx0 = (pos + MAX_FDN_LEN - int_d) % MAX_FDN_LEN;
                let idx1 = (pos + MAX_FDN_LEN - int_d - 1) % MAX_FDN_LEN;
                let raw = self.fdn_bufs[ch][idx0]
                    + (self.fdn_bufs[ch][idx1] - self.fdn_bufs[ch][idx0]) * frac;
                // Damping HF por línea
                let damped = raw * (1.0 - damp_coeff) + self.fdn_damp[ch] * damp_coeff;
                self.fdn_damp[ch] = damped;
                taps[ch] = damped * fdn_gains[ch];
            }

            // 5. Mezcla ortogonal Hadamard + inyección estéreo balanceada
            let mut mixed = taps;
            Self::hadamard8(&mut mixed);

            let inject_scale = 0.35;
            for ch in 0..FDN_CHANNELS {
                let inject = if ch % 2 == 0 { d_l } else { d_r } * inject_scale;
                let val = mixed[ch] + inject;
                let pos = self.fdn_pos[ch];
                self.fdn_bufs[ch][pos] = if val.is_finite() {
                    val.clamp(-4.0, 4.0)
                } else {
                    0.0
                };
                self.fdn_pos[ch] = (pos + 1) % MAX_FDN_LEN;
            }

            // 6. Salida estéreo descorrelacionada (ganancia musical equilibrada para Dry/Wet)
            let out_l = 0.65 * (taps[0] + taps[2] - taps[4] + taps[6]);
            let out_r = 0.65 * (taps[1] - taps[3] + taps[5] + taps[7]);

            frame[0] = in_l * dry + out_l * wet;
            frame[1] = in_r * dry + out_r * wet;
        }
    }

    fn reset(&mut self) {
        self.pre_l.fill(0.0);
        self.pre_r.fill(0.0);
        self.pre_pos = 0;
        self.in_hp_prev_in = [0.0; 2];
        self.in_hp_prev_out = [0.0; 2];
        self.in_lp_state = [0.0; 2];
        self.diff1_l.fill(0.0);
        self.diff1_r.fill(0.0);
        self.diff2_l.fill(0.0);
        self.diff2_r.fill(0.0);
        self.diff1_pos = 0;
        self.diff2_pos = 0;
        for buf in &mut self.fdn_bufs {
            buf.fill(0.0);
        }
        self.fdn_pos = [0; FDN_CHANNELS];
        self.fdn_damp = [0.0; FDN_CHANNELS];
        self.lfo_phase = 0.0;
    }

    fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_generates_stereo_tail_and_freeze_sustains_energy() {
        let spec = ProcessSpec::new(48_000.0).unwrap();
        let mut rev = Reverb::new(spec);
        rev.set_parameter(PREDELAY_MS, 5.0).unwrap();
        rev.set_parameter(DECAY_S, 2.5).unwrap();
        rev.set_parameter(MIX_PCT, 100.0).unwrap();
        rev.reset();

        // Impulso corto
        let mut block = vec![[0.0; 2]; 16_000];
        for f in block.iter_mut().take(64) {
            *f = [0.8, 0.8];
        }
        rev.process(&mut block);

        let tail_rms_l =
            (block[8000..12000].iter().map(|f| f[0] * f[0]).sum::<f32>() / 4000.0).sqrt();
        let tail_rms_r =
            (block[8000..12000].iter().map(|f| f[1] * f[1]).sum::<f32>() / 4000.0).sqrt();
        assert!(
            tail_rms_l > 0.02,
            "cola L esperada > 0.02, obt {tail_rms_l}"
        );
        assert!(
            tail_rms_r > 0.02,
            "cola R esperada > 0.02, obt {tail_rms_r}"
        );

        // Activar Freeze: tras 2 segundos de silencio en entrada, la energía se sostiene.
        rev.set_parameter(FREEZE, 1.0).unwrap();
        let mut frozen = vec![[0.0; 2]; 48_000];
        rev.process(&mut frozen);
        let frozen_rms = (frozen[44_000..48_000]
            .iter()
            .map(|f| f[0] * f[0])
            .sum::<f32>()
            / 4000.0)
            .sqrt();
        assert!(
            frozen_rms > 0.015,
            "Freeze debía sostener la cola sin decaer, obt {frozen_rms}"
        );
    }

    #[test]
    fn reverb_sync_scales_predelay_and_decay_with_daw_bpm() {
        // A 120 BPM: Pre 1/16 = 125 ms, Decay 1c (4 beats) = 2.0 s.
        assert!((predelay_sync_ms(3, 120.0) - 125.0).abs() < 0.1);
        assert!((decay_sync_s(2, 120.0) - 2.0).abs() < 0.01);
        // Al doblar a 240 BPM, ambos tiempos se dividen exactamente a la mitad.
        assert!((predelay_sync_ms(3, 240.0) - 62.5).abs() < 0.1);
        assert!((decay_sync_s(2, 240.0) - 1.0).abs() < 0.01);
    }
}
