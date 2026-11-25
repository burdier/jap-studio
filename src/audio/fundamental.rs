//! Fundamental local por autocorrelación normalizada del PCM del clip.
use elevate::StereoFrame;
use realfft::RealFftPlanner;

/// La ventana ya está limitada al tramo visible del clip. Un pico periódico
/// necesita al menos tres ciclos y suficiente energía sobre el silencio.
pub(crate) fn estimate(frames: &[StereoFrame], rate: u32) -> Option<f32> {
    if frames.len() < 24 || rate == 0 {
        return None;
    }
    let power = frames
        .iter()
        .fold([0.0; 2], |[l, r], &[x, y]| [l + x * x, r + y * y]);
    let channel = usize::from(power[1] > power[0]);
    let n = frames.len();
    let mean = frames.iter().map(|frame| frame[channel]).sum::<f32>() / n as f32;
    let size = (2 * n).next_power_of_two();
    let mut planner = RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(size);
    let inverse = planner.plan_fft_inverse(size);
    let mut signal = forward.make_input_vec();
    let mut energy = Vec::with_capacity(n + 1);
    energy.push(0.);
    for (i, frame) in frames.iter().enumerate() {
        let x = frame[channel] - mean;
        signal[i] = x;
        energy.push(energy[i] + x * x);
    }
    if energy[n] < n as f32 * 1e-9 {
        return None;
    }
    let mut bins = forward.make_output_vec();
    forward.process(&mut signal, &mut bins).ok()?;
    for bin in &mut bins {
        *bin = bin.norm_sqr().into();
    }
    inverse.process(&mut bins, &mut signal).ok()?;
    let score = |lag: usize| {
        let denom = energy[n - lag] + (energy[n] - energy[lag]);
        2. * signal[lag] / (size as f32 * denom.max(1e-12))
    };
    let limit = (n / 3).min((rate as usize).div_ceil(24));
    // Buscar el primer pico casi tan claro como el mejor evita confundir
    // múltiplos del período con la fundamental.
    let peaks: Vec<_> = (2..=limit)
        .filter_map(|lag| {
            let (a, b, c) = (score(lag - 1), score(lag), score(lag + 1));
            (b >= 0.85 && b > a && b >= c).then_some((lag, a, b, c))
        })
        .collect();
    let best = peaks.iter().map(|p| p.2).fold(0.0_f32, f32::max);
    let (lag, a, b, c) = peaks.into_iter().find(|p| p.2 >= 0.96 * best)?;
    let shift = (0.5 * (a - c) / (a - 2. * b + c)).clamp(-0.5, 0.5);
    let hz = rate as f32 / (lag as f32 + shift);
    (24.0..=2000.0).contains(&hz).then_some(hz)
}

/// Sugiere una raíz cuando varios tramos con señal coinciden en altura.
pub(crate) fn stable_root(frames: &[StereoFrame], rate: u32) -> Option<(u8, i32)> {
    let width = ((rate as usize * 18) / 100).min(frames.len());
    if width < 24 {
        return None;
    }
    let end = frames.len();
    let offsets = (end - width) / (width / 2).max(1) + 1;
    let count = offsets.min(15);
    let power = |window: &[StereoFrame]| {
        window.iter().map(|[l, r]| l * l + r * r).sum::<f32>() / width as f32
    };
    let windows: Vec<_> = (0..count)
        .map(|i| {
            let start = i * (end - width) / (count - 1).max(1);
            let window = &frames[start..start + width];
            (window, power(window))
        })
        .collect();
    let strongest = windows
        .iter()
        .map(|(_, energy)| *energy)
        .fold(0.0, f32::max);
    let mut pitches = Vec::new();
    let mut audible = 0;
    for (window, energy) in windows {
        if energy < (strongest * 0.03).max(1e-8) {
            continue;
        }
        audible += 1;
        if let Some(hz) = estimate(window, rate) {
            // Un acorde puede repetir a un subarmónico ausente de la señal.
            let phase = std::f32::consts::TAU * hz / rate as f32;
            let mut bins = [[0.0f32; 2]; 2];
            for (i, frame) in window.iter().enumerate() {
                let (sin, cos) = (phase * i as f32).sin_cos();
                for (bin, sample) in bins.iter_mut().zip(frame) {
                    bin[0] += sample * cos;
                    bin[1] += sample * sin;
                }
            }
            let fundamental =
                bins.iter().map(|[re, im]| re * re + im * im).sum::<f32>() / (width as f32).powi(2);
            if fundamental >= 0.06 * energy {
                pitches.push(69. + 12. * (hz / 440.).log2());
            }
        }
    }
    pitches.sort_by(f32::total_cmp);
    let pitch = *pitches.get(pitches.len() / 2)?;
    let agreement = pitches
        .iter()
        .filter(|p| (**p - pitch).abs() <= 0.25)
        .count();
    if agreement * 4 < audible * 3 || (count > 2 && agreement < 2) {
        return None;
    }
    let note = pitch.round();
    (0.0..=127.0)
        .contains(&note)
        .then(|| (note as u8, ((pitch - note) * 100.).round() as i32))
}
