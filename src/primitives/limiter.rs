//! Limiter lookahead estéreo estilo Ableton Live (Gain, Ceiling, Release, Lookahead 1.5/3/6 ms).
//! Cero asignaciones en RT; buffer circular prealocado y medidor de Gain Reduction.
use super::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};

pub const GAIN: ParameterId = ParameterId(0);
pub const CEILING: ParameterId = ParameterId(1);
pub const RELEASE: ParameterId = ParameterId(2);
pub const LOOKAHEAD: ParameterId = ParameterId(3);

/// Soporta hasta 6 ms a 192 kHz (~1152 frames) con holgura fija en heap preparado fuera de RT.
const MAX_LOOKAHEAD: usize = 2048;

const PARAMETERS: [ParameterSpec; 4] = [
    ParameterSpec {
        id: GAIN,
        key: "gain",
        name: "Gain",
        unit: "dB",
        min: -24.0,
        max: 24.0,
        default: 0.0,
    },
    ParameterSpec {
        id: CEILING,
        key: "ceiling",
        name: "Ceiling",
        unit: "dB",
        min: -24.0,
        max: 0.0,
        default: -0.3,
    },
    ParameterSpec {
        id: RELEASE,
        key: "release",
        name: "Release",
        unit: "ms",
        min: 1.0,
        max: 1000.0,
        default: 100.0,
    },
    ParameterSpec {
        id: LOOKAHEAD,
        key: "lookahead",
        name: "Lookahead",
        unit: "tipo",
        min: 0.0,
        max: 2.0,
        default: 1.0,
    },
];

static INFO: PrimitiveInfo = PrimitiveInfo {
    id: "elevate.dynamics.limiter",
    name: "Limiter",
    properties: &[SignalProperty::Amplitude],
    parameters: &PARAMETERS,
};

pub struct Limiter {
    sample_rate: f32,
    gain_db: f32,
    ceiling_db: f32,
    release_ms: f32,
    lookahead_mode: u8,
    bypassed: bool,
    // Buffer circular prealocado para el lookahead
    delay: Box<[StereoFrame; MAX_LOOKAHEAD]>,
    peaks: Box<[f32; MAX_LOOKAHEAD]>,
    write_pos: usize,
    env_gain: f32,
    last_gr_db: f32,
}

impl Limiter {
    pub const INFO: &'static PrimitiveInfo = &INFO;

    pub fn new(spec: ProcessSpec) -> Self {
        Self {
            sample_rate: spec.sample_rate().max(8_000.0) as f32,
            gain_db: 0.0,
            ceiling_db: -0.3,
            release_ms: 100.0,
            lookahead_mode: 1, // 3.0 ms por defecto como Ableton
            bypassed: false,
            delay: Box::new([[0.0; 2]; MAX_LOOKAHEAD]),
            peaks: Box::new([0.0; MAX_LOOKAHEAD]),
            write_pos: 0,
            env_gain: 1.0,
            last_gr_db: 0.0,
        }
    }

    fn lookahead_samples(&self) -> usize {
        let ms = match self.lookahead_mode {
            0 => 1.5,
            2 => 6.0,
            _ => 3.0,
        };
        ((self.sample_rate * ms * 0.001).round() as usize).clamp(1, MAX_LOOKAHEAD - 1)
    }
}

impl Primitive for Limiter {
    fn info(&self) -> &'static PrimitiveInfo {
        Self::INFO
    }

    fn parameter(&self, parameter: ParameterId) -> Result<f32, PrimitiveError> {
        Ok(match parameter {
            GAIN => self.gain_db,
            CEILING => self.ceiling_db,
            RELEASE => self.release_ms,
            LOOKAHEAD => f32::from(self.lookahead_mode),
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
            GAIN => self.gain_db = value,
            CEILING => self.ceiling_db = value,
            RELEASE => self.release_ms = value,
            LOOKAHEAD => self.lookahead_mode = value.round().clamp(0.0, 2.0) as u8,
            _ => unreachable!(),
        }
        Ok(())
    }

    fn prepare(&mut self, specification: ProcessSpec) {
        self.sample_rate = specification.sample_rate().max(8_000.0) as f32;
        self.reset();
    }

    fn process(&mut self, frames: &mut [StereoFrame]) {
        if self.bypassed || frames.is_empty() {
            self.last_gr_db = 0.0;
            return;
        }
        let sr = self.sample_rate.max(8_000.0);
        let in_gain = 10.0_f32.powf(self.gain_db / 20.0);
        let ceiling = 10.0_f32.powf(self.ceiling_db / 20.0).max(1e-4);
        let lookahead = self.lookahead_samples();
        let attack_coeff = (-1.0 / (lookahead as f32 * 0.25).max(1.0)).exp();
        let release_coeff = (-1.0 / (self.release_ms.max(1.0) * 0.001 * sr)).exp();

        let mut min_gain = 1.0_f32;
        for frame in frames.iter_mut() {
            let driven = [frame[0] * in_gain, frame[1] * in_gain];
            let peak = driven[0].abs().max(driven[1].abs());

            self.delay[self.write_pos] = driven;
            self.peaks[self.write_pos] = peak;

            // Máximo pico dentro de la ventana de lookahead (acotada a ~72..288 muestras @ 48kHz).
            let mut win_peak = peak;
            for k in 1..lookahead {
                let idx = (self.write_pos + MAX_LOOKAHEAD - k) % MAX_LOOKAHEAD;
                let p = self.peaks[idx];
                if p > win_peak {
                    win_peak = p;
                }
            }

            let read_pos = (self.write_pos + MAX_LOOKAHEAD - lookahead) % MAX_LOOKAHEAD;
            let delayed = self.delay[read_pos];
            self.write_pos = (self.write_pos + 1) % MAX_LOOKAHEAD;

            let target_gain = if win_peak > ceiling {
                ceiling / win_peak
            } else {
                1.0
            };
            let coeff = if target_gain < self.env_gain {
                attack_coeff
            } else {
                release_coeff
            };
            self.env_gain = target_gain + (self.env_gain - target_gain) * coeff;

            let delayed_peak = delayed[0].abs().max(delayed[1].abs());
            let instant_gain = if delayed_peak > ceiling {
                ceiling / delayed_peak
            } else {
                1.0
            };
            let applied = self.env_gain.min(instant_gain).min(1.0);
            self.env_gain = applied;
            min_gain = min_gain.min(applied);

            frame[0] = (delayed[0] * applied).clamp(-ceiling, ceiling);
            frame[1] = (delayed[1] * applied).clamp(-ceiling, ceiling);
        }
        self.last_gr_db = if min_gain >= 0.9999 {
            0.0
        } else {
            (-20.0 * min_gain.max(1e-6).log10()).max(0.0)
        };
    }

    fn gain_reduction_db(&self) -> f32 {
        self.last_gr_db
    }

    fn reset(&mut self) {
        self.delay.fill([0.0; 2]);
        self.peaks.fill(0.0);
        self.write_pos = 0;
        self.env_gain = 1.0;
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
    fn limiter_enforces_ceiling_and_reports_gain_reduction() {
        let spec = ProcessSpec::new(48_000.0).unwrap();
        let mut lim = Limiter::new(spec);
        lim.set_parameter(GAIN, 12.0).unwrap();
        lim.set_parameter(CEILING, -1.0).unwrap();
        let ceiling_lin = 10.0_f32.powf(-1.0 / 20.0);

        let mut audio: Vec<StereoFrame> = (0..4096)
            .map(|i| {
                let s = (i as f32 * 0.1).sin() * 0.7;
                [s, -s]
            })
            .collect();
        lim.process(&mut audio);
        assert!(
            audio
                .iter()
                .flatten()
                .all(|&s| s.abs() <= ceiling_lin + 1e-4),
            "el limiter no debe superar el techo"
        );
        assert!(
            lim.gain_reduction_db() > 6.0,
            "debió reportar GR > 6 dB, obtuvo {:.2}",
            lim.gain_reduction_db()
        );
    }
}
