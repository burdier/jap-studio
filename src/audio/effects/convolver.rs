//! Motor de convolución FFT lock-free para IRs estéreo (reverbs, salas y gabinetes).
//! Toda la preparación de kernels, resampling y buffers ocurre fuera del callback RT.
use super::{Plan, pdc::DelayBank};
use elevate::musical::fx::{FxId, cabinet::CabinetImpulse};
use fft_convolver::FFTConvolver;
use std::{
    cell::UnsafeCell,
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

#[derive(Clone, PartialEq)]
pub(super) struct Cabinet {
    pub model: usize,
    pub mic: usize,
    pub mix: f32,
    pub gain_db: f32,
    pub predelay_ms: f32,
    pub decay: f32,
    pub ir: Option<CabinetImpulse>,
}

/// Clave del kernel FFT: cambiar Mix o Gain NO recompila el convolver ni corta la cola.
#[derive(Clone, PartialEq)]
struct KernelKey {
    model: usize,
    mic: usize,
    predelay_ms_tenths: i32,
    decay_thousandths: i32,
    ir: Option<CabinetImpulse>,
}

impl Cabinet {
    fn kernel_key(&self) -> KernelKey {
        KernelKey {
            model: self.model,
            mic: self.mic,
            predelay_ms_tenths: (self.predelay_ms * 10.0).round() as i32,
            decay_thousandths: (self.decay * 1000.0).round() as i32,
            ir: self.ir.clone(),
        }
    }

    pub fn tail(&self, rate: u32) -> u64 {
        let predelay = ((self.predelay_ms / 1000.0) * rate as f32).round().max(0.0) as u64;
        let base = if let Some(ir) = &self.ir {
            (ir.pcm.len() as u64 / 4 * rate as u64).div_ceil(ir.rate.max(1) as u64)
        } else {
            let secs = match self.model {
                0 => 0.90, // Room
                1 => 2.40, // Hall
                2 => 1.60, // Plate
                _ => 0.04, // Cab 4x12
            };
            (secs * rate as f64).round() as u64
        };
        let scaled = ((base as f64) * f64::from(self.decay.clamp(0.1, 1.0))).round() as u64;
        predelay + scaled.max(64)
    }

    fn build_stereo_ir(&self, rate: u32) -> [Vec<f32>; 2] {
        let rate_f = rate.max(1) as f32;
        let predelay = ((self.predelay_ms / 1000.0) * rate_f).round().max(0.0) as usize;
        let decay = self.decay.clamp(0.1, 1.0);

        let mut stereo = if let Some(ir) = &self.ir {
            let count = ir.pcm.len() / 4;
            let mut src_frames = Vec::with_capacity(count);
            for i in 0..count {
                let p = i * 4;
                let l = i16::from_le_bytes([ir.pcm[p], ir.pcm[p + 1]]) as f32 / 32768.0;
                let r = i16::from_le_bytes([ir.pcm[p + 2], ir.pcm[p + 3]]) as f32 / 32768.0;
                src_frames.push([l, r]);
            }
            let resampled = if ir.rate != rate {
                crate::audio::export::resample(&src_frames, ir.rate, rate).unwrap_or(src_frames)
            } else {
                src_frames
            };
            let target_len = ((resampled.len() as f32 * decay).round() as usize).max(64);
            let mut left = Vec::with_capacity(target_len);
            let mut right = Vec::with_capacity(target_len);
            for (i, frame) in resampled.into_iter().take(target_len).enumerate() {
                // Fade suave en el último 12% si el usuario acortó el Decay
                let env = if decay < 0.995 {
                    let frac = i as f32 / target_len.max(1) as f32;
                    if frac > 0.85 {
                        ((1.0 - frac) / 0.15).clamp(0.0, 1.0).powi(2)
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                left.push(frame[0] * env);
                right.push(frame[1] * env);
            }
            [left, right]
        } else {
            self.synthesize_factory_ir(rate)
        };

        // Aplicar tono (damping) en presets o IRs si se eligió Cálido / Natural
        if self.ir.is_none() || self.mic != 0 {
            let cutoff = match self.mic {
                0 => 16000.0, // Brillante
                1 => 9500.0,  // Natural
                _ => 4200.0,  // Cálido
            };
            if cutoff < 15500.0 {
                let a = (-std::f32::consts::TAU * cutoff / rate_f).exp();
                for ch in &mut stereo {
                    let mut lp = 0.0_f32;
                    for s in ch.iter_mut() {
                        lp = a * lp + (1.0 - a) * *s;
                        *s = lp;
                    }
                }
            }
        }

        // Auto-normalización por energía RMS para evitar explosiones de volumen o IRs mudas
        let energy: f64 = stereo
            .iter()
            .flat_map(|ch| ch.iter())
            .map(|&s| f64::from(s) * f64::from(s))
            .sum();
        let rms_total = (energy * 0.5).sqrt();
        if rms_total > 1e-6 {
            let target = if self.ir.is_some() || self.model == 3 {
                0.85
            } else {
                0.55
            };
            let scale = (target / rms_total).clamp(0.05, 4.0) as f32;
            for ch in &mut stereo {
                for s in ch.iter_mut() {
                    *s *= scale;
                }
            }
        }

        // Prepend pre-delay sin costo en RT
        if predelay > 0 {
            for ch in &mut stereo {
                let mut padded = vec![0.0_f32; predelay + ch.len()];
                padded[predelay..].copy_from_slice(ch);
                *ch = padded;
            }
        }

        stereo
    }

    /// Genera IRs estéreo de convolución verdaderas (decorrelacionadas L/R) para Room, Hall, Plate y Cab.
    fn synthesize_factory_ir(&self, rate: u32) -> [Vec<f32>; 2] {
        let rate_f = rate.max(1) as f32;
        let decay = self.decay.clamp(0.1, 1.0);
        if self.model == 3 {
            // Cab 4x12: resonancia de caja + reflexiones internas + rolloff real de cono
            let len = ((0.04 * rate_f * decay).round() as usize).max(64);
            let mut out = [vec![0.0; len], vec![0.0; len]];
            let hp = (-std::f32::consts::TAU * 78.0 / rate_f).exp();
            let lp = (-std::f32::consts::TAU * 4300.0 / rate_f).exp();
            let res_w = std::f32::consts::TAU * 115.0 / rate_f;
            for (ch_idx, buf) in out.iter_mut().enumerate() {
                let (mut b, mut t1, mut t2) = (0.0_f32, 0.0_f32, 0.0_f32);
                let echo1 = ((0.00145 + ch_idx as f32 * 0.00018) * rate_f) as usize;
                let echo2 = ((0.00310 + ch_idx as f32 * 0.00027) * rate_f) as usize;
                for (i, sample) in buf.iter_mut().enumerate() {
                    let mut x = if i == 0 { 1.0 } else { 0.0 };
                    if i == echo1 {
                        x += 0.28;
                    }
                    if i == echo2 {
                        x -= 0.14;
                    }
                    // Resonancia de cabina amortiguada
                    x += 0.12 * (res_w * i as f32).sin() * (-(i as f32) / (0.008 * rate_f)).exp();
                    b = hp * b + (1.0 - hp) * x;
                    let highpassed = x - b;
                    t1 = lp * t1 + (1.0 - lp) * highpassed;
                    t2 = lp * t2 + (1.0 - lp) * t1;
                    *sample = t2;
                }
            }
            return out;
        }

        // Room (0), Hall (1), Plate (2): convolución de reverberación estéreo real
        let (base_secs, rt60_tau, er_gain, damp_start_hz, damp_end_hz) = match self.model {
            0 => (0.90_f32, 0.28_f32, 0.45_f32, 11000.0_f32, 2600.0_f32), // Room
            1 => (2.40_f32, 0.78_f32, 0.30_f32, 10000.0_f32, 1800.0_f32), // Hall
            _ => (1.60_f32, 0.52_f32, 0.18_f32, 14000.0_f32, 4200.0_f32), // Plate
        };
        let len = ((base_secs * rate_f * decay).round() as usize).max(256);
        let tau = (rt60_tau * decay * rate_f).max(64.0);
        let attack = (0.012 * rate_f).max(1.0);
        let mut out = [vec![0.0_f32; len], vec![0.0_f32; len]];

        // Reflexiones tempranas (Early Reflections) estéreo asimétricas
        let er_taps: [(f32, f32, f32); 8] = [
            (0.007, 0.72, 0.40),
            (0.013, -0.45, 0.68),
            (0.019, 0.58, -0.35),
            (0.027, -0.32, 0.54),
            (0.036, 0.44, 0.48),
            (0.047, -0.38, -0.41),
            (0.061, 0.30, -0.34),
            (0.078, -0.25, 0.29),
        ];
        for &(sec, gl, gr) in &er_taps {
            let idx = (sec * rate_f) as usize;
            if idx < len {
                out[0][idx] += gl * er_gain;
                out[1][idx] += gr * er_gain;
            }
        }

        // Cola difusa estéreo decorrelacionada con amortiguamiento progresivo dependiente del tiempo
        for (ch_idx, buf) in out.iter_mut().enumerate() {
            let mut rng: u64 = if ch_idx == 0 {
                0x9E37_79B9_7F4A_7C15 ^ (self.model as u64 * 0x10003)
            } else {
                0xBF58_476D_1CE4_E5B9 ^ (self.model as u64 * 0x20007)
            };
            let mut lp = 0.0_f32;
            let mut hp = 0.0_f32;
            let hp_a = (-std::f32::consts::TAU * 90.0 / rate_f).exp();
            for (i, sample) in buf.iter_mut().enumerate() {
                // SplitMix64 determinista -> ruido gaussiano aproximado [-1, 1]
                rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = rng;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                let u1 = ((z >> 40) as f32) / ((1u32 << 24) as f32) * 2.0 - 1.0;
                let u2 = (((z >> 16) & 0xFF_FFFF) as f32) / ((1u32 << 24) as f32) * 2.0 - 1.0;
                let noise = (u1 + u2) * 0.5;

                let frac = i as f32 / len as f32;
                let env = (1.0 - (-(i as f32) / attack).exp()) * (-(i as f32) / tau).exp();
                let tail_fade = if frac > 0.90 {
                    ((1.0 - frac) / 0.10).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let cutoff = damp_start_hz + (damp_end_hz - damp_start_hz) * frac;
                let lp_a = (-std::f32::consts::TAU * cutoff / rate_f).exp();
                let raw = *sample + noise * env * tail_fade * 0.35;
                lp = lp_a * lp + (1.0 - lp_a) * raw;
                hp = hp_a * hp + (1.0 - hp_a) * lp;
                *sample = lp - hp;
            }
        }
        out
    }
}

struct Instance {
    key: KernelKey,
    mix: AtomicU32,
    gain: AtomicU32,
    busy: AtomicBool,
    channels: UnsafeCell<[FFTConvolver<f32>; 2]>,
}

// Solo el callback RT que adquiere `busy` vía CAS accede a `channels`.
unsafe impl Sync for Instance {}
unsafe impl Send for Instance {}

pub(super) struct StreamDsp {
    pub delays: DelayBank,
    cabinets: BTreeMap<FxId, Arc<Instance>>,
}

impl StreamDsp {
    pub fn prepare(plan: &Plan, previous: Option<&Self>, rate: u32) -> Self {
        let mut cabinets = BTreeMap::new();
        fn visit(
            p: &Plan,
            rate: u32,
            old: Option<&StreamDsp>,
            out: &mut BTreeMap<FxId, Arc<Instance>>,
        ) {
            match p {
                Plan::Cabinet(id, spec, _) => {
                    let key = spec.kernel_key();
                    let mix_bits = spec.mix.clamp(0.0, 1.0).to_bits();
                    let gain_lin = 10.0_f32.powf(spec.gain_db.clamp(-12.0, 12.0) / 20.0);
                    let instance = old
                        .and_then(|s| s.cabinets.get(id))
                        .filter(|i| i.key == key)
                        .cloned()
                        .map(|existing| {
                            existing.mix.store(mix_bits, Ordering::Relaxed);
                            existing.gain.store(gain_lin.to_bits(), Ordering::Relaxed);
                            existing
                        })
                        .unwrap_or_else(|| {
                            let stereo = spec.build_stereo_ir(rate);
                            // Partición de 512 para IRs largas (>1.5 s) para mantener CPU baja.
                            let block_size = if stereo[0].len() > (rate as usize * 3 / 2) {
                                512
                            } else {
                                128
                            };
                            Arc::new(Instance {
                                key,
                                mix: AtomicU32::new(mix_bits),
                                gain: AtomicU32::new(gain_lin.to_bits()),
                                busy: AtomicBool::new(false),
                                channels: UnsafeCell::new(std::array::from_fn(|ch| {
                                    let mut engine = FFTConvolver::default();
                                    engine
                                        .init(block_size, &stereo[ch])
                                        .expect("nonzero convolution block");
                                    engine
                                })),
                            })
                        });
                    out.insert(*id, instance);
                }
                Plan::Serial(v) | Plan::Parallel(v) | Plan::Crossover(_, _, v) => {
                    for n in v {
                        visit(n, rate, old, out)
                    }
                }
                Plan::Observe(n) => visit(n, rate, old, out),
                _ => {}
            }
        }
        visit(plan, rate, previous, &mut cabinets);
        Self {
            delays: DelayBank::prepare(plan, previous.map(|s| &s.delays)),
            cabinets,
        }
    }

    pub fn convolve(&self, id: FxId, frames: &mut [[f32; 2]]) {
        let Some(inst) = self.cabinets.get(&id) else {
            return;
        };
        if inst
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        let mix = f32::from_bits(inst.mix.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        let dry_gain = 1.0 - mix;
        let out_gain = f32::from_bits(inst.gain.load(Ordering::Relaxed));
        let channels = unsafe { &mut *inst.channels.get() };
        let (mut input, mut output) = ([0.0_f32; 512], [0.0_f32; 512]);
        for block in frames.chunks_mut(512) {
            let n = block.len();
            for ch in 0..2 {
                for (x, f) in input[..n].iter_mut().zip(block.iter()) {
                    *x = f[ch];
                }
                if channels[ch].process(&input[..n], &mut output[..n]).is_ok() {
                    for (i, f) in block.iter_mut().enumerate() {
                        let mixed = input[i] * dry_gain + output[i] * mix;
                        f[ch] = mixed * out_gain;
                    }
                }
            }
        }
        inst.busy.store(false, Ordering::Release);
    }
}
