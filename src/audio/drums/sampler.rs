//! Sampler PCM nativo de batería: lectura en memoria sin glitches de bloque ni reservas en RT.
use crate::audio::sampler::SampleAsset;
use elevate::{
    StereoFrame,
    devices::{
        filter::FilterMode,
        sampler::{
            ATTACK, CUTOFF, DECAY, END, GAIN, HOLD, PAN, PITCH_DECAY, PITCH_ENV, PlayMode, RELEASE,
            RESONANCE, ROOT, START, SUSTAIN, SamplerParams, TUNE, VEL_FILTER, VEL_VOLUME, VOICES,
        },
    },
    musical::drums::OffsetMode,
};
use std::sync::Arc;

const MAX_VOICES: usize = 16;
const CHOKE_FADE_SECONDS: f32 = 0.0025;

pub(super) struct PreparedLayer {
    asset: Arc<SampleAsset>,
    velocity_min: u8,
    velocity_max: u8,
    start_offset: u16,
    offset_mode: OffsetMode,
    hits: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
    ChokeFade,
}

/// Filtro SVF estéreo TPT (Cytomic) en línea: cero asignaciones y estable hasta Nyquist.
#[derive(Clone, Copy, Default)]
struct SvfStereo {
    ic1eq: [f32; 2],
    ic2eq: [f32; 2],
}

impl SvfStereo {
    fn reset(&mut self) {
        self.ic1eq = [0.0; 2];
        self.ic2eq = [0.0; 2];
    }

    #[inline]
    fn process(
        &mut self,
        input: StereoFrame,
        cutoff_hz: f32,
        q: f32,
        mode: FilterMode,
        rate: f32,
    ) -> StereoFrame {
        let fc = cutoff_hz.clamp(20.0, rate * 0.45);
        let g = (std::f32::consts::PI * fc / rate).tan();
        let k = 1.0 / q.clamp(0.5, 10.0);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let mut out = [0.0; 2];
        for ch in 0..2 {
            let v0 = input[ch];
            let v3 = v0 - self.ic2eq[ch];
            let v1 = a1 * self.ic1eq[ch] + a2 * v3;
            let v2 = self.ic2eq[ch] + a2 * self.ic1eq[ch] + a3 * v3;
            self.ic1eq[ch] = 2.0 * v1 - self.ic1eq[ch];
            self.ic2eq[ch] = 2.0 * v2 - self.ic2eq[ch];
            out[ch] = match mode {
                FilterMode::LowPass => v2,
                FilterMode::BandPass => v1,
                FilterMode::HighPass => v0 - k * v1 - v2,
            };
        }
        out
    }
}

#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    layer: usize,
    pitch: u8,
    velocity: u8,
    age: u64,
    pos: f64,
    start: usize,
    end: usize,
    reverse: bool,
    base_step: f64,
    pitch_env_semitones: f32,
    pitch_env_coeff: f32,
    pitch_env_state: f32,
    stage: EnvStage,
    env: f32,
    stage_frames: u32,
    stage_total: u32,
    stage_start_env: f32,
    vel_gain: f32,
    filter: SvfStereo,
}

impl Voice {
    const IDLE: Self = Self {
        active: false,
        layer: 0,
        pitch: 36,
        velocity: 100,
        age: 0,
        pos: 0.0,
        start: 0,
        end: 0,
        reverse: false,
        base_step: 1.0,
        pitch_env_semitones: 0.0,
        pitch_env_coeff: 0.0,
        pitch_env_state: 0.0,
        stage: EnvStage::Sustain,
        env: 0.0,
        stage_frames: 0,
        stage_total: 0,
        stage_start_env: 0.0,
        vel_gain: 1.0,
        filter: SvfStereo {
            ic1eq: [0.0; 2],
            ic2eq: [0.0; 2],
        },
    };

    fn begin_choke(&mut self, fade_frames: u32) {
        if self.active && self.stage != EnvStage::ChokeFade {
            self.stage = EnvStage::ChokeFade;
            self.stage_start_env = self.env;
            self.stage_frames = 0;
            self.stage_total = fade_frames.max(1);
        }
    }
}

pub(super) struct DrumSampler {
    layers: Vec<PreparedLayer>,
    params: SamplerParams,
    voices: [Voice; MAX_VOICES],
    rate: u32,
    next_age: u64,
}

impl DrumSampler {
    pub fn prepare(
        layers: Vec<(Arc<SampleAsset>, (u8, u8), u16, OffsetMode)>,
        params: SamplerParams,
        rate: u32,
    ) -> Result<Self, String> {
        if layers.is_empty() {
            return Err("Un instrumento necesita una muestra".into());
        }
        let prepared = layers
            .into_iter()
            .map(
                |(asset, (lo, hi), start_offset, offset_mode)| PreparedLayer {
                    asset,
                    velocity_min: lo.min(hi),
                    velocity_max: hi.max(lo),
                    start_offset,
                    offset_mode,
                    hits: 0,
                },
            )
            .collect();
        Ok(Self {
            layers: prepared,
            params,
            voices: [Voice::IDLE; MAX_VOICES],
            rate: rate.max(1),
            next_age: 0,
        })
    }

    pub fn update_live(&mut self, params: &SamplerParams) {
        let pitch_changed = self.params.get(TUNE) != params.get(TUNE)
            || self.params.get(ROOT) != params.get(ROOT)
            || self.params.keytrack != params.keytrack;
        self.params = params.clone();
        if pitch_changed {
            for voice in &mut self.voices {
                if voice.active
                    && let Some(layer) = self.layers.get(voice.layer)
                {
                    voice.base_step = playback_step(
                        layer.asset.file.sample_rate,
                        self.rate,
                        voice.pitch,
                        &self.params,
                    );
                }
            }
        }
    }

    pub fn has_one_shot_tail(&self) -> bool {
        self.params.mode == PlayMode::OneShot && self.voices.iter().any(|v| v.active)
    }

    pub fn cut(&mut self) {
        let fade = ((self.rate as f32) * CHOKE_FADE_SECONDS).round() as u32;
        for voice in &mut self.voices {
            voice.begin_choke(fade);
        }
    }

    pub fn all_off(&mut self) {
        if self.params.mode == PlayMode::OneShot {
            return;
        }
        let release_frames = ms_to_frames(self.params.get(RELEASE), self.rate).max(1);
        for voice in &mut self.voices {
            if voice.active && !matches!(voice.stage, EnvStage::Release | EnvStage::ChokeFade) {
                voice.stage = EnvStage::Release;
                voice.stage_start_env = voice.env;
                voice.stage_frames = 0;
                voice.stage_total = release_frames;
            }
        }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) {
        if velocity == 0 || pitch > 127 {
            return;
        }
        let Some(layer_idx) = self
            .layers
            .iter()
            .position(|l| velocity >= l.velocity_min && velocity <= l.velocity_max)
            .or_else(|| (!self.layers.is_empty()).then_some(0))
        else {
            return;
        };

        let choke_fade = ((self.rate as f32) * CHOKE_FADE_SECONDS).round() as u32;
        if self.params.choke {
            for voice in &mut self.voices {
                voice.begin_choke(choke_fade);
            }
        }

        let max_voices = (self.params.get(VOICES).round() as usize).clamp(1, MAX_VOICES);
        // Al hacer choke permitimos una voz adicional temporal para el fade-out de 2.5 ms sin robarla.
        let slot_limit = if self.params.choke {
            MAX_VOICES
        } else {
            max_voices
        };
        let slot = self.voices[..slot_limit]
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                self.voices[..slot_limit]
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| (v.stage != EnvStage::ChokeFade, v.age))
                    .map_or(0, |(idx, _)| idx)
            });

        let layer = &mut self.layers[layer_idx];
        let frames_len = layer.asset.file.frames.len();
        if frames_len == 0 {
            return;
        }
        let start = ((f64::from(self.params.get(START)) / 100.0) * frames_len as f64) as usize;
        let end = (((f64::from(self.params.get(END)) / 100.0) * frames_len as f64) as usize)
            .max(start + 1)
            .min(frames_len);
        let span = end.saturating_sub(start).max(1);
        let eff_offset = effective_start_offset(layer.start_offset, layer.offset_mode, layer.hits);
        layer.hits = layer.hits.wrapping_add(1);
        let skip = ((span as u64 * u64::from(eff_offset)) / 1000) as usize;
        let skip = skip.min(span.saturating_sub(1));

        let reverse = self.params.reverse;
        let (region_start, region_end, init_pos) = if reverse {
            let rev_end = end.saturating_sub(skip).max(start + 1);
            (start, rev_end, (rev_end - 1) as f64)
        } else {
            let fwd_start = (start + skip).min(end - 1);
            (fwd_start, end, fwd_start as f64)
        };

        let attack_frames = ms_to_frames(self.params.get(ATTACK), self.rate);
        let hold_frames = ms_to_frames(self.params.get(HOLD), self.rate);
        let (stage, env, stage_total) = if attack_frames > 0 {
            (EnvStage::Attack, 0.0, attack_frames)
        } else if hold_frames > 0 {
            (EnvStage::Hold, 1.0, hold_frames)
        } else {
            let decay_frames = ms_to_frames(self.params.get(DECAY), self.rate);
            if decay_frames > 0 && self.params.get(SUSTAIN) < 99.99 {
                (EnvStage::Decay, 1.0, decay_frames)
            } else {
                (EnvStage::Sustain, 1.0, 0)
            }
        };

        let vel_norm = f32::from(velocity) / 127.0;
        let vel_track = (self.params.get(VEL_VOLUME) / 100.0).clamp(0.0, 1.0);
        // Curva cuadrática suave estándar de sampler (como SFZ amp_veltrack): mantiene golpe sólido.
        let vel_gain = (1.0 - vel_track) + vel_track * (vel_norm * vel_norm);

        let pitch_env_st = self.params.get(PITCH_ENV);
        let pitch_decay_frames = ms_to_frames(self.params.get(PITCH_DECAY), self.rate).max(1);
        let pitch_env_coeff = (-4.6 / pitch_decay_frames as f32).exp();

        let step = playback_step(layer.asset.file.sample_rate, self.rate, pitch, &self.params);
        self.next_age = self.next_age.wrapping_add(1);

        let mut filter = SvfStereo::default();
        filter.reset();

        self.voices[slot] = Voice {
            active: true,
            layer: layer_idx,
            pitch,
            velocity,
            age: self.next_age,
            pos: init_pos,
            start: region_start,
            end: region_end,
            reverse,
            base_step: step,
            pitch_env_semitones: pitch_env_st,
            pitch_env_coeff,
            pitch_env_state: if pitch_env_st.abs() > 0.001 { 1.0 } else { 0.0 },
            stage,
            env,
            stage_frames: 0,
            stage_total,
            stage_start_env: 1.0,
            vel_gain,
            filter,
        };
    }

    pub fn note_off(&mut self, pitch: u8) {
        if self.params.mode == PlayMode::OneShot {
            return;
        }
        let release_frames = ms_to_frames(self.params.get(RELEASE), self.rate).max(1);
        for voice in &mut self.voices {
            if voice.active
                && voice.pitch == pitch
                && !matches!(voice.stage, EnvStage::Release | EnvStage::ChokeFade)
            {
                voice.stage = EnvStage::Release;
                voice.stage_start_env = voice.env;
                voice.stage_frames = 0;
                voice.stage_total = release_frames;
            }
        }
    }

    pub fn render(&mut self, output: &mut [StereoFrame]) {
        output.fill([0.0; 2]);
        if self.voices.iter().all(|v| !v.active) {
            return;
        }
        let rate = self.rate as f32;
        let master_gain = db_to_lin(self.params.get(GAIN));
        let pan = (self.params.get(PAN) / 100.0).clamp(-1.0, 1.0);
        let (pan_l, pan_r) = equal_power_pan(pan);
        let sustain = (self.params.get(SUSTAIN) / 100.0).clamp(0.0, 1.0);
        let hold_frames = ms_to_frames(self.params.get(HOLD), self.rate);
        let decay_frames = ms_to_frames(self.params.get(DECAY), self.rate);
        let filter_on = self.params.filter;
        let filter_mode = self.params.filter_mode;
        let base_cutoff = self.params.get(CUTOFF);
        let vel_filter_cents = self.params.get(VEL_FILTER);
        let res_db = self.params.get(RESONANCE);
        let filter_q = (0.707 * 10.0_f32.powf(res_db / 24.0)).clamp(0.5, 10.0);

        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            let Some(layer) = self.layers.get(voice.layer) else {
                voice.active = false;
                continue;
            };
            let frames = &layer.asset.file.frames;
            if frames.is_empty() {
                voice.active = false;
                continue;
            }
            let voice_gain = master_gain * voice.vel_gain;
            let voice_cutoff = if vel_filter_cents.abs() > 0.5 {
                let vel_norm = f32::from(voice.velocity) / 127.0;
                let cents = vel_filter_cents * (vel_norm - 0.5) * 2.0;
                (base_cutoff * (cents / 1200.0).exp2()).clamp(20.0, rate * 0.45)
            } else {
                base_cutoff
            };
            let direct_step = !voice.reverse
                && (voice.base_step - 1.0).abs() < 1e-7
                && voice.pitch_env_state == 0.0
                && voice.pos.fract().abs() < 1e-7;

            for out_frame in output.iter_mut() {
                if !voice.active {
                    break;
                }
                let sample = if direct_step {
                    let idx = voice.pos.round() as usize;
                    if idx >= voice.end || idx >= frames.len() {
                        voice.active = false;
                        break;
                    }
                    voice.pos += 1.0;
                    frames[idx]
                } else {
                    if voice.reverse {
                        if voice.pos < voice.start as f64 {
                            voice.active = false;
                            break;
                        }
                    } else if voice.pos >= voice.end as f64 {
                        voice.active = false;
                        break;
                    }
                    let s = sample_cubic(frames, voice.pos, voice.start, voice.end);
                    let step = if voice.pitch_env_state > 1e-4 {
                        let ratio = f64::from(
                            (voice.pitch_env_semitones * voice.pitch_env_state / 12.0).exp2(),
                        );
                        voice.pitch_env_state *= voice.pitch_env_coeff;
                        if voice.pitch_env_state <= 1e-4 {
                            voice.pitch_env_state = 0.0;
                        }
                        voice.base_step * ratio
                    } else {
                        voice.base_step
                    };
                    if voice.reverse {
                        voice.pos -= step;
                    } else {
                        voice.pos += step;
                    }
                    s
                };

                let env = advance_envelope(voice, hold_frames, decay_frames, sustain);
                let mut sig = [sample[0] * env, sample[1] * env];
                if filter_on {
                    sig = voice
                        .filter
                        .process(sig, voice_cutoff, filter_q, filter_mode, rate);
                }
                out_frame[0] += sig[0] * voice_gain * pan_l;
                out_frame[1] += sig[1] * voice_gain * pan_r;
            }
        }
    }
}

#[inline]
fn advance_envelope(voice: &mut Voice, hold_frames: u32, decay_frames: u32, sustain: f32) -> f32 {
    let current = voice.env;
    match voice.stage {
        EnvStage::Attack => {
            voice.stage_frames += 1;
            let total = voice.stage_total.max(1);
            if voice.stage_frames >= total {
                voice.env = 1.0;
                voice.stage_frames = 0;
                if hold_frames > 0 {
                    voice.stage = EnvStage::Hold;
                    voice.stage_total = hold_frames;
                } else if decay_frames > 0 && sustain < 0.9999 {
                    voice.stage = EnvStage::Decay;
                    voice.stage_total = decay_frames;
                } else {
                    voice.stage = EnvStage::Sustain;
                }
            } else {
                voice.env = voice.stage_frames as f32 / total as f32;
            }
        }
        EnvStage::Hold => {
            voice.stage_frames += 1;
            if voice.stage_frames >= voice.stage_total {
                voice.stage_frames = 0;
                if decay_frames > 0 && sustain < 0.9999 {
                    voice.stage = EnvStage::Decay;
                    voice.stage_total = decay_frames;
                } else {
                    voice.stage = EnvStage::Sustain;
                    voice.env = sustain.max(if sustain < 0.9999 { sustain } else { 1.0 });
                }
            }
        }
        EnvStage::Decay => {
            voice.stage_frames += 1;
            let total = voice.stage_total.max(1);
            if voice.stage_frames >= total {
                voice.env = sustain;
                voice.stage = EnvStage::Sustain;
                if sustain <= 1e-4 {
                    voice.active = false;
                }
            } else {
                let t = voice.stage_frames as f32 / total as f32;
                // Curva exponencial suave para decaimiento percusivo natural.
                let curve = (1.0 - t) * (1.0 - t);
                voice.env = sustain + (1.0 - sustain) * curve;
            }
        }
        EnvStage::Sustain => {
            if sustain <= 1e-4 {
                voice.active = false;
            }
        }
        EnvStage::Release | EnvStage::ChokeFade => {
            voice.stage_frames += 1;
            let total = voice.stage_total.max(1);
            if voice.stage_frames >= total {
                voice.env = 0.0;
                voice.active = false;
            } else {
                let t = 1.0 - (voice.stage_frames as f32 / total as f32);
                voice.env = voice.stage_start_env * t;
            }
        }
    }
    current
}

#[inline]
fn sample_cubic(frames: &[StereoFrame], pos: f64, start: usize, end: usize) -> StereoFrame {
    let last = end.saturating_sub(1).min(frames.len() - 1);
    let idx = (pos.floor() as isize).clamp(start as isize, last as isize) as usize;
    let frac = (pos - idx as f64).clamp(0.0, 1.0) as f32;
    let i0 = idx.saturating_sub(1).max(start);
    let i1 = idx;
    let i2 = (idx + 1).min(last);
    let i3 = (idx + 2).min(last);
    let p0 = frames[i0];
    let p1 = frames[i1];
    let p2 = frames[i2];
    let p3 = frames[i3];
    [
        catmull_rom(p0[0], p1[0], p2[0], p3[0], frac),
        catmull_rom(p0[1], p1[1], p2[1], p3[1], frac),
    ]
}

#[inline]
fn catmull_rom(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let a2 = -0.5 * y0 + 0.5 * y2;
    let a3 = y1;
    ((a0 * t + a1) * t + a2) * t + a3
}

fn playback_step(source_rate: u32, output_rate: u32, pitch: u8, params: &SamplerParams) -> f64 {
    let sr_ratio = f64::from(source_rate.max(1)) / f64::from(output_rate.max(1));
    let key_delta = if params.keytrack {
        f64::from(pitch) - f64::from(params.get(ROOT))
    } else {
        0.0
    };
    let semitones = key_delta + f64::from(params.get(TUNE));
    if semitones.abs() < 1e-6 {
        sr_ratio
    } else {
        sr_ratio * (semitones / 12.0).exp2()
    }
}

fn effective_start_offset(base: u16, mode: OffsetMode, hit: u64) -> u16 {
    if base == 0 {
        return 0;
    }
    match mode {
        OffsetMode::Fixed => base,
        OffsetMode::EachNote => {
            if hit % 2 == 1 {
                base
            } else {
                0
            }
        }
        OffsetMode::Random => {
            let mixed = hit
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((mixed >> 24) % (u64::from(base) + 1)) as u16
        }
    }
}

#[inline]
fn ms_to_frames(ms: f32, rate: u32) -> u32 {
    if ms <= 0.01 {
        0
    } else {
        ((ms * 0.001) * rate as f32).round() as u32
    }
}

#[inline]
fn db_to_lin(db: f32) -> f32 {
    if db <= -59.9 {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

#[inline]
fn equal_power_pan(pan: f32) -> (f32, f32) {
    if pan.abs() < 1e-5 {
        return (1.0, 1.0);
    }
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let s = std::f32::consts::SQRT_2;
    (angle.cos() * s, angle.sin() * s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_asset(frames: Vec<StereoFrame>, sample_rate: u32) -> Arc<SampleAsset> {
        SampleAsset::from_frames(frames, sample_rate)
    }

    #[test]
    fn bit_exact_hihat_without_block_boundary_glitches() {
        let sr = 48_000u32;
        let orig: Vec<StereoFrame> = (0..4800)
            .map(|i| {
                let s = 0.5 * (std::f32::consts::TAU * 8000.0 * i as f32 / sr as f32).sin();
                [s, s]
            })
            .collect();
        let asset = test_asset(orig.clone(), sr);
        let mut params = SamplerParams::default();
        params.apply_drum_preset();
        params.play_full_sample();
        params.filter = false;
        params.set(ATTACK, 0.0);
        params.set(ROOT, 36.0);

        let mut sampler =
            DrumSampler::prepare(vec![(asset, (1, 127), 0, OffsetMode::Fixed)], params, sr)
                .unwrap();
        sampler.note_on(36, 127);

        // Renderizar en bloques irregulares (igual que render_segment)
        let mut rendered = Vec::new();
        for &chunk in &[73usize, 183, 256, 111, 145, 256, 256, 256] {
            let mut buf = vec![[0.0; 2]; chunk];
            sampler.render(&mut buf);
            rendered.extend_from_slice(&buf);
        }

        for (i, (&got, &expected)) in rendered.iter().zip(orig.iter()).enumerate() {
            assert!(
                (got[0] - expected[0]).abs() < 1e-5,
                "sample mismatch at frame {i}: got={} expected={}",
                got[0],
                expected[0]
            );
        }
    }

    #[test]
    fn choke_retrigger_is_click_free_and_filter_attenuates_highs() {
        let sr = 48_000u32;
        let orig: Vec<StereoFrame> = (0..4800)
            .map(|i| {
                let s = 0.5 * (std::f32::consts::TAU * 8000.0 * i as f32 / sr as f32).sin();
                [s, s]
            })
            .collect();
        let asset = test_asset(orig, sr);

        let mut params = SamplerParams::default();
        params.apply_drum_preset();
        params.play_full_sample();
        params.filter = true;
        params.set(ATTACK, 0.0);
        params.set(CUTOFF, 500.0);
        params.set(ROOT, 36.0);

        let mut sampler =
            DrumSampler::prepare(vec![(asset, (1, 127), 0, OffsetMode::Fixed)], params, sr)
                .unwrap();
        sampler.note_on(36, 127);
        let mut blk = vec![[0.0; 2]; 512];
        sampler.render(&mut blk);
        let rms = (blk.iter().map(|f| f[0] * f[0]).sum::<f32>() / blk.len() as f32).sqrt();
        // 8 kHz a través de un LP a 500 Hz debe atenuarse drásticamente (> 20 dB respecto a 0.35 RMS)
        assert!(rms < 0.03, "expected strong LP attenuation, got rms={rms}");
    }
}
