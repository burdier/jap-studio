//! Motor DSP polifónico en tiempo real de Hay Synth (Espacio + Cuerpo + Intención).
//! 0 asignaciones en render, 0 bloqueos, 0 dependencias externas.
use elevate::devices::synth::{EnvelopeMode, RoomKind, SynthEngineKind, SynthParams, cc};
use std::f32::consts::{PI, TAU};

const MAX_VOICES: usize = 12;
const PLUCK_BUF: usize = 2048;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Filtro TPT SVF mono (pasa-bajos + pasa-banda) estable a cualquier frecuencia.
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
    fn process(&mut self, v0: f32, cutoff_hz: f32, q: f32, sr: f32) -> (f32, f32) {
        let fc = cutoff_hz.clamp(25.0, sr * 0.44);
        let g = (PI * fc / sr).tan();
        let k = 1.0 / q.clamp(0.5, 10.0);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = v0 - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        (v2, v1) // (lp, bp)
    }
}

struct Voice {
    active: bool,
    note: u8,
    age: u64,
    velocity: f32,
    pan: f32,
    freq: f32,
    target_freq: f32,
    phase: [f32; 3],
    sub_phase: f32,
    mod_phase: f32,
    lfo_phase: f32,
    pitch_env: f32,
    timbre_env: f32,
    env: f32,
    stage: EnvStage,
    svf_l: Svf,
    svf_r: Svf,
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
            freq: 261.63,
            target_freq: 261.63,
            phase: [0.0; 3],
            sub_phase: 0.0,
            mod_phase: 0.0,
            lfo_phase: (seed as f32 * 0.137).fract(),
            pitch_env: 0.0,
            timbre_env: 0.0,
            env: 0.0,
            stage: EnvStage::Off,
            svf_l: Svf::default(),
            svf_r: Svf::default(),
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
        self.target_freq = target;
        self.freq = if params.glide > 0.02 && start_freq > 15.0 {
            start_freq
        } else {
            target
        };
        self.pitch_env = 1.0;
        self.timbre_env = 1.0;
        self.stage = EnvStage::Attack;
        // Conservar continuidad si ya había un remanente suave en env.
        if self.env < 0.001 {
            self.phase = [0.0, 0.21, 0.57];
            self.sub_phase = 0.0;
            self.mod_phase = 0.0;
            self.svf_l.reset();
            self.svf_r.reset();
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
        // Filtro comb por posición de pulsación en la cuerda (como guitarra/arpa real).
        for i in (comb_offset..len).rev() {
            self.pluck_buf[i] -= 0.55 * self.pluck_buf[i - comb_offset];
        }
        // Quitar DC.
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
        let fb = (fb_base + distance * 0.09).min(0.92);
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
            self.comb_l[i][il] = mono_in + in_l * 0.25 + self.comb_lp_l[i] * fb;
            self.comb_r[i][ir] = mono_in + in_r * 0.25 + self.comb_lp_r[i] * fb;
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

        // Absorción de aire con la distancia: alejar la entidad atenúa suavemente agudos.
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
    voices: Vec<Voice>,
    space: SpaceUnit,
    age_counter: u64,
    last_freq: f32,
    sustain_pedal: bool,
    sustained_notes: [bool; 128],
    pitch_bend_ratio: f32,
    master_volume: f32,
    bpm: f32,
}

impl NativeSynthEngine {
    pub fn new(mut params: SynthParams, sample_rate: u32) -> Self {
        params.clamp_all();
        let voices = (0..MAX_VOICES)
            .map(|i| Voice::new((i as u32 + 1) * 19937))
            .collect();
        Self {
            params,
            sample_rate: sample_rate.max(8_000),
            voices,
            space: SpaceUnit::new(sample_rate.max(8_000)),
            age_counter: 0,
            last_freq: 0.0,
            sustain_pedal: false,
            sustained_notes: [false; 128],
            pitch_bend_ratio: 1.0,
            master_volume: 1.0,
            bpm: 120.0,
        }
    }

    pub fn set_params(&mut self, mut params: SynthParams) {
        params.clamp_all();
        self.params = params;
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
            || (self.params.glide >= 0.45 && self.params.engine == SynthEngineKind::FoldAcid);

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
            if v.active && v.note == note && v.stage != EnvStage::Release {
                if self.params.mode != EnvelopeMode::Hit {
                    v.stage = EnvStage::Release;
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
        }
        self.space.reset();
    }

    pub fn midi(&mut self, command: u8, data1: u8, data2: u8) {
        match command & 0xf0 {
            0x90 if data2 > 0 => self.note_on(data1, data2, 0.0),
            0x80 | 0x90 => self.note_off(data1),
            0xb0 => match data1 {
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
                // CC 102..=113: control en vivo durante Play del Arrangement (0..=127).
                102 => self.control(cc::ENGINE, f32::from(data2.min(6))),
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

        // Tiempos de envolvente según el gesto (Hit / Hold / Swell) y `time`.
        let time = params.time;
        let (base_attack_s, decay_s, sustain_lvl, release_s) = match params.mode {
            EnvelopeMode::Hit => (0.002, 0.08 + time * time * 2.2, 0.0, 0.04 + time * 0.45),
            EnvelopeMode::Hold => (
                0.004 + time * 0.035,
                0.14 + time * 0.9,
                0.76,
                0.08 + time * 1.1,
            ),
            EnvelopeMode::Swell => (
                0.12 + time * time * 1.6,
                0.35 + time * 1.0,
                0.86,
                0.28 + time * 2.2,
            ),
        };

        let glide_coeff = if params.glide <= 0.02 {
            1.0
        } else {
            let glide_s = 0.015 + params.glide * params.glide * 0.38;
            1.0 - (-inv_sr / glide_s).exp()
        };

        // Frecuencia de respiración ligada al tempo del proyecto.
        let lfo_hz = (self.bpm / 60.0) * (0.25 + params.motion * 1.75);
        let lfo_inc = lfo_hz * inv_sr;
        let out_gain = params.gain * self.master_volume * 0.42;

        for i in 0..frames {
            let mut sum_l = 0.0_f32;
            let mut sum_r = 0.0_f32;

            for v in &mut self.voices {
                if !v.active {
                    continue;
                }

                // Intención acoplada a la fuerza del toque (`velocity * touch`).
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
                        if params.mode == EnvelopeMode::Hit && v.env < 0.0008 {
                            v.env = 0.0;
                            v.stage = EnvStage::Off;
                            v.active = false;
                            continue;
                        } else if params.mode != EnvelopeMode::Hit
                            && (v.env - sustain_lvl).abs() < 0.001
                        {
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

                v.pitch_env *= (-inv_sr / 0.042).exp();
                v.timbre_env *= (-inv_sr / (0.10 + time * 0.85)).exp();
                v.freq += (v.target_freq - v.freq) * glide_coeff;
                v.lfo_phase = (v.lfo_phase + lfo_inc).fract();
                let lfo = (v.lfo_phase * TAU).sin();

                // Micro-vibrato orgánico escalado por `motion`.
                let drift = 1.0 + lfo * params.motion * 0.0055;
                let hz = (v.freq * self.pitch_bend_ratio * drift).clamp(20.0, sr * 0.42);

                let (mut sl, mut sr_ch) = match params.engine {
                    SynthEngineKind::Sub808 => {
                        let drop_semi = params.shape * 18.0 * v.pitch_env;
                        let f = hz * 2.0_f32.powf(drop_semi / 12.0);
                        v.phase[0] = (v.phase[0] + f * inv_sr).fract();
                        let s0 = (v.phase[0] * TAU).sin();
                        let s2 = ((v.phase[0] * 2.0).fract() * TAU).sin() * 0.28 * params.shape;
                        let grit = 1.0 + params.shape * (2.2 + 2.5 * vel_eff);
                        let driven = ((s0 + s2) * grit).tanh();
                        let sub_w = params.body * 0.14 * lfo;
                        (driven * (1.0 - sub_w), driven * (1.0 + sub_w))
                    }
                    SynthEngineKind::Analog => {
                        let det = 0.002 + params.body * 0.011;
                        let freqs = [hz, hz * (1.0 - det), hz * (1.0 + det)];
                        let mut outs = [0.0_f32; 3];
                        for k in 0..3 {
                            let dt = freqs[k] * inv_sr;
                            v.phase[k] = (v.phase[k] + dt).fract();
                            let saw = polyblep_saw(v.phase[k], dt);
                            let tri = 4.0 * (v.phase[k] - 0.5).abs() - 1.0;
                            outs[k] = tri * (1.0 - params.shape) + saw * params.shape;
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
                        let stereo = (cur - v.pluck_lp) * params.body * 0.6;
                        ((cur + stereo) * 1.15, (cur - stereo) * 1.15)
                    }
                    SynthEngineKind::FmBell => {
                        // Ratios musicales: 1.0 (warm) -> 2.0 (e-piano) -> 3.5 (bell) -> 5.4 (metal)
                        let ratio = if params.shape < 0.33 {
                            1.0 + (params.shape / 0.33)
                        } else if params.shape < 0.66 {
                            2.0 + ((params.shape - 0.33) / 0.33) * 1.5
                        } else {
                            3.5 + ((params.shape - 0.66) / 0.34) * 1.9
                        };
                        let mod_f = hz * ratio;
                        v.mod_phase = (v.mod_phase + mod_f * inv_sr).fract();
                        v.phase[0] = (v.phase[0] + hz * inv_sr).fract();
                        v.phase[1] = (v.phase[1] + hz * (1.0 + params.body * 0.005) * inv_sr).fract();
                        let index = (0.4 + params.tone * 3.8) * (0.35 + 0.65 * v.timbre_env) * vel_eff;
                        let m = (v.mod_phase * TAU).sin() * index;
                        let cl = (v.phase[0] * TAU + m).sin();
                        let cr = (v.phase[1] * TAU + m * 0.94).sin();
                        (cl * 0.78, cr * 0.78)
                    }
                    SynthEngineKind::Vocal => {
                        let dt = hz * inv_sr;
                        v.phase[0] = (v.phase[0] + dt).fract();
                        v.phase[1] = (v.phase[1] + dt * (1.0 + params.body * 0.006)).fract();
                        let src = 0.55 * polyblep_saw(v.phase[0], dt)
                            + 0.45 * polyblep_saw(v.phase[1], dt)
                            + v.next_noise() * (0.04 + params.body * 0.06);
                        let (f1, f2, f3) = vowel_formants(params.shape);
                        let shift = 1.0 + (params.tone - 0.5) * 0.35 + lfo * params.motion * 0.04;
                        let (_, b1) = v.formants[0].process(src, f1 * shift, 5.5, sr);
                        let (_, b2) = v.formants[1].process(src, f2 * shift, 7.0, sr);
                        let (_, b3) = v.formants[2].process(src, f3 * shift, 8.0, sr);
                        let body_s = b1 * 0.70 + b2 * 0.55 + b3 * 0.32;
                        let diff = (b2 - b3) * params.body * 0.35;
                        ((body_s + diff) * 1.45, (body_s - diff) * 1.45)
                    }
                    SynthEngineKind::FoldAcid => {
                        let dt = hz * inv_sr;
                        v.phase[0] = (v.phase[0] + dt).fract();
                        v.phase[1] = (v.phase[1] + dt * (1.0 + params.body * 0.007)).fract();
                        let raw_l = 0.6 * polyblep_saw(v.phase[0], dt)
                            + 0.4 * (v.phase[0] * TAU).sin();
                        let raw_r = 0.6 * polyblep_saw(v.phase[1], dt)
                            + 0.4 * (v.phase[1] * TAU).sin();
                        let fold = 1.2 + params.shape * 4.5 * (0.4 + 0.6 * v.timbre_env);
                        let fl = (raw_l * fold).sin();
                        let fr = (raw_r * fold).sin();
                        (fl * 0.78, fr * 0.78)
                    }
                    SynthEngineKind::Texture => {
                        let d1 = hz * inv_sr;
                        let d2 = hz * 1.498 * (1.0 + lfo * 0.003) * inv_sr;
                        let d3 = hz * 2.003 * (1.0 - lfo * 0.004) * inv_sr;
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
                        let air_mix = 0.12 + params.shape * 0.45;
                        (
                            tones_l * (1.0 - air_mix * 0.4) + air_l * air_mix * 1.5,
                            tones_r * (1.0 - air_mix * 0.4) + air_r * air_mix * 1.5,
                        )
                    }
                };

                // Piel / Filtro acoplado a `tone` + `velocity * touch` + `timbre_env` + `motion`.
                let base_cut = 160.0 * (80.0_f32).powf(params.tone);
                let env_mod = 1.0 + v.timbre_env * (0.4 + 2.4 * vel_eff);
                let lfo_mod = 1.0 + lfo * params.motion * 0.18;
                // Keytracking físico suave: notas agudas abren ligeramente el aire.
                let key_scale = (hz / 261.63).powf(0.28).clamp(0.65, 2.4);
                let cutoff = (base_cut * env_mod * lfo_mod * key_scale).clamp(60.0, 18_500.0);
                let q = if params.engine == SynthEngineKind::FoldAcid {
                    1.2 + params.shape * 4.2
                } else {
                    0.72 + params.shape * 0.55
                };
                if params.engine != SynthEngineKind::Pluck {
                    (sl, _) = v.svf_l.process(sl, cutoff, q, sr);
                    (sr_ch, _) = v.svf_r.process(sr_ch, cutoff, q, sr);
                }

                // Calor analógico suave con la intensidad del toque.
                let drive = 1.0 + vel_eff * (0.18 + params.body * 0.35);
                sl = (sl * drive).tanh();
                sr_ch = (sr_ch * drive).tanh();

                let amp = v.env * (0.28 + 0.72 * vel_eff);
                let pan_l = (1.0 - v.pan).min(1.0);
                let pan_r = (1.0 + v.pan).min(1.0);
                sum_l += sl * amp * pan_l;
                sum_r += sr_ch * amp * pan_r;
            }

            let (out_l, out_r) =
                self.space
                    .process(sum_l, sum_r, params.room, params.distance, sr);
            left[i] = (out_l * out_gain).tanh();
            right[i] = (out_r * out_gain).tanh();
        }
    }
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
    use elevate::devices::synth::FACTORY_PRESETS;

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
    fn touch_couples_velocity_to_loudness_and_brightness() {
        let mut params = FACTORY_PRESETS[4].to_params(); // Lo-Fi Tape Keys
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
}
