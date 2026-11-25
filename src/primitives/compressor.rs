//! Compresor estéreo estilo Ableton Live (Peak/RMS, Soft Knee, Dry/Wet y Sidechain con HPF).
//! Cero asignaciones en RT; determinista en Play y Export offline.
use super::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};

pub const THRESHOLD: ParameterId = ParameterId(0);
pub const RATIO: ParameterId = ParameterId(1);
pub const ATTACK: ParameterId = ParameterId(2);
pub const RELEASE: ParameterId = ParameterId(3);
pub const KNEE: ParameterId = ParameterId(4);
pub const MAKEUP: ParameterId = ParameterId(5);
pub const MIX: ParameterId = ParameterId(6);
pub const MODE: ParameterId = ParameterId(7);
pub const SC_GAIN: ParameterId = ParameterId(8);
pub const SC_HP: ParameterId = ParameterId(9);

const PARAMETERS: [ParameterSpec; 10] = [
    ParameterSpec {
        id: THRESHOLD,
        key: "threshold",
        name: "Threshold",
        unit: "dB",
        min: -60.0,
        max: 0.0,
        default: -18.0,
    },
    ParameterSpec {
        id: RATIO,
        key: "ratio",
        name: "Ratio",
        unit: ":1",
        min: 1.0,
        max: 20.0,
        default: 2.0,
    },
    ParameterSpec {
        id: ATTACK,
        key: "attack",
        name: "Attack",
        unit: "ms",
        min: 0.01,
        max: 1000.0,
        default: 2.0,
    },
    ParameterSpec {
        id: RELEASE,
        key: "release",
        name: "Release",
        unit: "ms",
        min: 1.0,
        max: 3000.0,
        default: 50.0,
    },
    ParameterSpec {
        id: KNEE,
        key: "knee",
        name: "Knee",
        unit: "dB",
        min: 0.0,
        max: 18.0,
        default: 6.0,
    },
    ParameterSpec {
        id: MAKEUP,
        key: "makeup",
        name: "Output",
        unit: "dB",
        min: -18.0,
        max: 24.0,
        default: 0.0,
    },
    ParameterSpec {
        id: MIX,
        key: "mix",
        name: "Dry/Wet",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 100.0,
    },
    ParameterSpec {
        id: MODE,
        key: "mode",
        name: "Modo",
        unit: "tipo",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParameterSpec {
        id: SC_GAIN,
        key: "sc_gain",
        name: "SC Gain",
        unit: "dB",
        min: -24.0,
        max: 24.0,
        default: 0.0,
    },
    ParameterSpec {
        id: SC_HP,
        key: "sc_hp",
        name: "SC EQ HP",
        unit: "Hz",
        min: 20.0,
        max: 2000.0,
        default: 20.0,
    },
];

static INFO: PrimitiveInfo = PrimitiveInfo {
    id: "elevate.dynamics.compressor",
    name: "Compressor",
    properties: &[SignalProperty::Amplitude],
    parameters: &PARAMETERS,
};

pub struct Compressor {
    sample_rate: f32,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    makeup_db: f32,
    mix_pct: f32,
    rms_mode: bool,
    sc_gain_db: f32,
    sc_hp_hz: f32,
    bypassed: bool,
    // Estado DSP (cero asignaciones en RT)
    env_db: f32,
    rms_sq: f32,
    hp_prev_in: [f32; 2],
    hp_prev_out: [f32; 2],
    last_gr_db: f32,
}

impl Compressor {
    pub const INFO: &'static PrimitiveInfo = &INFO;

    pub fn new(spec: ProcessSpec) -> Self {
        Self {
            sample_rate: spec.sample_rate().max(8_000.0) as f32,
            threshold_db: -18.0,
            ratio: 2.0,
            attack_ms: 2.0,
            release_ms: 50.0,
            knee_db: 6.0,
            makeup_db: 0.0,
            mix_pct: 100.0,
            rms_mode: false,
            sc_gain_db: 0.0,
            sc_hp_hz: 20.0,
            bypassed: false,
            env_db: -96.0,
            rms_sq: 0.0,
            hp_prev_in: [0.0; 2],
            hp_prev_out: [0.0; 2],
            last_gr_db: 0.0,
        }
    }

    /// Curva de transferencia estática Input dB → Output dB (soft knee cuadrática estilo Ableton).
    pub fn transfer_db(input_db: f32, threshold_db: f32, ratio: f32, knee_db: f32) -> f32 {
        let r = ratio.max(1.0);
        let w = knee_db.max(0.0);
        let diff = input_db - threshold_db;
        if w <= 0.01 {
            if diff <= 0.0 {
                input_db
            } else {
                threshold_db + diff / r
            }
        } else if 2.0 * diff < -w {
            input_db
        } else if 2.0 * diff.abs() <= w {
            let x = diff + w * 0.5;
            input_db + (1.0 / r - 1.0) * (x * x) / (2.0 * w)
        } else {
            threshold_db + diff / r
        }
    }

    fn run_block(&mut self, frames: &mut [StereoFrame], sidechain: Option<&[StereoFrame]>) {
        if self.bypassed || frames.is_empty() {
            self.last_gr_db = 0.0;
            return;
        }
        let sr = self.sample_rate.max(8_000.0);
        let attack_coeff = (-1.0 / (self.attack_ms.max(0.01) * 0.001 * sr)).exp();
        let release_coeff = (-1.0 / (self.release_ms.max(1.0) * 0.001 * sr)).exp();
        let rms_coeff = (-1.0 / (0.010 * sr)).exp();
        let sc_gain = 10.0_f32.powf(self.sc_gain_db / 20.0);
        let makeup_lin = 10.0_f32.powf(self.makeup_db / 20.0);
        let wet = (self.mix_pct / 100.0).clamp(0.0, 1.0);
        let dry = 1.0 - wet;

        let use_hp = self.sc_hp_hz > 22.0;
        let hp_alpha = if use_hp {
            let rc = 1.0 / (std::f32::consts::TAU * self.sc_hp_hz);
            let dt = 1.0 / sr;
            rc / (rc + dt)
        } else {
            0.0
        };

        let mut block_gr = 0.0_f32;
        for (i, frame) in frames.iter_mut().enumerate() {
            let src = sidechain
                .and_then(|sc| sc.get(i).copied())
                .unwrap_or(*frame);
            let mut det_l = src[0] * sc_gain;
            let mut det_r = src[1] * sc_gain;
            if use_hp {
                let out_l = hp_alpha * (self.hp_prev_out[0] + det_l - self.hp_prev_in[0]);
                let out_r = hp_alpha * (self.hp_prev_out[1] + det_r - self.hp_prev_in[1]);
                self.hp_prev_in = [det_l, det_r];
                self.hp_prev_out = [out_l, out_r];
                det_l = out_l;
                det_r = out_r;
            }
            let level_lin = if self.rms_mode {
                let sq = 0.5 * (det_l * det_l + det_r * det_r);
                self.rms_sq = sq + (self.rms_sq - sq) * rms_coeff;
                self.rms_sq.max(0.0).sqrt()
            } else {
                det_l.abs().max(det_r.abs())
            };
            let level_db = if level_lin <= 1e-5 {
                -100.0
            } else {
                20.0 * level_lin.log10()
            };
            let coeff = if level_db > self.env_db {
                attack_coeff
            } else {
                release_coeff
            };
            self.env_db = level_db + (self.env_db - level_db) * coeff;
            let out_db =
                Self::transfer_db(self.env_db, self.threshold_db, self.ratio, self.knee_db);
            let gr_db = (self.env_db - out_db).max(0.0);
            block_gr = block_gr.max(gr_db);
            let gain = 10.0_f32.powf(-gr_db / 20.0) * makeup_lin;
            frame[0] = frame[0] * (dry + wet * gain);
            frame[1] = frame[1] * (dry + wet * gain);
        }
        self.last_gr_db = block_gr;
    }
}

impl Primitive for Compressor {
    fn info(&self) -> &'static PrimitiveInfo {
        Self::INFO
    }

    fn parameter(&self, parameter: ParameterId) -> Result<f32, PrimitiveError> {
        Ok(match parameter {
            THRESHOLD => self.threshold_db,
            RATIO => self.ratio,
            ATTACK => self.attack_ms,
            RELEASE => self.release_ms,
            KNEE => self.knee_db,
            MAKEUP => self.makeup_db,
            MIX => self.mix_pct,
            MODE => {
                if self.rms_mode {
                    1.0
                } else {
                    0.0
                }
            }
            SC_GAIN => self.sc_gain_db,
            SC_HP => self.sc_hp_hz,
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
            THRESHOLD => self.threshold_db = value,
            RATIO => self.ratio = value,
            ATTACK => self.attack_ms = value,
            RELEASE => self.release_ms = value,
            KNEE => self.knee_db = value,
            MAKEUP => self.makeup_db = value,
            MIX => self.mix_pct = value,
            MODE => self.rms_mode = value.round() >= 1.0,
            SC_GAIN => self.sc_gain_db = value,
            SC_HP => self.sc_hp_hz = value,
            _ => unreachable!(),
        }
        Ok(())
    }

    fn prepare(&mut self, specification: ProcessSpec) {
        self.sample_rate = specification.sample_rate().max(8_000.0) as f32;
        self.reset();
    }

    fn process(&mut self, frames: &mut [StereoFrame]) {
        self.run_block(frames, None);
    }

    fn process_sidechain(&mut self, frames: &mut [StereoFrame], sidechain: Option<&[StereoFrame]>) {
        self.run_block(frames, sidechain);
    }

    fn gain_reduction_db(&self) -> f32 {
        self.last_gr_db
    }

    fn reset(&mut self) {
        self.env_db = -96.0;
        self.rms_sq = 0.0;
        self.hp_prev_in = [0.0; 2];
        self.hp_prev_out = [0.0; 2];
        self.last_gr_db = 0.0;
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
    fn compressor_attenuates_above_threshold_and_responds_to_external_sidechain() {
        let spec = ProcessSpec::new(48_000.0).unwrap();
        let mut comp = Compressor::new(spec);
        comp.set_parameter(THRESHOLD, -20.0).unwrap();
        comp.set_parameter(RATIO, 8.0).unwrap();
        comp.set_parameter(ATTACK, 0.5).unwrap();
        comp.set_parameter(RELEASE, 50.0).unwrap();
        comp.set_parameter(KNEE, 0.0).unwrap();

        // Sin sidechain: señal constante a -6 dBFS (amp 0.5) se comprime claramente.
        let mut audio = vec![[0.5, 0.5]; 2048];
        comp.process(&mut audio);
        assert!(audio[2047][0] < 0.22, "debió comprimir: {}", audio[2047][0]);
        assert!(comp.gain_reduction_db() > 8.0);

        // Con sidechain externo: audio suave (-30 dB, amp 0.03) + kick fuerte en sidechain (0.9).
        comp.reset();
        let mut bass = vec![[0.03, 0.03]; 2048];
        let kick = vec![[0.9, 0.9]; 2048];
        comp.process_sidechain(&mut bass, Some(&kick));
        assert!(
            bass[2047][0] < 0.01,
            "el sidechain del kick debió agachar el bajo: {}",
            bass[2047][0]
        );
    }
}
