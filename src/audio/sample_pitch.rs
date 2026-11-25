//! Transposición offline con bloqueo de fase alrededor de los picos espectrales.
//! El salto de análisis y el de síntesis son iguales para conservar los frames del clip.
use elevate::StereoFrame;
use realfft::{RealFftPlanner, num_complex::Complex32};
use std::f32::consts::{PI, TAU};

pub(super) fn transpose(
    input: &[StereoFrame],
    rate: u32,
    cents: i16,
) -> Result<Vec<StereoFrame>, String> {
    let ratio = 2f32.powf(f32::from(cents) / 1200.);
    // ~60–95 ms para resolver graves; el relleno centrado compensa la latencia en los bordes.
    let n = ((rate as usize / 14).next_power_of_two()).clamp(512, 8192);
    let hop = n / 8;
    let bins = n / 2 + 1;
    let mut planner = RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(n);
    let inverse = planner.plan_fft_inverse(n);
    let window: Vec<_> = (0..n)
        .map(|i| 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos())
        .collect();
    let mut time = vec![0.; n];
    let mut scratch =
        vec![Complex32::default(); forward.get_scratch_len().max(inverse.get_scratch_len())];
    let mut spectra = [forward.make_output_vec(), forward.make_output_vec()];
    let mut previous = spectra.clone();
    let mut shifted = inverse.make_input_vec();
    let mut power = vec![0.; bins];
    let mut phase = vec![0f32; bins];
    let mut shifts = vec![0isize; bins];
    let mut peaks = Vec::with_capacity(bins);
    let mut output = vec![[0.; 2]; input.len()];
    // Hann² suma 3 con un salto de N/8, incluida la extensión por ceros de los extremos.
    let scale = 1. / (3. * n as f32);
    for end in (hop..input.len() + n).step_by(hop) {
        let start = end as isize - n as isize;
        let first = (-start).max(0) as usize;
        let last = n.min((input.len() as isize - start).max(0) as usize);
        for (ch, spec) in spectra.iter_mut().enumerate() {
            time.fill(0.);
            for j in first..last {
                time[j] = window[j] * input[(start + j as isize) as usize][ch]
            }
            forward
                .process_with_scratch(&mut time, spec, &mut scratch)
                .map_err(|e| e.to_string())?;
        }
        peaks.clear();
        for k in 0..bins {
            let l = spectra[0][k];
            let r = spectra[1][k];
            power[k] = l.norm_sqr() + r.norm_sqr();
            let cross = l * previous[0][k].conj() + r * previous[1][k].conj();
            // Fase esperada reducida antes de restarla para mantener precisión en agudos.
            let expected = TAU * (k % 8) as f32 / 8.;
            let deviation = if cross.norm_sqr() > 1e-20 {
                (cross.arg() - expected + PI).rem_euclid(TAU) - PI
            } else {
                0.
            };
            let omega = TAU * k as f32 / n as f32 + deviation / hop as f32;
            let delta = omega * (ratio - 1.);
            phase[k] = (phase[k] + delta * hop as f32).rem_euclid(TAU);
            shifts[k] = (delta * n as f32 / TAU).round() as isize;
        }
        for k in 0..bins {
            if power[k] > 1e-18
                && (k == 0 || power[k] > power[k - 1])
                && (k + 1 == bins || power[k] >= power[k + 1])
            {
                peaks.push(k)
            }
        }
        for (ch, spec) in spectra.iter().enumerate() {
            shifted.fill(Complex32::default());
            for (i, &k) in peaks.iter().enumerate() {
                let left = if i == 0 {
                    0
                } else {
                    (peaks[i - 1] + k) / 2 + 1
                };
                let right = peaks.get(i + 1).map_or(bins, |&next| (k + next) / 2 + 1);
                let rotation = Complex32::from_polar(1., phase[k]);
                for bin in left..right {
                    let target = bin as isize + shifts[k];
                    if (0..bins as isize).contains(&target) {
                        shifted[target as usize] += spec[bin] * rotation
                    }
                }
            }
            shifted[0].im = 0.;
            shifted[bins - 1].im = 0.;
            inverse
                .process_with_scratch(&mut shifted, &mut time, &mut scratch)
                .map_err(|e| e.to_string())?;
            for j in first..last {
                output[(start + j as isize) as usize][ch] += time[j] * window[j] * scale;
            }
        }
        std::mem::swap(&mut spectra, &mut previous);
    }
    Ok(output)
}
