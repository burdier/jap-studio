//! Motor de Warp / Time-Stretch offline estilo Ableton Live (100% Rust puro):
//! - `RePitch`: remuestreo cúbico Catmull-Rom de alta calidad (vinilo/cinta, cero artefactos).
//! - `Beats`: detección de transientes + preservación de ataques + estiramiento WSOLA entre golpes.
//! - `Complex`: Phase Vocoder estéreo con bloqueo de fase sobre `realfft` (mantiene el tono).
use elevate::{StereoFrame, musical::sample_edit::WarpMode};
use realfft::{RealFftPlanner, num_complex::Complex32};
use std::f32::consts::{PI, TAU};

/// Estira o encoge `input` por `stretch_factor` (`> 1.0` alarga, `< 1.0` acorta).
pub(super) fn warp(
    input: &[StereoFrame],
    rate: u32,
    mode: WarpMode,
    stretch_factor: f64,
) -> Result<Vec<StereoFrame>, String> {
    if input.is_empty() || mode == WarpMode::Off || (stretch_factor - 1.0).abs() <= 1e-4 {
        return Ok(input.to_vec());
    }
    let factor = stretch_factor.clamp(0.25, 4.0);
    let target_len = ((input.len() as f64 * factor).round() as usize).max(1);
    if target_len > 60 * 192_000 {
        return Err("El clip estirado supera el límite máximo de duración".into());
    }
    match mode {
        WarpMode::Off => Ok(input.to_vec()),
        WarpMode::RePitch => Ok(warp_repitch(input, target_len)),
        WarpMode::Beats => Ok(warp_beats(input, rate, factor, target_len)),
        WarpMode::Complex => warp_complex(input, rate, factor, target_len),
    }
}

/// Estima el BPM más probable de un clip a partir de su duración asumiendo 1, 2, 4, 8 o 16 compases.
pub(crate) fn estimate_loop_bpm(frames: usize, sample_rate: u32, hint_bpm: f64) -> f32 {
    if frames == 0 || sample_rate == 0 {
        return hint_bpm.clamp(20.0, 300.0) as f32;
    }
    let secs = frames as f64 / f64::from(sample_rate);
    if secs < 0.1 {
        return hint_bpm.clamp(20.0, 300.0) as f32;
    }
    // Probar potencias de 2 en beats (1, 2, 4, 8, 16, 32, 64) y elegir la más cercana al hint.
    let mut best_bpm = hint_bpm.clamp(60.0, 200.0);
    let mut best_dist = f64::MAX;
    for beats in [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0] {
        let candidate = beats * 60.0 / secs;
        if (60.0..=200.0).contains(&candidate) {
            let dist = (candidate - hint_bpm).abs();
            if dist < best_dist {
                best_dist = dist;
                best_bpm = candidate;
            }
        }
    }
    (best_bpm * 2.0).round() as f32 / 2.0
}

/// Modo Re-Pitch: interpolación cúbica Catmull-Rom de 4 puntos.
fn warp_repitch(input: &[StereoFrame], target_len: usize) -> Vec<StereoFrame> {
    let n = input.len();
    if n == 1 {
        return vec![input[0]; target_len];
    }
    let mut out = vec![[0.0_f32; 2]; target_len];
    let step = (n - 1) as f64 / (target_len.saturating_sub(1).max(1)) as f64;

    let sample_at = |idx: isize, ch: usize| -> f32 {
        let clamped = idx.clamp(0, (n - 1) as isize) as usize;
        input[clamped][ch]
    };

    for (i, dst) in out.iter_mut().enumerate() {
        let pos = i as f64 * step;
        let base = pos.floor() as isize;
        let t = (pos - base as f64) as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        for ch in 0..2 {
            let y0 = sample_at(base - 1, ch);
            let y1 = sample_at(base, ch);
            let y2 = sample_at(base + 1, ch);
            let y3 = sample_at(base + 2, ch);
            // Catmull-Rom spline
            let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
            let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let a2 = -0.5 * y0 + 0.5 * y2;
            let a3 = y1;
            dst[ch] = a0 * t3 + a1 * t2 + a2 * t + a3;
        }
    }
    out
}

/// Detecta inicios de golpes (transientes) comparando envolvente rápida vs lenta.
fn detect_transients(input: &[StereoFrame], rate: u32) -> Vec<usize> {
    let sr = rate.max(8_000) as f32;
    let fast_a = (-1.0 / (0.002 * sr)).exp(); // 2 ms
    let slow_a = (-1.0 / (0.035 * sr)).exp(); // 35 ms
    let min_gap = ((0.040 * sr).round() as usize).max(256); // mínimo 40 ms entre golpes

    let mut fast = 0.0_f32;
    let mut slow = 1e-4_f32;
    let mut cuts = vec![0usize];
    let mut last_cut = 0usize;

    for (i, frame) in input.iter().enumerate() {
        let energy = frame[0].abs().max(frame[1].abs());
        fast = energy + (fast - energy) * fast_a;
        slow = energy + (slow - energy) * slow_a;
        if i >= last_cut + min_gap && fast > 0.025 && fast > slow * 2.15 {
            // Retroceder ~1.5 ms para capturar el inicio exacto del ataque del golpe.
            let pre = ((0.0015 * sr) as usize).min(i - last_cut);
            let cut = i - pre;
            if cut > last_cut + min_gap / 2 {
                cuts.push(cut);
                last_cut = cut;
            }
        }
    }
    if *cuts.last().unwrap_or(&0) < input.len() {
        cuts.push(input.len());
    }
    cuts
}

/// Modo Beats: preserva los transientes intactos en su nueva posición temporal y
/// rellena o recorta suavemente el cuerpo/cola entre golpes (sin perder pegada).
fn warp_beats(
    input: &[StereoFrame],
    rate: u32,
    factor: f64,
    target_len: usize,
) -> Vec<StereoFrame> {
    let sr = rate.max(8_000) as usize;
    let cuts = detect_transients(input, rate);
    let mut out = vec![[0.0_f32; 2]; target_len];
    let xfade = (sr / 160).clamp(32, 512); // ~6 ms crossfade anti-clics

    for pair in cuts.windows(2) {
        let src_start = pair[0];
        let src_end = pair[1];
        let src_len = src_end.saturating_sub(src_start);
        if src_len == 0 {
            continue;
        }
        let dst_start = ((src_start as f64 * factor).round() as usize).min(target_len);
        let dst_end = ((src_end as f64 * factor).round() as usize).min(target_len);
        let dst_len = dst_end.saturating_sub(dst_start);
        if dst_len == 0 {
            continue;
        }

        if dst_len <= src_len {
            // Acortando (BPM más rápido): copiamos el golpe intacto y hacemos micro fade-out al final.
            let fade = xfade.min(dst_len / 3);
            for i in 0..dst_len {
                let mut sample = input[src_start + i];
                if fade > 0 && i >= dst_len - fade {
                    let g = (dst_len - i) as f32 / fade as f32;
                    sample[0] *= g;
                    sample[1] *= g;
                }
                out[dst_start + i] = sample;
            }
        } else {
            // Alargando (BPM más lento): golpe intacto + cola extendida por bucle ping-pong suavizado.
            let copy_len = src_len.min(dst_len);
            out[dst_start..dst_start + copy_len]
                .copy_from_slice(&input[src_start..src_start + copy_len]);

            let tail_needed = dst_len - copy_len;
            // Usar el último tercio del golpe como cola resonante (evita repetir el ataque del kick).
            let tail_src_len = (src_len / 2).clamp(1, (sr / 8).max(64));
            let tail_base = src_end - tail_src_len;
            let fade = xfade.min(tail_src_len / 2).max(1);

            for i in 0..tail_needed {
                // Lectura ping-pong en la cola para que nunca haya salto de amplitud
                let cycle = tail_src_len.saturating_sub(1).max(1);
                let phase = i % (cycle * 2);
                let offset = if phase < cycle {
                    cycle - phase
                } else {
                    phase - cycle
                };
                let src_idx = (tail_base + offset).min(src_end - 1);
                // Decaimiento natural suave en el tramo extendido (estilo Ableton Beats 1->)
                let decay = (1.0 - (i as f32 / (tail_needed as f32 + fade as f32)) * 0.85).max(0.0);
                let mut sample = [input[src_idx][0] * decay, input[src_idx][1] * decay];
                // Crossfade en la costura inicial de la cola
                if i < fade && copy_len >= fade {
                    let t = i as f32 / fade as f32;
                    let orig = input[src_end - 1];
                    sample[0] = orig[0] * (1.0 - t) + sample[0] * t;
                    sample[1] = orig[1] * (1.0 - t) + sample[1] * t;
                }
                out[dst_start + copy_len + i] = sample;
            }
        }
    }
    out
}

/// Modo Complex: Phase Vocoder estéreo con bloqueo de fase alrededor de los picos espectrales.
fn warp_complex(
    input: &[StereoFrame],
    rate: u32,
    factor: f64,
    target_len: usize,
) -> Result<Vec<StereoFrame>, String> {
    let n = ((rate as usize / 18).next_power_of_two()).clamp(1024, 4096);
    let hop_s = n / 4; // Salto de síntesis constante (Hann² suma constante con N/4)
    let hop_a_f = (hop_s as f64) / factor;
    let bins = n / 2 + 1;

    let mut planner = RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(n);
    let inverse = planner.plan_fft_inverse(n);
    let window: Vec<f32> = (0..n)
        .map(|i| 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos())
        .collect();

    let mut time = vec![0.0_f32; n];
    let mut scratch =
        vec![Complex32::default(); forward.get_scratch_len().max(inverse.get_scratch_len())];
    let mut spectra = [forward.make_output_vec(), forward.make_output_vec()];
    let mut prev_spectra = spectra.clone();
    let mut synth = inverse.make_input_vec();
    let mut synth_phase = vec![0.0_f32; bins];
    let mut power = vec![0.0_f32; bins];
    let mut peaks = Vec::with_capacity(bins);
    let mut output = vec![[0.0_f32; 2]; target_len];
    let mut norm = vec![0.0_f32; target_len];

    let total_steps = (target_len + n) / hop_s + 1;
    let hop_a_eff = hop_a_f as f32;

    for step in 0..total_steps {
        let out_start = step as isize * hop_s as isize - (n / 2) as isize;
        let in_start = (step as f64 * hop_a_f).round() as isize - (n / 2) as isize;

        for (ch, spec) in spectra.iter_mut().enumerate() {
            time.fill(0.0);
            for j in 0..n {
                let idx = in_start + j as isize;
                if idx >= 0 && (idx as usize) < input.len() {
                    time[j] = window[j] * input[idx as usize][ch];
                }
            }
            forward
                .process_with_scratch(&mut time, spec, &mut scratch)
                .map_err(|e| e.to_string())?;
        }

        if step == 0 {
            for k in 0..bins {
                let sum = spectra[0][k] + spectra[1][k];
                synth_phase[k] = sum.arg();
            }
        } else {
            peaks.clear();
            for k in 0..bins {
                let l = spectra[0][k];
                let r = spectra[1][k];
                power[k] = l.norm_sqr() + r.norm_sqr();
                let cross = l * prev_spectra[0][k].conj() + r * prev_spectra[1][k].conj();
                let expected = TAU * k as f32 * hop_a_eff / n as f32;
                let dev = if cross.norm_sqr() > 1e-20 {
                    (cross.arg() - expected + PI).rem_euclid(TAU) - PI
                } else {
                    0.0
                };
                let true_freq = TAU * k as f32 / n as f32 + dev / hop_a_eff.max(1.0);
                synth_phase[k] = (synth_phase[k] + true_freq * hop_s as f32).rem_euclid(TAU);
            }
            for k in 0..bins {
                if power[k] > 1e-18
                    && (k == 0 || power[k] > power[k - 1])
                    && (k + 1 == bins || power[k] >= power[k + 1])
                {
                    peaks.push(k);
                }
            }
        }

        for (ch, spec) in spectra.iter().enumerate() {
            if step == 0 || peaks.is_empty() {
                synth.copy_from_slice(spec);
            } else {
                synth.fill(Complex32::default());
                for (i, &k) in peaks.iter().enumerate() {
                    let left = if i == 0 {
                        0
                    } else {
                        (peaks[i - 1] + k) / 2 + 1
                    };
                    let right = peaks.get(i + 1).map_or(bins, |&next| (k + next) / 2 + 1);
                    let sum_peak = spectra[0][k] + spectra[1][k];
                    let delta_phase = synth_phase[k] - sum_peak.arg();
                    let rot = Complex32::from_polar(1.0, delta_phase);
                    for bin in left..right {
                        synth[bin] = spec[bin] * rot;
                    }
                }
            }
            synth[0].im = 0.0;
            synth[bins - 1].im = 0.0;
            inverse
                .process_with_scratch(&mut synth, &mut time, &mut scratch)
                .map_err(|e| e.to_string())?;
            let inv_n = 1.0 / n as f32;
            for j in 0..n {
                let idx = out_start + j as isize;
                if idx >= 0 && (idx as usize) < target_len {
                    let w = window[j];
                    output[idx as usize][ch] += time[j] * inv_n * w;
                    if ch == 0 {
                        norm[idx as usize] += w * w;
                    }
                }
            }
        }
        std::mem::swap(&mut spectra, &mut prev_spectra);
    }

    for (frame, &w2) in output.iter_mut().zip(&norm) {
        if w2 > 1e-4 {
            let inv = 1.0 / w2;
            frame[0] *= inv;
            frame[1] *= inv;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_modes_resize_audio_and_preserve_signal_integrity() {
        let rate = 48_000;
        // Generar señal con 4 golpes (transientes claros cada 12000 muestras = 0.25s = 120 BPM)
        let mut drums = vec![[0.0_f32; 2]; 48_000];
        for hit in 0..4 {
            let base = hit * 12_000;
            for j in 0..2400 {
                let env = (-(j as f32) / 600.0).exp();
                let s = (j as f32 * 0.08).sin() * env * 0.9;
                drums[base + j] = [s, s];
            }
        }

        // 1. Re-Pitch a 1.5x más largo
        let rp = warp(&drums, rate, WarpMode::RePitch, 1.5).unwrap();
        assert_eq!(rp.len(), 72_000);
        assert!(rp.iter().all(|f| f[0].is_finite() && f[1].is_finite()));

        // 2. Beats al doble de duración (factor 2.0): cada golpe mantiene su pegada en su nueva posición
        let bt = warp(&drums, rate, WarpMode::Beats, 2.0).unwrap();
        assert_eq!(bt.len(), 96_000);
        let hit2_peak = bt[23_800..24_600]
            .iter()
            .map(|f| f[0].abs())
            .fold(0.0_f32, f32::max);
        assert!(
            hit2_peak > 0.6,
            "golpe #2 debía conservar su pegada en Beats, obt {hit2_peak}"
        );

        // 3. Complex sobre un tono continuo 440 Hz estirado 1.25x
        let tone: Vec<StereoFrame> = (0..24_000)
            .map(|i| {
                let s = (TAU * 440.0 * i as f32 / rate as f32).sin() * 0.5;
                [s, s]
            })
            .collect();
        let cx = warp(&tone, rate, WarpMode::Complex, 1.25).unwrap();
        assert_eq!(cx.len(), 30_000);
        let mid_rms =
            (cx[10_000..20_000].iter().map(|f| f[0] * f[0]).sum::<f32>() / 10_000.0).sqrt();
        assert!(
            mid_rms > 0.25 && mid_rms < 0.55,
            "Complex debía mantener energía estable del tono, obt {mid_rms}"
        );
    }
}
