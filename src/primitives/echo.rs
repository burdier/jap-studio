//! Eco estéreo estilo Ableton Live Echo: Stereo / Ping-Pong / Mid-Side, filtro HP/LP en
//! el bucle de feedback, saturación cálida de cinta (`drive`) y modulación sutil (`wobble`).
//! Sin reservas ni bloqueos en `process`; buffers preasignados al construir.
use crate::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};

pub const MODE: ParameterId = ParameterId(0);
pub const TIME_L: ParameterId = ParameterId(1);
pub const TIME_R: ParameterId = ParameterId(2);
pub const FEEDBACK: ParameterId = ParameterId(3);
pub const HP_HZ: ParameterId = ParameterId(4);
pub const LP_HZ: ParameterId = ParameterId(5);
pub const DRIVE: ParameterId = ParameterId(6);
pub const WOBBLE: ParameterId = ParameterId(7);
pub const MIX: ParameterId = ParameterId(8);
pub const SYNC: ParameterId = ParameterId(9);
pub const DIV_L: ParameterId = ParameterId(10);
pub const DIV_R: ParameterId = ParameterId(11);

pub const MODE_STEREO: u8 = 0;
pub const MODE_PING_PONG: u8 = 1;
pub const MODE_MID_SIDE: u8 = 2;

/// Divisiones musicales estilo Ableton Echo (`1/16` .. `1/1`).
pub const ECHO_DIVISIONS: [(&str, f64); 9] = [
    ("1/16", 0.25),
    ("1/8T", 1.0 / 3.0),
    ("1/8", 0.5),
    ("1/8.", 0.75),
    ("1/4T", 2.0 / 3.0),
    ("1/4", 1.0),
    ("1/4.", 1.5),
    ("1/2", 2.0),
    ("1/1", 4.0),
];

pub fn division_ms(index: u8, bpm: f64) -> f32 {
    let beats = ECHO_DIVISIONS
        .get(usize::from(index))
        .map_or(0.75, |&(_, b)| b);
    let safe_bpm = bpm.clamp(20.0, 360.0);
    ((beats * 60_000.0 / safe_bpm) as f32).clamp(1.0, 2000.0)
}

pub fn division_label(index: u8) -> &'static str {
    ECHO_DIVISIONS
        .get(usize::from(index))
        .map_or("1/8.", |&(l, _)| l)
}

/// Hasta 2.0 s a 192 kHz (384 000 muestras por canal), preasignado fuera de RT.
const MAX_DELAY_SAMPLES: usize = 384_000;

const PARAMETERS: &[ParameterSpec] = &[
    ParameterSpec {
        id: MODE,
        key: "mode",
        name: "Modo",
        unit: "tipo",
        min: 0.0,
        max: 2.0,
        default: 0.0,
    },
    ParameterSpec {
        id: TIME_L,
        key: "time_l",
        name: "Tiempo L",
        unit: "ms",
        min: 1.0,
        max: 2000.0,
        default: 375.0,
    },
    ParameterSpec {
        id: TIME_R,
        key: "time_r",
        name: "Tiempo R",
        unit: "ms",
        min: 1.0,
        max: 2000.0,
        default: 375.0,
    },
    ParameterSpec {
        id: FEEDBACK,
        key: "feedback",
        name: "Feedback",
        unit: "%",
        min: 0.0,
        max: 95.0,
        default: 42.0,
    },
    ParameterSpec {
        id: HP_HZ,
        key: "hp",
        name: "Filtro HP",
        unit: "Hz",
        min: 20.0,
        max: 4000.0,
        default: 120.0,
    },
    ParameterSpec {
        id: LP_HZ,
        key: "lp",
        name: "Filtro LP",
        unit: "Hz",
        min: 500.0,
        max: 20000.0,
        default: 6500.0,
    },
    ParameterSpec {
        id: DRIVE,
        key: "drive",
        name: "Drive",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 15.0,
    },
    ParameterSpec {
        id: WOBBLE,
        key: "wobble",
        name: "Wobble",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 12.0,
    },
    ParameterSpec {
        id: MIX,
        key: "mix",
        name: "Dry/Wet",
        unit: "%",
        min: 0.0,
        max: 100.0,
        default: 30.0,
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
        id: DIV_L,
        key: "div_l",
        name: "División L",
        unit: "tipo",
        min: 0.0,
        max: 8.0,
        default: 3.0,
    },
    ParameterSpec {
        id: DIV_R,
        key: "div_r",
        name: "División R",
        unit: "tipo",
        min: 0.0,
        max: 8.0,
        default: 3.0,
    },
];

pub struct Echo {
    sample_rate: f32,
    tempo_bpm: f64,
    bypassed: bool,
    mode: u8,
    time_l_ms: f32,
    time_r_ms: f32,
    feedback_pct: f32,
    hp_hz: f32,
    lp_hz: f32,
    drive_pct: f32,
    wobble_pct: f32,
    mix_pct: f32,
    sync: bool,
    div_l: u8,
    div_r: u8,

    // Estado interno preasignado
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    write_pos: usize,
    smooth_delay_l: f32,
    smooth_delay_r: f32,
    delay_initialized: bool,
    hp_prev_in: [f32; 2],
    hp_prev_out: [f32; 2],
    lp_state: [f32; 2],
    lfo_phase: f32,
}

impl Echo {
    pub const INFO: &'static PrimitiveInfo = &PrimitiveInfo {
        id: "elevate.time.echo",
        name: "Hay Echo",
        properties: &[SignalProperty::Time, SignalProperty::Spatial],
        parameters: PARAMETERS,
    };

    pub fn new(specification: ProcessSpec) -> Self {
        let sr = specification.sample_rate().max(8_000.0) as f32;
        let init_delay = (375.0 * 0.001 * sr).clamp(1.0, (MAX_DELAY_SAMPLES - 4) as f32);
        Self {
            sample_rate: sr,
            tempo_bpm: 120.0,
            bypassed: false,
            mode: MODE_STEREO,
            time_l_ms: 375.0,
            time_r_ms: 375.0,
            feedback_pct: 42.0,
            hp_hz: 120.0,
            lp_hz: 6500.0,
            drive_pct: 15.0,
            wobble_pct: 12.0,
            mix_pct: 30.0,
            sync: false,
            div_l: 3,
            div_r: 3,
            buf_l: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            buf_r: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            write_pos: 0,
            smooth_delay_l: init_delay,
            smooth_delay_r: init_delay,
            delay_initialized: false,
            hp_prev_in: [0.0; 2],
            hp_prev_out: [0.0; 2],
            lp_state: [0.0; 2],
            lfo_phase: 0.0,
        }
    }

    #[inline]
    fn read_linear(buf: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
        let len = buf.len();
        let d = delay_samples.clamp(1.0, (len - 2) as f32);
        let int_part = d.floor() as usize;
        let frac = d - int_part as f32;
        let idx0 = (write_pos + len - int_part) % len;
        let idx1 = (write_pos + len - int_part - 1) % len;
        buf[idx0] + (buf[idx1] - buf[idx0]) * frac
    }

    #[inline]
    fn saturate(sample: f32, drive: f32) -> f32 {
        if drive <= 0.001 {
            return sample.clamp(-4.0, 4.0);
        }
        let g = 1.0 + drive * 3.5;
        (sample * g).tanh() / g.tanh().max(0.1)
    }
}

impl Primitive for Echo {
    fn info(&self) -> &'static PrimitiveInfo {
        Self::INFO
    }

    fn parameter(&self, parameter: ParameterId) -> Result<f32, PrimitiveError> {
        Ok(match parameter {
            MODE => f32::from(self.mode),
            TIME_L => self.time_l_ms,
            TIME_R => self.time_r_ms,
            FEEDBACK => self.feedback_pct,
            HP_HZ => self.hp_hz,
            LP_HZ => self.lp_hz,
            DRIVE => self.drive_pct,
            WOBBLE => self.wobble_pct,
            MIX => self.mix_pct,
            SYNC => {
                if self.sync {
                    1.0
                } else {
                    0.0
                }
            }
            DIV_L => f32::from(self.div_l),
            DIV_R => f32::from(self.div_r),
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
            MODE => self.mode = value.round().clamp(0.0, 2.0) as u8,
            TIME_L => self.time_l_ms = value,
            TIME_R => self.time_r_ms = value,
            FEEDBACK => self.feedback_pct = value,
            HP_HZ => self.hp_hz = value,
            LP_HZ => self.lp_hz = value,
            DRIVE => self.drive_pct = value,
            WOBBLE => self.wobble_pct = value,
            MIX => self.mix_pct = value,
            SYNC => self.sync = value >= 0.5,
            DIV_L => self.div_l = value.round().clamp(0.0, 8.0) as u8,
            DIV_R => self.div_r = value.round().clamp(0.0, 8.0) as u8,
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
        let max_d = (MAX_DELAY_SAMPLES - 4) as f32;
        let eff_l_ms = if self.sync {
            division_ms(self.div_l, self.tempo_bpm)
        } else {
            self.time_l_ms
        };
        let eff_r_ms = if self.sync {
            division_ms(self.div_r, self.tempo_bpm)
        } else {
            self.time_r_ms
        };
        let target_l = (eff_l_ms * 0.001 * sr).clamp(1.0, max_d);
        let target_r = (eff_r_ms * 0.001 * sr).clamp(1.0, max_d);
        if !self.delay_initialized {
            self.smooth_delay_l = target_l;
            self.smooth_delay_r = target_r;
            self.delay_initialized = true;
        }
        let delay_smooth = (-1.0 / (0.025 * sr)).exp();

        let fb = (self.feedback_pct / 100.0).clamp(0.0, 0.95);
        let drive = (self.drive_pct / 100.0).clamp(0.0, 1.0);
        let wobble_depth_samples = (self.wobble_pct / 100.0).clamp(0.0, 1.0) * 0.0045 * sr;
        let lfo_inc = std::f32::consts::TAU * 0.65 / sr;
        let wet = (self.mix_pct / 100.0).clamp(0.0, 1.0);
        let dry = 1.0 - wet;

        let dt = 1.0 / sr;
        let hp_rc = 1.0 / (std::f32::consts::TAU * self.hp_hz.clamp(20.0, sr * 0.45));
        let hp_alpha = hp_rc / (hp_rc + dt);
        let lp_rc = 1.0 / (std::f32::consts::TAU * self.lp_hz.clamp(100.0, sr * 0.45));
        let lp_alpha = dt / (lp_rc + dt);

        let len = self.buf_l.len();

        for frame in frames.iter_mut() {
            self.smooth_delay_l = target_l + (self.smooth_delay_l - target_l) * delay_smooth;
            self.smooth_delay_r = target_r + (self.smooth_delay_r - target_r) * delay_smooth;

            self.lfo_phase += lfo_inc;
            if self.lfo_phase >= std::f32::consts::TAU {
                self.lfo_phase -= std::f32::consts::TAU;
            }
            let mod_l = self.lfo_phase.sin() * wobble_depth_samples;
            let mod_r = (self.lfo_phase + 1.570_796_4).sin() * wobble_depth_samples;

            let dl = (self.smooth_delay_l + mod_l).clamp(1.0, max_d);
            let dr = (self.smooth_delay_r + mod_r).clamp(1.0, max_d);

            let tap_l = Self::read_linear(&self.buf_l, self.write_pos, dl);
            let tap_r = Self::read_linear(&self.buf_r, self.write_pos, dr);

            // Filtro HP + LP + Drive dentro del lazo de retroalimentación (timbre analógico).
            let mut proc = [tap_l, tap_r];
            for ch in 0..2 {
                let hp = hp_alpha * (self.hp_prev_out[ch] + proc[ch] - self.hp_prev_in[ch]);
                self.hp_prev_in[ch] = proc[ch];
                self.hp_prev_out[ch] = hp;
                let lp = self.lp_state[ch] + lp_alpha * (hp - self.lp_state[ch]);
                self.lp_state[ch] = lp;
                proc[ch] = Self::saturate(lp, drive);
            }

            let in_l = frame[0];
            let in_r = frame[1];

            let (write_l, write_r, out_l, out_r) = match self.mode {
                MODE_PING_PONG => {
                    // Entrada sumada a mono en L y rebote cruzado L <-> R.
                    let mono = 0.5 * (in_l + in_r);
                    let wl = mono + proc[1] * fb;
                    let wr = proc[0] * fb;
                    (wl, wr, proc[0], proc[1])
                }
                MODE_MID_SIDE => {
                    let mid = 0.5 * (in_l + in_r);
                    let side = 0.5 * (in_l - in_r);
                    let wl = mid + proc[0] * fb;
                    let wr = side + proc[1] * fb;
                    let ol = proc[0] + proc[1];
                    let or = proc[0] - proc[1];
                    (wl, wr, ol, or)
                }
                _ => {
                    // Stereo independiente L / R
                    let wl = in_l + proc[0] * fb;
                    let wr = in_r + proc[1] * fb;
                    (wl, wr, proc[0], proc[1])
                }
            };

            self.buf_l[self.write_pos] = if write_l.is_finite() {
                write_l.clamp(-4.0, 4.0)
            } else {
                0.0
            };
            self.buf_r[self.write_pos] = if write_r.is_finite() {
                write_r.clamp(-4.0, 4.0)
            } else {
                0.0
            };
            self.write_pos = (self.write_pos + 1) % len;

            frame[0] = in_l * dry + out_l * wet;
            frame[1] = in_r * dry + out_r * wet;
        }
    }

    fn reset(&mut self) {
        self.buf_l.fill(0.0);
        self.buf_r.fill(0.0);
        self.write_pos = 0;
        let max_d = (MAX_DELAY_SAMPLES - 4) as f32;
        self.smooth_delay_l = (self.time_l_ms * 0.001 * self.sample_rate).clamp(1.0, max_d);
        self.smooth_delay_r = (self.time_r_ms * 0.001 * self.sample_rate).clamp(1.0, max_d);
        self.delay_initialized = false;
        self.hp_prev_in = [0.0; 2];
        self.hp_prev_out = [0.0; 2];
        self.lp_state = [0.0; 2];
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
    fn echo_produces_delayed_repeats_and_ping_pong_alternates_channels() {
        let spec = ProcessSpec::new(48_000.0).unwrap();
        let mut echo = Echo::new(spec);
        echo.set_parameter(MODE, f32::from(MODE_PING_PONG)).unwrap();
        echo.set_parameter(TIME_L, 10.0).unwrap(); // 480 muestras
        echo.set_parameter(TIME_R, 10.0).unwrap(); // 480 muestras
        echo.set_parameter(FEEDBACK, 60.0).unwrap();
        echo.set_parameter(WOBBLE, 0.0).unwrap();
        echo.set_parameter(MIX, 100.0).unwrap();
        echo.reset();

        let mut frames = vec![[0.0; 2]; 2000];
        frames[0] = [1.0, 1.0];
        echo.process(&mut frames);

        // Primer eco (~480) aparece en L, segundo (~960) aparece en R.
        let l1 = frames[475..486]
            .iter()
            .map(|f| f[0].abs())
            .fold(0.0_f32, f32::max);
        let r1 = frames[475..486]
            .iter()
            .map(|f| f[1].abs())
            .fold(0.0_f32, f32::max);
        let r2 = frames[955..966]
            .iter()
            .map(|f| f[1].abs())
            .fold(0.0_f32, f32::max);
        assert!(l1 > 0.25, "primer eco ping-pong en L esperado, obt {l1}");
        assert!(
            r1 < 0.02,
            "R debe permanecer casi en silencio en tap 1, obt {r1}"
        );
        assert!(r2 > 0.10, "segundo eco ping-pong en R esperado, obt {r2}");
    }

    #[test]
    fn echo_sync_mode_tracks_daw_bpm_divisions_accurately() {
        let spec = ProcessSpec::new(48_000.0).unwrap();
        let mut echo = Echo::new(spec);
        echo.set_parameter(SYNC, 1.0).unwrap();
        echo.set_parameter(DIV_L, 0.0).unwrap(); // 1/16 = 0.25 beats
        echo.set_parameter(DIV_R, 0.0).unwrap();
        echo.set_parameter(WOBBLE, 0.0).unwrap();
        echo.set_parameter(MIX, 100.0).unwrap();
        // A 300 BPM, 1/16 = 0.25 * 60000 / 300 = 50 ms = 2400 muestras @ 48 kHz.
        echo.set_tempo_bpm(300.0);
        echo.reset();

        let mut frames = vec![[0.0; 2]; 3000];
        frames[0] = [1.0, 1.0];
        echo.process(&mut frames);

        let peak_at_50ms = frames[2395..2406]
            .iter()
            .map(|f| f[0].abs())
            .fold(0.0_f32, f32::max);
        assert!(
            peak_at_50ms > 0.25,
            "eco Sync 1/16 @ 300 BPM esperado en muestra 2400, obt {peak_at_50ms}"
        );
    }
}
