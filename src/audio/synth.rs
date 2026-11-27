//! Motor DSP polifónico en tiempo real de Hay Synth (Stage + Voice Lab estilo Vital + Mod Matrix).
//! 0 asignaciones en render, 0 bloqueos, 0 dependencias externas.
use elevate::devices::synth::{
    CUSTOM_WT_MIPS, CustomWavetable, DistMode, EnvelopeMode, FilterKind, LfoShape, ModSource,
    ModTarget, RoomKind, SynthEngineKind, SynthParams, WarpMode, WavetableKind,
    build_bandlimited_mips, cc,
};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

const MAX_VOICES: usize = 12;
const MAX_UNISON: usize = 7;
const PLUCK_BUF: usize = 2048;
const WT_TABLES: usize = 8;
const WT_FRAMES: usize = 8;
const WT_SIZE: usize = 256;
const CHORUS_BUF: usize = 2048;

/// Banco Hi-Fi de 8 wavetables × 6 mip-maps band-limited × 8 frames × 256 muestras.
struct WavetableBank {
    mips: Vec<Box<[[[f32; WT_SIZE]; WT_FRAMES]; CUSTOM_WT_MIPS]>>,
}

impl WavetableBank {
    fn new() -> Self {
        let mut data = Box::new([[[0.0_f32; WT_SIZE]; WT_FRAMES]; WT_TABLES]);

        let build_additive = |dst: &mut [f32; WT_SIZE], amps: &[(usize, f32, f32)]| {
            let mut max_abs = 1e-5_f32;
            for i in 0..WT_SIZE {
                let p = (i as f32 / WT_SIZE as f32) * TAU;
                let mut s = 0.0_f32;
                for &(h, a, ph) in amps {
                    s += a * (p * h as f32 + ph).sin();
                }
                dst[i] = s;
                max_abs = max_abs.max(s.abs());
            }
            let inv = 0.95 / max_abs;
            for x in dst.iter_mut() {
                *x *= inv;
            }
        };

        // 0: BasicShapes (Sine -> Tri -> Saw -> Square -> Pulse)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let mut amps = Vec::new();
            for h in 1..=24usize {
                let hf = h as f32;
                let sine_a = if h == 1 { 1.0 } else { 0.0 };
                let tri_a = if h % 2 == 1 {
                    let sign = if (h / 2) % 2 == 0 { 1.0 } else { -1.0 };
                    sign / (hf * hf)
                } else {
                    0.0
                };
                let saw_a = (if h % 2 == 1 { 1.0 } else { -1.0 }) / hf;
                let sq_a = if h % 2 == 1 { 1.0 / hf } else { 0.0 };
                let pulse_a = (PI * hf * 0.22).sin() / hf;
                let a = if pos < 0.25 {
                    let t = pos * 4.0;
                    sine_a * (1.0 - t) + tri_a * t
                } else if pos < 0.50 {
                    let t = (pos - 0.25) * 4.0;
                    tri_a * (1.0 - t) + saw_a * t
                } else if pos < 0.75 {
                    let t = (pos - 0.50) * 4.0;
                    saw_a * (1.0 - t) + sq_a * t
                } else {
                    let t = (pos - 0.75) * 4.0;
                    sq_a * (1.0 - t) + pulse_a * t
                };
                if a.abs() > 1e-4 {
                    amps.push((h, a, 0.0));
                }
            }
            build_additive(&mut data[0][f], &amps);
        }

        // 1: AnalogWarm (armónicos tipo Moog/Juno con rolloff cálido)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let mut amps = Vec::new();
            for h in 1..=20usize {
                let hf = h as f32;
                let tilt = 1.0 / hf.powf(1.35 - pos * 0.55);
                let even_boost = if h % 2 == 0 { 0.35 + pos * 0.65 } else { 1.0 };
                amps.push((h, tilt * even_boost, (h as f32) * 0.17 * pos));
            }
            build_additive(&mut data[1][f], &amps);
        }

        // 2: DigitalGlass (parciales cristalinos y brillantes)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let mut amps = Vec::new();
            let harmonics = [1, 2, 3, 5, 7, 9, 11, 13, 17, 19, 23];
            for (k, &h) in harmonics.iter().enumerate() {
                let weight = ((k as f32 / harmonics.len() as f32) - pos).abs();
                let a = (-weight * 3.8).exp() / (h as f32).powf(0.45);
                amps.push((h, a, (k as f32) * 0.6));
            }
            build_additive(&mut data[2][f], &amps);
        }

        // 3: VocalFormant (espectro aditivo de formantes A -> E -> I -> O -> U)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let (f1, f2, f3) = vowel_formants(pos);
            let mut amps = Vec::new();
            let fund = 140.0_f32;
            for h in 1..=28usize {
                let freq = fund * h as f32;
                let g1 = (-((freq - f1) / 210.0).powi(2)).exp();
                let g2 = (-((freq - f2) / 340.0).powi(2)).exp() * 0.85;
                let g3 = (-((freq - f3) / 480.0).powi(2)).exp() * 0.45;
                let a = (0.08 / h as f32) + g1 + g2 + g3;
                amps.push((h, a, 0.0));
            }
            build_additive(&mut data[3][f], &amps);
        }

        // 4: GrowlBass (Reese / Dubstep formantes agresivos + muescas)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let mut amps = Vec::new();
            let center = 3.0 + pos * 14.0;
            for h in 1..=28usize {
                let hf = h as f32;
                let base = 0.65 / hf.powf(0.7);
                let peak = (-((hf - center) / 2.2).powi(2)).exp() * 1.1;
                let notch = 1.0 - 0.75 * (-((hf - (center + 4.0)) / 1.4).powi(2)).exp();
                amps.push((h, (base + peak) * notch, (hf * 0.35 * pos)));
            }
            build_additive(&mut data[4][f], &amps);
        }

        // 5: OrganBell (drawbars -> campana aditiva)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let mut amps = Vec::new();
            let drawbars = [
                (1, 1.0),
                (2, 0.75),
                (3, 0.60),
                (4, 0.50),
                (6, 0.35 + pos * 0.3),
                (8, 0.20 + pos * 0.45),
                (10, pos * 0.38),
                (12, pos * 0.32),
                (15, pos * 0.25),
            ];
            for &(h, a) in &drawbars {
                amps.push((h, a, 0.0));
            }
            build_additive(&mut data[5][f], &amps);
        }

        // 6: PluckString (cuerda tensa con armónicos nítidos)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let pick = 0.14 + pos * 0.34;
            let mut amps = Vec::new();
            for h in 1..=24usize {
                let hf = h as f32;
                let comb = (PI * hf * pick).sin().abs();
                let a = comb / hf.powf(0.95);
                if a > 1e-4 {
                    amps.push((h, a, 0.0));
                }
            }
            build_additive(&mut data[6][f], &amps);
        }

        // 7: HarshFold (wavefolder pre-horneado por frames)
        for f in 0..WT_FRAMES {
            let pos = f as f32 / (WT_FRAMES - 1) as f32;
            let fold = 1.2 + pos * 4.5;
            let mut max_abs = 1e-5_f32;
            for i in 0..WT_SIZE {
                let p = (i as f32 / WT_SIZE as f32) * TAU;
                let s = (p.sin() * fold + (p * 2.0).sin() * 0.35 * pos).sin();
                data[7][f][i] = s;
                max_abs = max_abs.max(s.abs());
            }
            let inv = 0.95 / max_abs;
            for x in data[7][f].iter_mut() {
                *x *= inv;
            }
        }

        let mips = (0..WT_TABLES)
            .map(|t| build_bandlimited_mips(&data[t]))
            .collect();
        Self { mips }
    }

    #[inline]
    fn sample(&self, kind: WavetableKind, frame_pos: f32, phase: f32, hz: f32) -> f32 {
        let t = (kind as usize).min(WT_TABLES - 1);
        sample_mip_table(&self.mips[t], frame_pos, phase, hz)
    }

    #[inline]
    fn sample_custom(wt: &CustomWavetable, frame_pos: f32, phase: f32, hz: f32) -> f32 {
        sample_mip_table(&wt.mips, frame_pos, phase, hz)
    }
}

/// Interpolación cúbica Catmull-Rom de 4 puntos (Hi-Fi, elimina ruido de cuantización lineal).
#[inline]
fn cubic_read(table: &[f32; WT_SIZE], phase: f32) -> f32 {
    let p = phase.fract();
    let p = if p < 0.0 { p + 1.0 } else { p };
    let idx_f = p * WT_SIZE as f32;
    let i1 = (idx_f as usize) & (WT_SIZE - 1);
    let i0 = i1.wrapping_sub(1) & (WT_SIZE - 1);
    let i2 = (i1 + 1) & (WT_SIZE - 1);
    let i3 = (i1 + 2) & (WT_SIZE - 1);
    let t = idx_f - idx_f.floor();
    let y0 = table[i0];
    let y1 = table[i1];
    let y2 = table[i2];
    let y3 = table[i3];
    let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let a2 = -0.5 * y0 + 0.5 * y2;
    ((a0 * t + a1) * t + a2) * t + y1
}

/// Muestreo band-limited con selección continua de nivel mip según `hz` (0 aliasing).
#[inline]
fn sample_mip_table(
    mips: &[[[f32; WT_SIZE]; WT_FRAMES]; CUSTOM_WT_MIPS],
    frame_pos: f32,
    phase: f32,
    hz: f32,
) -> f32 {
    let mip_f = ((hz.max(60.0) / 115.0).max(1.0).log2()).clamp(0.0, (CUSTOM_WT_MIPS - 1) as f32);
    let m0 = (mip_f.floor() as usize).min(CUSTOM_WT_MIPS - 2);
    let m1 = m0 + 1;
    let mf = mip_f - m0 as f32;

    let fp = frame_pos.clamp(0.0, 1.0) * (WT_FRAMES - 1) as f32;
    let f0 = (fp.floor() as usize).min(WT_FRAMES - 2);
    let f1 = f0 + 1;
    let ff = fp - f0 as f32;

    let s_m0 = {
        let a = cubic_read(&mips[m0][f0], phase);
        let b = cubic_read(&mips[m0][f1], phase);
        a + (b - a) * ff
    };
    let s_m1 = {
        let a = cubic_read(&mips[m1][f0], phase);
        let b = cubic_read(&mips[m1][f1], phase);
        a + (b - a) * ff
    };
    s_m0 + (s_m1 - s_m0) * mf
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Filtro TPT SVF mono (pasa-bajos, pasa-banda, pasa-altos, notch) estable a cualquier frecuencia.
#[derive(Clone, Copy, Default)]
struct Svf {
    ic1eq: f32,
    ic2eq: f32,
}

impl Svf {
    fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    #[inline]
    fn process_all(&mut self, v0: f32, cutoff_hz: f32, q: f32, sr: f32) -> (f32, f32, f32, f32) {
        let fc = cutoff_hz.clamp(22.0, sr * 0.44);
        let g = (PI * fc / sr).tan();
        let k = 1.0 / q.clamp(0.5, 11.0);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        let hp = v0 - k * v1 - v2;
        let notch = v0 - k * v1;
        (v2, v1, hp, notch) // (lp, bp, hp, notch)
    }

    #[inline]
    fn process(&mut self, v0: f32, cutoff_hz: f32, q: f32, sr: f32) -> (f32, f32) {
        let (lp, bp, _, _) = self.process_all(v0, cutoff_hz, q, sr);
        (lp, bp)
    }
}

/// Chorus estéreo preasignado (0 allocs en RT).
struct ChorusUnit {
    buf_l: Box<[f32; CHORUS_BUF]>,
    buf_r: Box<[f32; CHORUS_BUF]>,
    write_idx: usize,
    phase: f32,
}

impl ChorusUnit {
    fn new() -> Self {
        Self {
            buf_l: Box::new([0.0; CHORUS_BUF]),
            buf_r: Box::new([0.0; CHORUS_BUF]),
            write_idx: 0,
            phase: 0.0,
        }
    }

    fn reset(&mut self) {
        self.buf_l.fill(0.0);
        self.buf_r.fill(0.0);
        self.write_idx = 0;
        self.phase = 0.0;
    }

    #[inline]
    fn read_delay(buf: &[f32; CHORUS_BUF], write_idx: usize, delay_samples: f32) -> f32 {
        let len = CHORUS_BUF as f32;
        let mut r = write_idx as f32 - delay_samples;
        while r < 0.0 {
            r += len;
        }
        let i0 = (r as usize) & (CHORUS_BUF - 1);
        let i1 = (i0 + 1) & (CHORUS_BUF - 1);
        let frac = r - r.floor();
        buf[i0] + (buf[i1] - buf[i0]) * frac
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, mix: f32, rate: f32, sr: f32) -> (f32, f32) {
        self.buf_l[self.write_idx] = in_l;
        self.buf_r[self.write_idx] = in_r;
        let hz = 0.15 + rate * rate * 4.2;
        self.phase = (self.phase + hz / sr).fract();
        if mix <= 0.001 {
            self.write_idx = (self.write_idx + 1) & (CHORUS_BUF - 1);
            return (in_l, in_r);
        }
        let lfo_l = (self.phase * TAU).sin();
        let lfo_r = ((self.phase + 0.25) * TAU).sin();
        let base_ms = 0.011 * sr; // ~11 ms
        let depth = 0.0045 * sr; // ±4.5 ms
        let dl = (base_ms + lfo_l * depth).clamp(4.0, (CHORUS_BUF - 4) as f32);
        let dr = (base_ms + lfo_r * depth).clamp(4.0, (CHORUS_BUF - 4) as f32);
        let wet_l = Self::read_delay(&self.buf_l, self.write_idx, dl);
        let wet_r = Self::read_delay(&self.buf_r, self.write_idx, dr);
        self.write_idx = (self.write_idx + 1) & (CHORUS_BUF - 1);
        let dry = 1.0 - mix * 0.35;
        (in_l * dry + wet_l * mix * 0.72, in_r * dry + wet_r * mix * 0.72)
    }
}

#[derive(Default)]
struct ModAccum {
    osc_a_pos: f32,
    osc_a_warp: f32,
    osc_a_pitch: f32,
    osc_a_detune: f32,
    osc_a_level: f32,
    osc_b_pos: f32,
    osc_b_pitch: f32,
    osc_b_level: f32,
    sub_level: f32,
    noise_level: f32,
    filter_cutoff: f32,
    filter_res: f32,
    filter_drive: f32,
    chorus_mix: f32,
    dist_drive: f32,
    distance: f32,
    lfo1_rate: f32,
    lfo2_rate: f32,
    pan: f32,
    out_gain: f32,
}

struct Voice {
    active: bool,
    note: u8,
    age: u64,
    velocity: f32,
    pan: f32,
    voice_rand: f32,
    freq: f32,
    target_freq: f32,
    phase: [f32; 3],
    osc_a_uni: [f32; MAX_UNISON],
    osc_b_uni: [f32; MAX_UNISON],
    sub_phase: f32,
    mod_phase: f32,
    lfo_phase: f32,
    lfo1_phase: f32,
    lfo2_phase: f32,
    lfo1_sh: f32,
    lfo2_sh: f32,
    pitch_env: f32,
    timbre_env: f32,
    env: f32,
    stage: EnvStage,
    env2: f32,
    env2_stage: EnvStage,
    env3: f32,
    noise_lp: f32,
    cut_smooth: f32,
    svf_l: Svf,
    svf_r: Svf,
    svf2_l: Svf,
    svf2_r: Svf,
    formants: [Svf; 3],
    pluck_buf: Box<[f32; PLUCK_BUF]>,
    pluck_idx: usize,
    pluck_len: usize,
    pluck_lp: f32,
    rng: u32,
}

impl Voice {
    fn new(seed: u32) -> Self {
        Self {
            active: false,
            note: 0,
            age: 0,
            velocity: 0.0,
            pan: 0.0,
            voice_rand: 0.0,
            freq: 261.63,
            target_freq: 261.63,
            phase: [0.0; 3],
            osc_a_uni: [0.0, 0.13, 0.37, 0.59, 0.71, 0.83, 0.91],
            osc_b_uni: [0.05, 0.19, 0.41, 0.63, 0.77, 0.89, 0.97],
            sub_phase: 0.0,
            mod_phase: 0.0,
            lfo_phase: (seed as f32 * 0.137).fract(),
            lfo1_phase: (seed as f32 * 0.193).fract(),
            lfo2_phase: (seed as f32 * 0.311).fract(),
            lfo1_sh: 0.0,
            lfo2_sh: 0.0,
            pitch_env: 0.0,
            timbre_env: 0.0,
            env: 0.0,
            stage: EnvStage::Off,
            env2: 0.0,
            env2_stage: EnvStage::Off,
            env3: 0.0,
            noise_lp: 0.0,
            cut_smooth: 2400.0,
            svf_l: Svf::default(),
            svf_r: Svf::default(),
            svf2_l: Svf::default(),
            svf2_r: Svf::default(),
            formants: [Svf::default(); 3],
            pluck_buf: Box::new([0.0; PLUCK_BUF]),
            pluck_idx: 0,
            pluck_len: 128,
            pluck_lp: 0.0,
            rng: seed.wrapping_mul(747796405).wrapping_add(2891336453) | 1,
        }
    }

    #[inline]
    fn next_noise(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn trigger(
        &mut self,
        note: u8,
        velocity: u8,
        pan: f32,
        start_freq: f32,
        age: u64,
        params: &SynthParams,
        sr: f32,
    ) {
        let target = midi_to_hz(note);
        self.active = true;
        self.note = note;
        self.age = age;
        self.velocity = (velocity.max(1) as f32 / 127.0).clamp(0.02, 1.0);
        self.pan = pan.clamp(-1.0, 1.0);
        self.voice_rand = self.next_noise() * 0.5 + 0.5;
        self.target_freq = target;
        self.freq = if params.glide > 0.02 && start_freq > 15.0 {
            start_freq
        } else {
            target
        };
        self.pitch_env = 1.0;
        self.timbre_env = 1.0;
        self.stage = EnvStage::Attack;
        self.env2_stage = EnvStage::Attack;
        self.env3 = 1.0;
        if params.voice.lfo1_retrigger {
            self.lfo1_phase = 0.0;
            self.lfo1_sh = self.next_noise();
        }
        if params.voice.lfo2_retrigger {
            self.lfo2_phase = 0.0;
            self.lfo2_sh = self.next_noise();
        }
        if self.env < 0.001 {
            self.phase = [0.0, 0.21, 0.57];
            self.osc_a_uni = [0.0, 0.13, 0.37, 0.59, 0.71, 0.83, 0.91];
            self.osc_b_uni = [0.05, 0.19, 0.41, 0.63, 0.77, 0.89, 0.97];
            self.sub_phase = 0.0;
            self.mod_phase = 0.0;
            self.env2 = 0.0;
            self.cut_smooth = 70.0 * (250.0_f32).powf(params.tone.clamp(0.0, 1.0));
            self.svf_l.reset();
            self.svf_r.reset();
            self.svf2_l.reset();
            self.svf2_r.reset();
            for f in &mut self.formants {
                f.reset();
            }
        }
        if params.engine == SynthEngineKind::Pluck {
            self.init_pluck(target, params, sr);
        }
    }

    fn init_pluck(&mut self, hz: f32, params: &SynthParams, sr: f32) {
        let len = ((sr / hz.max(24.0)).round() as usize).clamp(16, PLUCK_BUF - 2);
        self.pluck_len = len;
        self.pluck_idx = 0;
        self.pluck_lp = 0.0;
        let pick_pos = (0.12 + params.shape * 0.42).clamp(0.08, 0.5);
        let comb_offset = ((len as f32 * pick_pos).round() as usize).clamp(1, len - 1);
        let vel_bright = 0.35 + 0.65 * (0.4 + 0.6 * self.velocity * params.touch);
        let mut lp = 0.0_f32;
        for i in 0..len {
            let n = self.next_noise();
            lp += (n - lp) * vel_bright;
            self.pluck_buf[i] = lp;
        }
        for i in (comb_offset..len).rev() {
            self.pluck_buf[i] -= 0.55 * self.pluck_buf[i - comb_offset];
        }
        let mean = self.pluck_buf[..len].iter().sum::<f32>() / len as f32;
        for s in &mut self.pluck_buf[..len] {
            *s = (*s - mean).clamp(-1.0, 1.0);
        }
    }
}

/// Motor de Espacio estéreo pre-asignado (4 comb + 2 allpass por canal, 0 alloc en RT).
struct SpaceUnit {
    comb_l: [Vec<f32>; 4],
    comb_r: [Vec<f32>; 4],
    comb_idx: [usize; 4],
    comb_lp_l: [f32; 4],
    comb_lp_r: [f32; 4],
    ap_l: [Vec<f32>; 2],
    ap_r: [Vec<f32>; 2],
    ap_idx: [usize; 2],
    air_l: f32,
    air_r: f32,
}

impl SpaceUnit {
    fn new(sample_rate: u32) -> Self {
        let scale = (sample_rate as f32 / 48_000.0).max(0.5);
        let sz = |base: usize| ((base as f32 * scale).round() as usize).max(32);
        let comb_base = [1217, 1429, 1699, 1949];
        let ap_base = [241, 563];
        Self {
            comb_l: [
                vec![0.0; sz(comb_base[0])],
                vec![0.0; sz(comb_base[1])],
                vec![0.0; sz(comb_base[2])],
                vec![0.0; sz(comb_base[3])],
            ],
            comb_r: [
                vec![0.0; sz(comb_base[0] + 29)],
                vec![0.0; sz(comb_base[1] + 37)],
                vec![0.0; sz(comb_base[2] + 43)],
                vec![0.0; sz(comb_base[3] + 53)],
            ],
            comb_idx: [0; 4],
            comb_lp_l: [0.0; 4],
            comb_lp_r: [0.0; 4],
            ap_l: [vec![0.0; sz(ap_base[0])], vec![0.0; sz(ap_base[1])]],
            ap_r: [vec![0.0; sz(ap_base[0] + 19)], vec![0.0; sz(ap_base[1] + 23)]],
            ap_idx: [0; 2],
            air_l: 0.0,
            air_r: 0.0,
        }
    }

    fn reset(&mut self) {
        for v in self.comb_l.iter_mut().chain(&mut self.comb_r) {
            v.fill(0.0);
        }
        for v in self.ap_l.iter_mut().chain(&mut self.ap_r) {
            v.fill(0.0);
        }
        self.comb_lp_l = [0.0; 4];
        self.comb_lp_r = [0.0; 4];
        self.air_l = 0.0;
        self.air_r = 0.0;
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, room: RoomKind, distance: f32, sr: f32) -> (f32, f32) {
        let (fb_base, damp, wet_max) = match room {
            RoomKind::Close => (0.36, 0.58, 0.24),
            RoomKind::Wood => (0.64, 0.50, 0.42),
            RoomKind::Hall => (0.79, 0.36, 0.58),
            RoomKind::Open => (0.87, 0.24, 0.72),
        };
        let fb = (fb_base + distance * 0.09).min(0.90);
        let mono_in = 0.5 * (in_l + in_r);

        let mut acc_l = 0.0_f32;
        let mut acc_r = 0.0_f32;
        for i in 0..4 {
            let il = self.comb_idx[i] % self.comb_l[i].len();
            let ir = self.comb_idx[i] % self.comb_r[i].len();
            let yl = self.comb_l[i][il];
            let yr = self.comb_r[i][ir];
            self.comb_lp_l[i] = yl * (1.0 - damp) + self.comb_lp_l[i] * damp;
            self.comb_lp_r[i] = yr * (1.0 - damp) + self.comb_lp_r[i] * damp;
            // Protección suave en realimentación: evita que acordes densos saturen la sala.
            self.comb_l[i][il] =
                ((mono_in * 0.45 + in_l * 0.20) + self.comb_lp_l[i] * fb).clamp(-2.5, 2.5);
            self.comb_r[i][ir] =
                ((mono_in * 0.45 + in_r * 0.20) + self.comb_lp_r[i] * fb).clamp(-2.5, 2.5);
            self.comb_idx[i] = self.comb_idx[i].wrapping_add(1);
            acc_l += yl;
            acc_r += yr;
        }
        let mut wl = acc_l * 0.25;
        let mut wr = acc_r * 0.25;

        for i in 0..2 {
            let il = self.ap_idx[i] % self.ap_l[i].len();
            let ir = self.ap_idx[i] % self.ap_r[i].len();
            let dl = self.ap_l[i][il];
            let dr = self.ap_r[i][ir];
            let g = 0.5;
            self.ap_l[i][il] = wl + dl * g;
            self.ap_r[i][ir] = wr + dr * g;
            wl = dl - wl * g;
            wr = dr - wr * g;
            self.ap_idx[i] = self.ap_idx[i].wrapping_add(1);
        }

        let air_hz = (18_000.0 * (1.0 - distance * 0.72).powi(2)).max(1100.0);
        let air_coeff = (TAU * air_hz / sr).clamp(0.02, 0.99);
        self.air_l += (in_l - self.air_l) * air_coeff;
        self.air_r += (in_r - self.air_r) * air_coeff;

        let dry_l = in_l * (1.0 - distance * 0.45) + self.air_l * (distance * 0.45);
        let dry_r = in_r * (1.0 - distance * 0.45) + self.air_r * (distance * 0.45);

        let wet = (0.04 + distance * wet_max).clamp(0.0, 0.78);
        let dry = (1.0 - distance * 0.42).clamp(0.35, 1.0);
        (dry_l * dry + wl * wet, dry_r * dry + wr * wet)
    }
}

pub(crate) struct NativeSynthEngine {
    params: SynthParams,
    sample_rate: u32,
    wt: WavetableBank,
    voices: Vec<Voice>,
    chorus: ChorusUnit,
    space: SpaceUnit,
    dc_x_l: f32,
    dc_y_l: f32,
    dc_x_r: f32,
    dc_y_r: f32,
    age_counter: u64,
    last_freq: f32,
    sustain_pedal: bool,
    sustained_notes: [bool; 128],
    pitch_bend_ratio: f32,
    mod_wheel: f32,
    master_volume: f32,
    bpm: f32,
}

impl NativeSynthEngine {
    pub fn new(mut params: SynthParams, sample_rate: u32) -> Self {
        params.clamp_all();
        let sr = sample_rate.max(8_000);
        let voices = (0..MAX_VOICES)
            .map(|i| Voice::new((i as u32 + 1) * 19937))
            .collect();
        Self {
            params,
            sample_rate: sr,
            wt: WavetableBank::new(),
            voices,
            chorus: ChorusUnit::new(),
            space: SpaceUnit::new(sr),
            dc_x_l: 0.0,
            dc_y_l: 0.0,
            dc_x_r: 0.0,
            dc_y_r: 0.0,
            age_counter: 0,
            last_freq: 0.0,
            sustain_pedal: false,
            sustained_notes: [false; 128],
            pitch_bend_ratio: 1.0,
            mod_wheel: 0.0,
            master_volume: 1.0,
            bpm: 120.0,
        }
    }

    pub fn control(&mut self, control_id: u8, value: f32) {
        let v = value.clamp(0.0, 1.0);
        match control_id {
            cc::ENGINE => self.params.engine = SynthEngineKind::from_u8(value.round() as u8),
            cc::ROOM => self.params.room = RoomKind::from_u8(value.round() as u8),
            cc::DISTANCE => self.params.distance = v,
            cc::SHAPE => self.params.shape = v,
            cc::TONE => self.params.tone = v,
            cc::BODY => self.params.body = v,
            cc::MOTION => self.params.motion = v,
            cc::MODE => self.params.mode = EnvelopeMode::from_u8(value.round() as u8),
            cc::TIME => self.params.time = v,
            cc::TOUCH => self.params.touch = v,
            cc::GLIDE => self.params.glide = v,
            cc::GAIN => self.params.gain = v,
            _ => {}
        }
    }

    pub fn control_param(&mut self, id: u16, value: f32) {
        self.params.set_param(id, value);
    }

    pub fn set_custom_wavetable(&mut self, slot: u8, table: Option<Arc<CustomWavetable>>) {
        match slot {
            0 => self.params.custom_wt_a = table,
            1 => self.params.custom_wt_b = table,
            _ => {}
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 2.0);
    }

    pub fn set_tempo(&mut self, bpm: f32) {
        if bpm.is_finite() && bpm > 10.0 {
            self.bpm = bpm.clamp(20.0, 300.0);
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8, pan: f32) {
        if velocity == 0 {
            self.note_off(note);
            return;
        }
        self.age_counter = self.age_counter.wrapping_add(1);
        let mono_mode = self.params.engine == SynthEngineKind::Sub808
            || (self.params.glide >= 0.45
                && matches!(
                    self.params.engine,
                    SynthEngineKind::FoldAcid | SynthEngineKind::Wavetable
                ));

        let slot = if mono_mode {
            0
        } else if let Some(idx) = self.voices.iter().position(|v| !v.active) {
            idx
        } else if let Some(idx) = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.stage == EnvStage::Release)
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
        {
            idx
        } else {
            self.voices
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.age)
                .map(|(i, _)| i)
                .unwrap_or(0)
        };

        let sr = self.sample_rate as f32;
        let start_freq = if self.last_freq > 15.0 {
            self.last_freq
        } else {
            midi_to_hz(note)
        };
        let target = midi_to_hz(note);
        self.last_freq = target;
        self.sustained_notes[usize::from(note.min(127))] = false;
        self.voices[slot].trigger(
            note,
            velocity,
            pan,
            start_freq,
            self.age_counter,
            &self.params,
            sr,
        );
    }

    pub fn note_off(&mut self, note: u8) {
        if self.sustain_pedal {
            self.sustained_notes[usize::from(note.min(127))] = true;
            return;
        }
        for v in &mut self.voices {
            if v.active && v.note == note {
                if v.stage != EnvStage::Release
                    && (self.params.voice.env1_override || self.params.mode != EnvelopeMode::Hit)
                {
                    v.stage = EnvStage::Release;
                }
                if v.env2_stage != EnvStage::Release {
                    v.env2_stage = EnvStage::Release;
                }
            }
        }
    }

    pub fn all_off(&mut self) {
        self.sustain_pedal = false;
        self.sustained_notes.fill(false);
        for v in &mut self.voices {
            if v.active {
                v.stage = EnvStage::Release;
                v.env2_stage = EnvStage::Release;
            }
        }
    }

    pub fn panic(&mut self) {
        self.sustain_pedal = false;
        self.sustained_notes.fill(false);
        for v in &mut self.voices {
            v.active = false;
            v.stage = EnvStage::Off;
            v.env = 0.0;
            v.env2 = 0.0;
            v.env2_stage = EnvStage::Off;
        }
        self.chorus.reset();
        self.space.reset();
    }

    pub fn midi(&mut self, command: u8, data1: u8, data2: u8) {
        match command & 0xf0 {
            0x90 if data2 > 0 => self.note_on(data1, data2, 0.0),
            0x80 | 0x90 => self.note_off(data1),
            0xb0 => match data1 {
                1 => self.mod_wheel = f32::from(data2) / 127.0,
                64 => {
                    let down = data2 >= 64;
                    self.sustain_pedal = down;
                    if !down {
                        for n in 0..128u8 {
                            if self.sustained_notes[usize::from(n)] {
                                self.sustained_notes[usize::from(n)] = false;
                                self.note_off(n);
                            }
                        }
                    }
                }
                120 | 123 => self.all_off(),
                // CC 102..=113: control Stage en vivo durante Play del Arrangement.
                102 => self.control(cc::ENGINE, f32::from(data2.min(7))),
                103 => self.control(cc::ROOM, f32::from(data2.min(3))),
                104 => self.control(cc::DISTANCE, f32::from(data2) / 127.0),
                105 => self.control(cc::SHAPE, f32::from(data2) / 127.0),
                106 => self.control(cc::TONE, f32::from(data2) / 127.0),
                107 => self.control(cc::BODY, f32::from(data2) / 127.0),
                108 => self.control(cc::MOTION, f32::from(data2) / 127.0),
                109 => self.control(cc::MODE, f32::from(data2.min(2))),
                110 => self.control(cc::TIME, f32::from(data2) / 127.0),
                111 => self.control(cc::TOUCH, f32::from(data2) / 127.0),
                112 => self.control(cc::GLIDE, f32::from(data2) / 127.0),
                113 => self.control(cc::GAIN, f32::from(data2) / 127.0),
                _ => {}
            },
            0xe0 => {
                let raw = (i32::from(data2) << 7) | i32::from(data1);
                let semis = ((raw - 8192) as f32 / 8192.0) * 2.0;
                self.pitch_bend_ratio = 2.0_f32.powf(semis / 12.0);
            }
            _ => {}
        }
    }

    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let sr = self.sample_rate as f32;
        let inv_sr = 1.0 / sr;
        let params = &self.params;
        let vp = &params.voice;

        // Tiempos de ENV 1 (o Mode+Time si env1_override == false).
        let time = params.time;
        let (base_attack_s, decay_s, sustain_lvl, release_s, is_hit) = if vp.env1_override {
            (
                0.0015 + vp.env1_attack * vp.env1_attack * 2.4,
                0.02 + vp.env1_decay * vp.env1_decay * 3.5,
                vp.env1_sustain,
                0.02 + vp.env1_release * vp.env1_release * 4.0,
                false,
            )
        } else {
            match params.mode {
                EnvelopeMode::Hit => (0.002, 0.08 + time * time * 2.2, 0.0, 0.04 + time * 0.45, true),
                EnvelopeMode::Hold => (
                    0.004 + time * 0.035,
                    0.14 + time * 0.9,
                    0.76,
                    0.08 + time * 1.1,
                    false,
                ),
                EnvelopeMode::Swell => (
                    0.12 + time * time * 1.6,
                    0.30 + time * 1.2,
                    0.85,
                    0.28 + time * time * 2.6,
                    false,
                ),
            }
        };

        let env2_atk_inc = inv_sr / (0.0015 + vp.env2_attack * vp.env2_attack * 2.2);
        let env2_dec_mul = (-inv_sr / (0.02 + vp.env2_decay * vp.env2_decay * 3.0)).exp();
        let env2_rel_mul = (-inv_sr / (0.02 + vp.env2_release * vp.env2_release * 3.0)).exp();
        let env3_dec_mul = (-inv_sr / (0.012 + vp.env3_decay * vp.env3_decay * 0.9)).exp();

        let glide_coeff = if params.glide <= 0.02 {
            1.0
        } else {
            let glide_s = 0.015 + params.glide * params.glide * 0.38;
            1.0 - (-inv_sr / glide_s).exp()
        };

        let lfo_hz = (self.bpm / 60.0) * (0.25 + params.motion * 1.75);
        let lfo_inc = lfo_hz * inv_sr;
        let base_out_gain = params.gain * self.master_volume * 0.36;
        let dc_r = 1.0 - (TAU * 18.0 * inv_sr).clamp(0.0005, 0.02);

        for i in 0..frames {
            let mut sum_l = 0.0_f32;
            let mut sum_r = 0.0_f32;
            let mut bus_chorus_mod = 0.0_f32;
            let mut bus_dist_mod = 0.0_f32;
            let mut bus_dist_space_mod = 0.0_f32;
            let mut bus_out_mod = 0.0_f32;
            let mut bus_mod_voices = 0_u32;

            for v in &mut self.voices {
                if !v.active {
                    continue;
                }

                // ── ENV 1 (Amp) ──
                let vel_eff = 1.0 - params.touch + params.touch * v.velocity;
                let atk_s = (base_attack_s * (1.25 - 0.65 * vel_eff)).max(0.0015);
                let atk_inc = inv_sr / atk_s;
                let dec_mul = (-inv_sr / decay_s.max(0.02)).exp();
                let rel_mul = (-inv_sr / release_s.max(0.02)).exp();

                match v.stage {
                    EnvStage::Off => {
                        v.active = false;
                        continue;
                    }
                    EnvStage::Attack => {
                        v.env += atk_inc;
                        if v.env >= 1.0 {
                            v.env = 1.0;
                            v.stage = EnvStage::Decay;
                        }
                    }
                    EnvStage::Decay => {
                        v.env = sustain_lvl + (v.env - sustain_lvl) * dec_mul;
                        if is_hit && v.env < 0.0008 {
                            v.env = 0.0;
                            v.stage = EnvStage::Off;
                            v.active = false;
                            continue;
                        } else if !is_hit && (v.env - sustain_lvl).abs() < 0.001 {
                            v.env = sustain_lvl;
                            v.stage = EnvStage::Sustain;
                        }
                    }
                    EnvStage::Sustain => {}
                    EnvStage::Release => {
                        v.env *= rel_mul;
                        if v.env < 0.0005 {
                            v.env = 0.0;
                            v.stage = EnvStage::Off;
                            v.active = false;
                            continue;
                        }
                    }
                }

                // ── ENV 2 (Mod) ──
                match v.env2_stage {
                    EnvStage::Off => {}
                    EnvStage::Attack => {
                        v.env2 += env2_atk_inc;
                        if v.env2 >= 1.0 {
                            v.env2 = 1.0;
                            v.env2_stage = EnvStage::Decay;
                        }
                    }
                    EnvStage::Decay => {
                        v.env2 = vp.env2_sustain + (v.env2 - vp.env2_sustain) * env2_dec_mul;
                        if (v.env2 - vp.env2_sustain).abs() < 0.001 {
                            v.env2 = vp.env2_sustain;
                            v.env2_stage = EnvStage::Sustain;
                        }
                    }
                    EnvStage::Sustain => {}
                    EnvStage::Release => {
                        v.env2 *= env2_rel_mul;
                        if v.env2 < 0.0005 {
                            v.env2 = 0.0;
                            v.env2_stage = EnvStage::Off;
                        }
                    }
                }

                // ── ENV 3 (Pitch / Punch) y envolventes rápidas ──
                v.env3 *= env3_dec_mul;
                v.pitch_env *= (-inv_sr / 0.042).exp();
                v.timbre_env *= (-inv_sr / (0.10 + time * 0.85)).exp();
                v.freq += (v.target_freq - v.freq) * glide_coeff;

                // ── LFOs (organic motion + LFO 1 + LFO 2) ──
                v.lfo_phase = (v.lfo_phase + lfo_inc).fract();
                let lfo = (v.lfo_phase * TAU).sin();

                let lfo1_val = eval_lfo(vp.lfo1_shape, v.lfo1_phase, v.lfo1_sh);
                let lfo2_val = eval_lfo(vp.lfo2_shape, v.lfo2_phase, v.lfo2_sh);

                // ── Evaluación de la Mod Matrix (12 ranuras) ──
                let key_norm = ((f32::from(v.note) - 60.0) / 36.0).clamp(-1.0, 1.0);
                let mut m = ModAccum::default();
                for slot in &vp.matrix {
                    if slot.source == ModSource::None
                        || slot.target == ModTarget::None
                        || slot.amount.abs() < 1e-4
                    {
                        continue;
                    }
                    let src = match slot.source {
                        ModSource::None => 0.0,
                        ModSource::Env1 => v.env,
                        ModSource::Env2 => v.env2,
                        ModSource::Env3 => v.env3,
                        ModSource::Lfo1 => lfo1_val,
                        ModSource::Lfo2 => lfo2_val,
                        ModSource::Velocity => v.velocity,
                        ModSource::Keytrack => key_norm,
                        ModSource::ModWheel => self.mod_wheel,
                        ModSource::Random => v.voice_rand,
                        ModSource::Distance => params.distance,
                        ModSource::Tone => params.tone,
                        ModSource::Shape => params.shape,
                        ModSource::Body => params.body,
                        ModSource::Motion => params.motion,
                    };
                    let delta = src * slot.amount;
                    match slot.target {
                        ModTarget::None => {}
                        ModTarget::OscAPos => m.osc_a_pos += delta,
                        ModTarget::OscAWarp => m.osc_a_warp += delta,
                        ModTarget::OscAPitch => m.osc_a_pitch += delta,
                        ModTarget::OscADetune => m.osc_a_detune += delta,
                        ModTarget::OscALevel => m.osc_a_level += delta,
                        ModTarget::OscBPos => m.osc_b_pos += delta,
                        ModTarget::OscBPitch => m.osc_b_pitch += delta,
                        ModTarget::OscBLevel => m.osc_b_level += delta,
                        ModTarget::SubLevel => m.sub_level += delta,
                        ModTarget::NoiseLevel => m.noise_level += delta,
                        ModTarget::FilterCutoff => m.filter_cutoff += delta,
                        ModTarget::FilterRes => m.filter_res += delta,
                        ModTarget::FilterDrive => m.filter_drive += delta,
                        ModTarget::ChorusMix => m.chorus_mix += delta,
                        ModTarget::DistDrive => m.dist_drive += delta,
                        ModTarget::Distance => m.distance += delta,
                        ModTarget::Lfo1Rate => m.lfo1_rate += delta,
                        ModTarget::Lfo2Rate => m.lfo2_rate += delta,
                        ModTarget::Pan => m.pan += delta,
                        ModTarget::OutGain => m.out_gain += delta,
                    }
                }

                // Avanzar LFO 1 y LFO 2 (con modulación de rate posible desde la Matrix).
                let lfo1_hz = lfo_rate_hz((vp.lfo1_rate + m.lfo1_rate).clamp(0.0, 1.0), vp.lfo1_sync, self.bpm);
                let next_p1 = v.lfo1_phase + lfo1_hz * inv_sr;
                if next_p1 >= 1.0 {
                    v.lfo1_sh = v.next_noise();
                }
                v.lfo1_phase = next_p1.fract();

                let lfo2_hz = lfo_rate_hz((vp.lfo2_rate + m.lfo2_rate).clamp(0.0, 1.0), vp.lfo2_sync, self.bpm);
                let next_p2 = v.lfo2_phase + lfo2_hz * inv_sr;
                if next_p2 >= 1.0 {
                    v.lfo2_sh = v.next_noise();
                }
                v.lfo2_phase = next_p2.fract();

                bus_chorus_mod += m.chorus_mix;
                bus_dist_mod += m.dist_drive;
                bus_dist_space_mod += m.distance;
                bus_out_mod += m.out_gain;
                bus_mod_voices += 1;

                // Frecuencias efectivas con pitch bend + drift + semitonos + Matrix pitch (±24 st).
                let drift = 1.0 + lfo * params.motion * 0.0055;
                let base_hz = (v.freq * self.pitch_bend_ratio * drift).clamp(20.0, sr * 0.42);
                let osc_a_semi = f32::from(vp.osc_a_semi) + m.osc_a_pitch * 24.0;
                let hz = (base_hz * 2.0_f32.powf(osc_a_semi / 12.0)).clamp(20.0, sr * 0.42);

                let shape_eff = (params.shape + m.osc_a_pos).clamp(0.0, 1.0);
                let osc_a_lvl = (vp.osc_a_level + m.osc_a_level).clamp(0.0, 1.5);
                let osc_b_lvl = (vp.osc_b_level + m.osc_b_level).clamp(0.0, 1.5);
                let need_osc_b = osc_b_lvl > 0.001
                    || (params.engine == SynthEngineKind::Wavetable
                        && matches!(vp.osc_a_warp_mode, WarpMode::FmFromB | WarpMode::RingMod));

                // ── OSC B (Wavetable band-limited Hi-Fi + fuente FM/RM para OSC A) ──
                let (mut ob_l, mut ob_r, mut ob_mono) = (0.0_f32, 0.0_f32, 0.0_f32);
                if need_osc_b {
                    let b_semi =
                        f32::from(vp.osc_b_semi) + vp.osc_b_fine * 0.5 + m.osc_b_pitch * 24.0;
                    let hz_b = (base_hz * 2.0_f32.powf(b_semi / 12.0)).clamp(16.0, sr * 0.44);
                    let pos_b = (vp.osc_b_pos + m.osc_b_pos).clamp(0.0, 1.0);
                    let uni_b = usize::from(vp.osc_b_unison.clamp(1, MAX_UNISON as u8));
                    let det_b = vp.osc_b_detune * 0.016;
                    let norm_b = 1.0 / (uni_b as f32).powf(0.65);
                    for u in 0..uni_b {
                        let frac = if uni_b == 1 {
                            0.0
                        } else {
                            (u as f32 / (uni_b - 1) as f32) * 2.0 - 1.0
                        };
                        let f_u = hz_b * (1.0 + frac * det_b);
                        v.osc_b_uni[u] = (v.osc_b_uni[u] + f_u * inv_sr).fract();
                        let s = if let Some(custom) = &params.custom_wt_b {
                            WavetableBank::sample_custom(custom, pos_b, v.osc_b_uni[u], f_u)
                        } else {
                            self.wt.sample(vp.osc_b_table, pos_b, v.osc_b_uni[u], f_u)
                        };
                        if u == 0 {
                            ob_mono = s;
                        }
                        let pan_u = frac * 0.65;
                        ob_l += s * (1.0 - pan_u).min(1.0);
                        ob_r += s * (1.0 + pan_u).min(1.0);
                    }
                    ob_l *= norm_b;
                    ob_r *= norm_b;
                }

                // ── OSC A (7 motores originales + 8º motor Wavetable estilo Vital) ──
                let (mut sl, mut sr_ch) = match params.engine {
                    SynthEngineKind::Sub808 => {
                        let drop_semi = shape_eff * 18.0 * v.pitch_env;
                        let f = hz * 2.0_f32.powf(drop_semi / 12.0);
                        v.phase[0] = (v.phase[0] + f * inv_sr).fract();
                        let s0 = (v.phase[0] * TAU).sin();
                        let s2 = ((v.phase[0] * 2.0).fract() * TAU).sin() * 0.28 * shape_eff;
                        let grit = 1.0 + shape_eff * (2.2 + 2.5 * vel_eff);
                        let driven = ((s0 + s2) * grit).tanh();
                        let sub_w = params.body * 0.14 * lfo;
                        (driven * (1.0 - sub_w), driven * (1.0 + sub_w))
                    }
                    SynthEngineKind::Analog => {
                        let det = 0.002 + (params.body + m.osc_a_detune).clamp(0.0, 1.0) * 0.011;
                        let freqs = [hz, hz * (1.0 - det), hz * (1.0 + det)];
                        let mut outs = [0.0_f32; 3];
                        for k in 0..3 {
                            let dt = freqs[k] * inv_sr;
                            v.phase[k] = (v.phase[k] + dt).fract();
                            let saw = polyblep_saw(v.phase[k], dt);
                            let tri = 4.0 * (v.phase[k] - 0.5).abs() - 1.0;
                            outs[k] = tri * (1.0 - shape_eff) + saw * shape_eff;
                        }
                        v.sub_phase = (v.sub_phase + (hz * 0.5) * inv_sr).fract();
                        let sub = (v.sub_phase * TAU).sin() * (0.28 * (1.0 - params.tone * 0.4));
                        let spread = 0.35 + params.body * 0.65;
                        let l = (outs[0] * 0.55 + outs[1] * spread + outs[2] * (1.0 - spread) + sub)
                            * 0.62;
                        let r = (outs[0] * 0.55 + outs[2] * spread + outs[1] * (1.0 - spread) + sub)
                            * 0.62;
                        (l, r)
                    }
                    SynthEngineKind::Pluck => {
                        let idx = v.pluck_idx;
                        let next = (idx + 1) % v.pluck_len;
                        let cur = v.pluck_buf[idx];
                        let nxt = v.pluck_buf[next];
                        let avg = 0.5 * (cur + nxt);
                        let damp_a = 0.25 + params.tone * 0.68;
                        v.pluck_lp += (avg - v.pluck_lp) * damp_a;
                        let decay_fb = 0.968 + params.time * 0.029;
                        v.pluck_buf[idx] = v.pluck_lp * decay_fb;
                        v.pluck_idx = next;
                        let body_s = v.pluck_lp * (1.1 + params.body * 0.35);
                        let micro_st = (v.pluck_buf[(idx + 7) % v.pluck_len]) * params.body * 0.22;
                        (body_s + micro_st, body_s - micro_st)
                    }
                    SynthEngineKind::FmBell => {
                        const RATIOS: [f32; 5] = [1.0, 2.0, 3.0, 3.5, 5.0];
                        let rpos = shape_eff * 4.0;
                        let ridx = (rpos.floor() as usize).min(3);
                        let rfrac = rpos - ridx as f32;
                        let ratio = RATIOS[ridx] + (RATIOS[ridx + 1] - RATIOS[ridx]) * rfrac;
                        v.mod_phase = (v.mod_phase + (hz * ratio) * inv_sr).fract();
                        let fm_index = (0.35 + shape_eff * 4.2) * (0.25 + 0.75 * v.timbre_env)
                            * (0.4 + 0.6 * vel_eff);
                        let mod_sig = (v.mod_phase * TAU).sin() * fm_index;
                        v.phase[0] = (v.phase[0] + hz * inv_sr).fract();
                        let det = 1.0 + params.body * 0.004;
                        v.phase[1] = (v.phase[1] + hz * det * inv_sr).fract();
                        let cl = (v.phase[0] * TAU + mod_sig).sin();
                        let cr = (v.phase[1] * TAU + mod_sig * 0.94).sin();
                        (cl * 0.85, cr * 0.85)
                    }
                    SynthEngineKind::Vocal => {
                        let dt0 = hz * inv_sr;
                        let dt1 = (hz * (1.0 + params.body * 0.005)) * inv_sr;
                        v.phase[0] = (v.phase[0] + dt0).fract();
                        v.phase[1] = (v.phase[1] + dt1).fract();
                        let glot_l = polyblep_saw(v.phase[0], dt0);
                        let glot_r = polyblep_saw(v.phase[1], dt1);
                        let breath = v.next_noise() * (0.04 + params.motion * 0.07);
                        let src_l = glot_l * 0.75 + breath;
                        let src_r = glot_r * 0.75 + breath;
                        let (f1, f2, f3) = vowel_formants(shape_eff);
                        let (_, b1) = v.formants[0].process(src_l, f1, 5.2, sr);
                        let (_, b2) = v.formants[1].process(src_r, f2, 6.2, sr);
                        let (_, b3) = v.formants[2].process(0.5 * (src_l + src_r), f3, 7.0, sr);
                        let l = (b1 * 1.15 + b2 * 0.72 + b3 * 0.35) * 0.78;
                        let r = (b1 * 0.95 + b2 * 0.92 + b3 * 0.42) * 0.78;
                        (l, r)
                    }
                    SynthEngineKind::FoldAcid => {
                        let dt = hz * inv_sr;
                        v.phase[0] = (v.phase[0] + dt).fract();
                        v.phase[1] = (v.phase[1] + (hz * (1.0 + params.body * 0.007)) * inv_sr).fract();
                        let raw_l = polyblep_saw(v.phase[0], dt);
                        let raw_r = polyblep_saw(v.phase[1], dt);
                        let fold = 1.0 + shape_eff * (2.6 + 2.4 * vel_eff);
                        let fl = (raw_l * fold).sin() * 0.65 + (raw_l * (1.0 + shape_eff)).tanh() * 0.35;
                        let fr = (raw_r * fold).sin() * 0.65 + (raw_r * (1.0 + shape_eff)).tanh() * 0.35;
                        (fl * 0.85, fr * 0.85)
                    }
                    SynthEngineKind::Texture => {
                        let d1 = hz * inv_sr;
                        let d2 = (hz * 1.5 * (1.0 + lfo * 0.004)) * inv_sr;
                        let d3 = (hz * 2.0 * (1.0 - lfo * 0.004)) * inv_sr;
                        v.phase[0] = (v.phase[0] + d1).fract();
                        v.phase[1] = (v.phase[1] + d2).fract();
                        v.phase[2] = (v.phase[2] + d3).fract();
                        let tones_l = 0.50 * (v.phase[0] * TAU).sin()
                            + 0.30 * (v.phase[1] * TAU).sin()
                            + 0.20 * (v.phase[2] * TAU).sin();
                        let tones_r = 0.50 * (v.phase[0] * TAU + 0.4).sin()
                            + 0.30 * (v.phase[2] * TAU).sin()
                            + 0.20 * (v.phase[1] * TAU + 0.9).sin();
                        let n_l = v.next_noise();
                        let n_r = v.next_noise();
                        let (_, air_l) =
                            v.formants[0].process(n_l, (hz * 2.0).clamp(240.0, 6000.0), 2.2, sr);
                        let (_, air_r) =
                            v.formants[1].process(n_r, (hz * 3.0).clamp(320.0, 7500.0), 2.5, sr);
                        let air_mix = 0.12 + shape_eff * 0.45;
                        (
                            tones_l * (1.0 - air_mix * 0.4) + air_l * air_mix * 1.5,
                            tones_r * (1.0 - air_mix * 0.4) + air_r * air_mix * 1.5,
                        )
                    }
                    SynthEngineKind::Wavetable => {
                        let uni_a = usize::from(vp.osc_a_unison.clamp(1, MAX_UNISON as u8));
                        let det_a = (vp.osc_a_detune + m.osc_a_detune).clamp(0.0, 1.0) * 0.016;
                        let spread_a = vp.osc_a_spread.clamp(0.0, 1.0);
                        let warp_amt = (vp.osc_a_warp + m.osc_a_warp).clamp(0.0, 1.0);
                        let norm_a = 1.0 / (uni_a as f32).sqrt();
                        let mut al = 0.0_f32;
                        let mut ar = 0.0_f32;
                        for u in 0..uni_a {
                            let frac = if uni_a == 1 {
                                0.0
                            } else {
                                (u as f32 / (uni_a - 1) as f32) * 2.0 - 1.0
                            };
                            let f_u = hz * (1.0 + frac * det_a);
                            v.osc_a_uni[u] = (v.osc_a_uni[u] + f_u * inv_sr).fract();
                            let ph = v.osc_a_uni[u];
                            let (warped_ph, mip_hz) = match vp.osc_a_warp_mode {
                                WarpMode::Off | WarpMode::Fold | WarpMode::RingMod => (ph, f_u),
                                WarpMode::Sync => {
                                    let mult = 1.0 + warp_amt * 3.5;
                                    // Ventana suave en el reinicio del ciclo para evitar clicks de hard sync.
                                    ((ph * mult).fract(), f_u * mult)
                                }
                                WarpMode::Bend => {
                                    let exp = 2.0_f32.powf((warp_amt - 0.5) * 2.2);
                                    (ph.powf(exp), f_u * (1.0 + (warp_amt - 0.5).abs() * 1.5))
                                }
                                WarpMode::FmFromB => (
                                    (ph + ob_mono * warp_amt * 0.45).rem_euclid(1.0),
                                    f_u * (1.0 + warp_amt * 2.0),
                                ),
                            };
                            let mut s = if let Some(custom) = &params.custom_wt_a {
                                WavetableBank::sample_custom(custom, shape_eff, warped_ph, mip_hz)
                            } else {
                                self.wt.sample(vp.osc_a_table, shape_eff, warped_ph, mip_hz)
                            };
                            match vp.osc_a_warp_mode {
                                WarpMode::Fold => {
                                    s = (s * (1.0 + warp_amt * 2.8)).sin();
                                }
                                WarpMode::RingMod => {
                                    s = s * (1.0 - warp_amt * 0.8) + (s * ob_mono) * (warp_amt * 0.8);
                                }
                                _ => {}
                            }
                            let pan_u = frac * spread_a;
                            al += s * (1.0 - pan_u).min(1.0);
                            ar += s * (1.0 + pan_u).min(1.0);
                        }
                        (al * norm_a * 0.78, ar * norm_a * 0.78)
                    }
                };

                sl *= osc_a_lvl;
                sr_ch *= osc_a_lvl;

                // Sumar OSC B audible con headroom limpio
                if osc_b_lvl > 0.001 {
                    sl += ob_l * osc_b_lvl * 0.48;
                    sr_ch += ob_r * osc_b_lvl * 0.48;
                }

                // Sumar SUB dedicado (-1 o -2 oct)
                let sub_lvl = (vp.sub_level + m.sub_level).clamp(0.0, 1.5);
                if sub_lvl > 0.001 {
                    let sub_div = if vp.sub_octave == 0 { 0.5 } else { 0.25 };
                    v.sub_phase = (v.sub_phase + (base_hz * sub_div) * inv_sr).fract();
                    let sub_sig = (v.sub_phase * TAU).sin() * sub_lvl * 0.40;
                    sl += sub_sig;
                    sr_ch += sub_sig;
                }

                // Sumar NOISE / AIR dedicado
                let noise_lvl = (vp.noise_level + m.noise_level).clamp(0.0, 1.5);
                if noise_lvl > 0.001 {
                    let raw_n = v.next_noise();
                    let n_cut = 300.0 * (45.0_f32).powf(vp.noise_color);
                    let n_a = (TAU * n_cut * inv_sr).clamp(0.01, 0.99);
                    v.noise_lp += (raw_n - v.noise_lp) * n_a;
                    let n_sig = if vp.noise_color > 0.72 {
                        raw_n - v.noise_lp * 0.55
                    } else {
                        v.noise_lp
                    };
                    sl += n_sig * noise_lvl * 0.22;
                    sr_ch += n_sig * noise_lvl * 0.22;
                }

                // ── Filtro TPT SVF Hi-Fi (suavizado anti-zipper + compensación de Q) ──
                let pre_drive = (vp.filter_drive + m.filter_drive).clamp(0.0, 1.0);
                if pre_drive > 0.005 {
                    let g = 1.0 + pre_drive * 2.4;
                    let comp = 1.0 / g.sqrt();
                    sl = (sl * g).tanh() * comp;
                    sr_ch = (sr_ch * g).tanh() * comp;
                }

                let tone_eff = (params.tone + m.filter_cutoff * 0.75).clamp(0.0, 1.0);
                let base_cut = 70.0 * (250.0_f32).powf(tone_eff);
                let env_mod = 1.0 + v.timbre_env * (0.28 + 1.6 * vel_eff);
                let lfo_mod = 1.0 + lfo * params.motion * 0.16;
                let key_scale = (hz / 261.63)
                    .powf(vp.filter_keytrack.clamp(0.0, 1.0))
                    .clamp(0.45, 3.0);
                let target_cut = (base_cut * env_mod * lfo_mod * key_scale).clamp(35.0, 18_500.0);
                v.cut_smooth += (target_cut - v.cut_smooth) * 0.08;
                let cutoff = v.cut_smooth;

                let res_eff = (vp.filter_res + m.filter_res).clamp(0.0, 0.90);
                let q = if params.engine == SynthEngineKind::FoldAcid {
                    0.95 + shape_eff * 2.4 + res_eff * 2.6
                } else {
                    0.65 + res_eff * 4.0
                };
                let q_comp = 1.0 / (1.0 + res_eff * 0.55);

                if params.engine != SynthEngineKind::Pluck
                    || vp.filter_kind != FilterKind::Lp12
                    || res_eff > 0.15
                {
                    let (lp_l, bp_l, hp_l, nt_l) =
                        v.svf_l.process_all(sl * q_comp, cutoff, q, sr);
                    let (lp_r, bp_r, hp_r, nt_r) =
                        v.svf_r.process_all(sr_ch * q_comp, cutoff, q, sr);
                    match vp.filter_kind {
                        FilterKind::Lp12 => {
                            sl = lp_l;
                            sr_ch = lp_r;
                        }
                        FilterKind::Lp24 => {
                            let (lp2_l, _, _, _) =
                                v.svf2_l.process_all(lp_l, cutoff, (q * 0.62).max(0.65), sr);
                            let (lp2_r, _, _, _) =
                                v.svf2_r.process_all(lp_r, cutoff, (q * 0.62).max(0.65), sr);
                            sl = lp2_l;
                            sr_ch = lp2_r;
                        }
                        FilterKind::Bp12 => {
                            sl = bp_l * 1.15;
                            sr_ch = bp_r * 1.15;
                        }
                        FilterKind::Hp12 => {
                            sl = hp_l;
                            sr_ch = hp_r;
                        }
                        FilterKind::Notch => {
                            sl = nt_l;
                            sr_ch = nt_r;
                        }
                    }
                }

                // Calor analógico suave sin aplastar dinámicas.
                let drive = 1.0 + vel_eff * (0.10 + params.body * 0.20);
                sl = soft_sat(sl * drive);
                sr_ch = soft_sat(sr_ch * drive);

                let amp = v.env * (0.28 + 0.72 * vel_eff);
                let voice_pan = (v.pan + m.pan).clamp(-1.0, 1.0);
                let pan_l = (1.0 - voice_pan).min(1.0);
                let pan_r = (1.0 + voice_pan).min(1.0);
                sum_l += sl * amp * pan_l;
                sum_r += sr_ch * amp * pan_r;
            }

            // DC blocker 18 Hz antes de FX para que el reverb nunca acumule offset.
            let y_l = sum_l - self.dc_x_l + dc_r * self.dc_y_l;
            self.dc_x_l = sum_l;
            self.dc_y_l = y_l;
            let y_r = sum_r - self.dc_x_r + dc_r * self.dc_y_r;
            self.dc_x_r = sum_r;
            self.dc_y_r = y_r;
            sum_l = y_l;
            sum_r = y_r;

            let inv_v = if bus_mod_voices > 0 {
                1.0 / bus_mod_voices as f32
            } else {
                0.0
            };

            // ── Bus FX 1: Distortion (compensada en ganancia) ──
            let dist_drive = (vp.dist_drive + bus_dist_mod * inv_v).clamp(0.0, 1.0);
            let dist_mix = vp.dist_mix.clamp(0.0, 1.0);
            if dist_mix > 0.001 && dist_drive > 0.001 {
                sum_l = apply_distortion(sum_l, vp.dist_mode, dist_drive, dist_mix);
                sum_r = apply_distortion(sum_r, vp.dist_mode, dist_drive, dist_mix);
            }

            // ── Bus FX 2: Stereo Chorus ──
            let chorus_mix = (vp.chorus_mix + bus_chorus_mod * inv_v).clamp(0.0, 1.0);
            let (ch_l, ch_r) = self
                .chorus
                .process(sum_l, sum_r, chorus_mix, vp.chorus_rate, sr);

            // ── Bus FX 3: Espacio (Room + Distance + absorción de aire) ──
            let dist_eff = (params.distance + bus_dist_space_mod * inv_v).clamp(0.0, 1.0);
            let (out_l, out_r) = self.space.process(ch_l, ch_r, params.room, dist_eff, sr);

            let final_gain = (base_out_gain * (1.0 + bus_out_mod * inv_v * 0.55)).clamp(0.0, 1.4);
            left[i] = soft_limit(out_l * final_gain);
            right[i] = soft_limit(out_r * final_gain);
        }
    }
}

#[inline]
fn eval_lfo(shape: LfoShape, phase: f32, sh: f32) -> f32 {
    match shape {
        LfoShape::Sine => (phase * TAU).sin(),
        LfoShape::Tri => 4.0 * (phase - 0.5).abs() - 1.0,
        LfoShape::SawUp => phase * 2.0 - 1.0,
        LfoShape::SawDown => 1.0 - phase * 2.0,
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::Random => sh,
    }
}

#[inline]
fn lfo_rate_hz(rate: f32, sync: bool, bpm: f32) -> f32 {
    if sync {
        // Divisiones musicales: 4 bars, 2 bars, 1 bar, 1/2, 1/4, 1/8, 1/16, 1/32
        const MULT: [f32; 8] = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
        let idx = ((rate * 7.0).round() as usize).min(7);
        (bpm / 60.0) * MULT[idx]
    } else {
        0.05 * (320.0_f32).powf(rate) // 0.05 Hz .. 16 Hz
    }
}

/// Saturación suave transparente: completamente lineal debajo de 0.72.
#[inline]
fn soft_sat(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.72 {
        x
    } else {
        x.signum() * (0.72 + 0.28 * ((a - 0.72) / 0.28).tanh())
    }
}

/// Limitador de salida Hi-Fi transparente: cero coloración hasta 0.80, techo seguro en 0.96.
#[inline]
fn soft_limit(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.80 {
        x
    } else {
        x.signum() * (0.80 + 0.16 * ((a - 0.80) / 0.16).tanh())
    }
}

#[inline]
fn apply_distortion(x: f32, mode: DistMode, drive: f32, mix: f32) -> f32 {
    let g = 1.0 + drive * 3.5;
    let norm = 1.0 / g.powf(0.55);
    let wet = match mode {
        DistMode::Soft => (x * g).tanh() * norm,
        DistMode::Hard => (x * g).clamp(-0.90, 0.90) * norm,
        DistMode::Fold => (x * (1.0 + drive * 2.5)).sin() * 0.75,
        DistMode::Bitcrush => {
            let steps = (36.0 - drive * 28.0).max(6.0);
            ((x * (1.0 + drive * 1.4)) * steps).round() / steps * 0.85
        }
    };
    x * (1.0 - mix) + wet * mix
}

#[inline]
fn midi_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(note) - 69.0) / 12.0)
}

#[inline]
fn polyblep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

#[inline]
fn polyblep_saw(phase: f32, dt: f32) -> f32 {
    2.0 * phase - 1.0 - polyblep(phase, dt)
}

/// Interpolación continua de formantes vocales A -> E -> I -> O -> U.
#[inline]
fn vowel_formants(shape: f32) -> (f32, f32, f32) {
    const TABLE: [(f32, f32, f32); 5] = [
        (730.0, 1090.0, 2440.0), // A
        (530.0, 1840.0, 2480.0), // E
        (270.0, 2290.0, 3010.0), // I
        (570.0, 840.0, 2410.0),  // O
        (300.0, 870.0, 2240.0),  // U
    ];
    let pos = shape.clamp(0.0, 1.0) * 4.0;
    let idx = (pos.floor() as usize).min(3);
    let frac = pos - idx as f32;
    let a = TABLE[idx];
    let b = TABLE[idx + 1];
    (
        a.0 + (b.0 - a.0) * frac,
        a.1 + (b.1 - a.1) * frac,
        a.2 + (b.2 - a.2) * frac,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::devices::synth::{FACTORY_PRESETS, ModSlot, vital_factory_presets};

    #[test]
    fn all_factory_presets_render_clean_balanced_audio() {
        let sr = 48_000u32;
        for preset in FACTORY_PRESETS {
            let mut engine = NativeSynthEngine::new(preset.to_params(), sr);
            let note = if preset.engine == SynthEngineKind::Sub808 {
                36
            } else {
                60
            };
            engine.note_on(note, 105, 0.0);
            let mut left = vec![0.0; 24_000];
            let mut right = vec![0.0; 24_000];
            engine.render(&mut left[..12_000], &mut right[..12_000]);
            engine.note_off(note);
            engine.render(&mut left[12_000..], &mut right[12_000..]);

            let mut peak = 0.0_f32;
            let mut sum_sq = 0.0_f64;
            for (&l, &r) in left.iter().zip(&right) {
                assert!(l.is_finite() && r.is_finite(), "NaN/Inf in {}", preset.name);
                peak = peak.max(l.abs().max(r.abs()));
                sum_sq += f64::from(l) * f64::from(l) + f64::from(r) * f64::from(r);
            }
            let rms = (sum_sq / (left.len() as f64 * 2.0)).sqrt() as f32;
            let rms_db = 20.0 * rms.max(1e-6).log10();
            assert!(
                rms_db > -34.0 && rms_db < -3.0 && peak <= 1.0,
                "Preset {} out of range: rms_db={rms_db:.1}, peak={peak:.3}",
                preset.name
            );
        }
    }

    #[test]
    fn all_vital_factory_presets_render_clean_balanced_audio() {
        let sr = 48_000u32;
        for (name, _, params) in vital_factory_presets() {
            let mut engine = NativeSynthEngine::new(params, sr);
            engine.note_on(48, 110, 0.0);
            let mut left = vec![0.0; 24_000];
            let mut right = vec![0.0; 24_000];
            engine.render(&mut left[..12_000], &mut right[..12_000]);
            engine.note_off(48);
            engine.render(&mut left[12_000..], &mut right[12_000..]);

            let mut peak = 0.0_f32;
            let mut sum_sq = 0.0_f64;
            for (&l, &r) in left.iter().zip(&right) {
                assert!(l.is_finite() && r.is_finite(), "NaN/Inf in {name}");
                peak = peak.max(l.abs().max(r.abs()));
                sum_sq += f64::from(l) * f64::from(l) + f64::from(r) * f64::from(r);
            }
            let rms = (sum_sq / (left.len() as f64 * 2.0)).sqrt() as f32;
            let rms_db = 20.0 * rms.max(1e-6).log10();
            assert!(
                rms_db > -34.0 && rms_db < -2.0 && peak <= 1.0,
                "Vital preset {name} out of range: rms_db={rms_db:.1}, peak={peak:.3}"
            );
        }
    }

    #[test]
    fn touch_couples_velocity_to_loudness_and_brightness() {
        let mut params = FACTORY_PRESETS[4].to_params();
        params.touch = 0.9;
        let render_rms = |vel: u8| {
            let mut engine = NativeSynthEngine::new(params.clone(), 48_000);
            engine.note_on(60, vel, 0.0);
            let mut l = vec![0.0; 8192];
            let mut r = vec![0.0; 8192];
            engine.render(&mut l, &mut r);
            (l.iter().map(|x| x * x).sum::<f32>() / l.len() as f32).sqrt()
        };
        let soft = render_rms(35);
        let hard = render_rms(120);
        assert!(hard > soft * 1.8, "soft={soft}, hard={hard}");
    }

    #[test]
    fn mod_matrix_cross_maps_macro_distance_to_cutoff() {
        let mut base = vital_factory_presets()[0].2.clone();
        base.tone = 0.15;
        base.distance = 0.85;
        base.voice.matrix = [ModSlot::default(); 12];

        let mut mapped = base.clone();
        mapped.voice.matrix[0] = ModSlot {
            source: ModSource::Distance,
            target: ModTarget::FilterCutoff,
            amount: 0.95,
        };

        let brightness = |p: SynthParams| {
            let mut engine = NativeSynthEngine::new(p, 48_000);
            engine.note_on(60, 100, 0.0);
            let mut l = vec![0.0; 8192];
            let mut r = vec![0.0; 8192];
            engine.render(&mut l, &mut r);
            // Energía de alta frecuencia aproximada por diferencias consecutivas.
            l.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / l.len() as f32
        };

        let dark = brightness(base);
        let bright = brightness(mapped);
        assert!(bright > dark * 1.5, "dark={dark}, bright={bright}");
    }

    #[test]
    fn polyphonic_chord_with_custom_saw_wavetable_renders_hifi_without_clipping() {
        // Sierra cruda con salto brusco -> los 6 mip-maps DFT deben eliminar aliasing y evitar clipping en acordes.
        let mut raw_frames = [[0.0_f32; 256]; 8];
        for frame in &mut raw_frames {
            for (i, s) in frame.iter_mut().enumerate() {
                *s = (i as f32 / 256.0) * 2.0 - 1.0;
            }
        }
        let mut params = vital_factory_presets()[0].2.clone(); // 7x unison + OSC B + Sub + Chorus
        params.custom_wt_a = Some(Arc::new(CustomWavetable::new("Raw Saw".into(), raw_frames)));
        let mut engine = NativeSynthEngine::new(params, 48_000);
        // Acorde de 4 notas en registro medio-alto
        for note in [60, 64, 67, 72] {
            engine.note_on(note, 115, 0.0);
        }
        let mut l = vec![0.0; 16_384];
        let mut r = vec![0.0; 16_384];
        engine.render(&mut l, &mut r);
        let peak = l
            .iter()
            .chain(&r)
            .fold(0.0_f32, |m, &x| {
                assert!(x.is_finite());
                m.max(x.abs())
            });
        let rms = (l.iter().map(|x| x * x).sum::<f32>() / l.len() as f32).sqrt();
        let rms_db = 20.0 * rms.max(1e-6).log10();
        assert!(peak < 0.96, "Hard clipping detected: peak={peak}");
        assert!(rms_db > -28.0 && rms_db < -5.0, "rms_db={rms_db:.1}");
    }

    #[test]
    fn imported_heavy_patch_chord_renders_hifi_without_clipping_or_dc() {
        let json = r#"{
            "preset_name": "Heavy Imported Lead",
            "settings": {
                "osc_1_on": 1.0,
                "osc_1_level": 1.0,
                "osc_1_unison_voices": 8.0,
                "osc_1_unison_detune": 4.5,
                "osc_1_stereo_spread": 1.0,
                "osc_2_on": 1.0,
                "osc_2_level": 0.9,
                "osc_2_transpose": 12.0,
                "osc_2_unison_voices": 4.0,
                "osc_2_unison_detune": 3.0,
                "osc_3_on": 1.0,
                "osc_3_level": 0.8,
                "osc_3_transpose": -12.0,
                "filter_1_on": 1.0,
                "filter_1_cutoff": 95.0,
                "filter_1_resonance": 0.65,
                "filter_1_drive": 10.0,
                "distortion_on": 1.0,
                "distortion_drive": 14.0,
                "distortion_mix": 0.8,
                "chorus_on": 1.0,
                "chorus_dry_wet": 0.5,
                "reverb_on": 1.0,
                "reverb_dry_wet": 0.45,
                "env_1_attack": 0.01,
                "env_1_decay": 0.6,
                "env_1_sustain": 0.85,
                "env_1_release": 0.4,
                "modulations": [
                    { "source": "lfo_1", "destination": "filter_1_cutoff", "amount": 0.6 },
                    { "source": "env_2", "destination": "osc_1_wave_frame", "amount": 0.5 }
                ]
            }
        }"#;
        let (params, _summary) =
            elevate::devices::vital_import::parse_vital_json(json, "Heavy Imported Lead")
                .expect("heavy patch should parse");
        let mut engine = NativeSynthEngine::new(params, 48_000);
        for note in [57, 60, 64, 69] {
            engine.note_on(note, 120, 0.0);
        }
        let mut l = vec![0.0; 24_000];
        let mut r = vec![0.0; 24_000];
        engine.render(&mut l, &mut r);

        let mut peak = 0.0_f32;
        let mut dc_sum = 0.0_f64;
        let mut sq_sum = 0.0_f64;
        for (&xl, &xr) in l.iter().zip(&r) {
            assert!(xl.is_finite() && xr.is_finite());
            peak = peak.max(xl.abs().max(xr.abs()));
            dc_sum += f64::from(xl) + f64::from(xr);
            sq_sum += f64::from(xl * xl) + f64::from(xr * xr);
        }
        let dc_offset = (dc_sum / (l.len() as f64 * 2.0)).abs();
        let rms = (sq_sum / (l.len() as f64 * 2.0)).sqrt() as f32;
        let rms_db = 20.0 * rms.max(1e-6).log10();
        assert!(peak < 0.96, "Hard clipping on imported heavy patch: peak={peak}");
        assert!(dc_offset < 0.03, "Excessive DC offset: {dc_offset}");
        assert!(rms_db > -26.0 && rms_db < -3.0, "rms_db={rms_db:.1}");
    }

    #[test]
    fn real_esw_reese_bank_renders_hifi_clean_if_present() {
        let bank = std::path::Path::new(
            "/home/burdier/Music/Echo+Sound+Works+-+Reese+Basses/Echo Sound Works - Reese Basses/Vital/ESW - Reese Basses V1.vitalbank",
        );
        if !bank.is_file() {
            return;
        }
        let entries = elevate::devices::vital_import::extract_vitalbank(bank)
            .expect("should extract ESW Reese Basses bank");
        assert!(entries.len() >= 10, "expected >= 10 presets, got {}", entries.len());

        let mut decoded_total = 0usize;
        for entry in &entries {
            let (params, summary) = elevate::devices::vital_import::load_vital_file(&entry.path)
                .unwrap_or_else(|e| panic!("failed to load {}: {e}", entry.name));
            if summary.decoded_wts > 0 {
                decoded_total += 1;
            }
            let mut engine = NativeSynthEngine::new(params, 48_000);
            // Nota C4 (60): estos Reese traen transpose -24/-12, así que C4 suena en C2 (65 Hz).
            engine.note_on(60, 110, 0.0);
            let mut l = vec![0.0; 24_000];
            let mut r = vec![0.0; 24_000];
            engine.render(&mut l[..18_000], &mut r[..18_000]);
            engine.note_off(60);
            engine.render(&mut l[18_000..], &mut r[18_000..]);

            let mut peak = 0.0_f32;
            let mut dc_sum = 0.0_f64;
            let mut sq_sum = 0.0_f64;
            for (&xl, &xr) in l.iter().zip(&r) {
                assert!(xl.is_finite() && xr.is_finite(), "NaN in {}", entry.name);
                peak = peak.max(xl.abs().max(xr.abs()));
                dc_sum += f64::from(xl) + f64::from(xr);
                sq_sum += f64::from(xl * xl) + f64::from(xr * xr);
            }
            let dc = (dc_sum / (l.len() as f64 * 2.0)).abs();
            let rms = (sq_sum / (l.len() as f64 * 2.0)).sqrt() as f32;
            let rms_db = 20.0 * rms.max(1e-6).log10();
            eprintln!(
                "ESW {:30} | wts={} | peak={:.3} | rms={:5.1} dB | dc={:.4}",
                entry.name, summary.decoded_wts, peak, rms_db, dc
            );
            assert!(
                peak < 0.96,
                "Clipping in {}: peak={peak:.3}, rms={rms_db:.1}",
                entry.name
            );
            assert!(
                rms_db > -36.0 && rms_db < -3.0,
                "Out-of-range loudness in {}: rms={rms_db:.1}, peak={peak:.3}",
                entry.name
            );
            assert!(dc < 0.04, "DC offset in {}: {dc:.4}", entry.name);
        }
        assert_eq!(
            decoded_total,
            entries.len(),
            "All ESW Reese presets should decode real custom wavetables"
        );
    }
}
