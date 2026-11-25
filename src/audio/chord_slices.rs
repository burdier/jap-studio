//! Detect sustained changes in pitch-class content in source-frame coordinates.
use realfft::RealFftPlanner;

fn similarity(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let power = |v: &[f32; 12]| v.iter().map(|x| x * x).sum::<f32>();
    dot / (power(a) * power(b)).sqrt().max(1e-12)
}

pub fn boundaries(frames: &[[f32; 2]], rate: u32) -> Result<Vec<usize>, String> {
    let size = (rate as usize / 12).next_power_of_two().clamp(1024, 16384);
    let hop = size / 4;
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(size);
    let window: Vec<_> = (0..size)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / size as f32).cos())
        .collect();
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let mut magnitude = vec![0.; output.len()];
    let mut profiles = Vec::new();
    for block in frames.windows(size).step_by(hop) {
        magnitude.fill(0.);
        for ch in 0..2 {
            for (i, x) in input.iter_mut().enumerate() {
                *x = block[i][ch] * window[i];
            }
            fft.process_with_scratch(&mut input, &mut output, &mut scratch)
                .map_err(|e| e.to_string())?;
            for (m, c) in magnitude.iter_mut().zip(&output) {
                *m += c.norm();
            }
        }
        let mut profile = [0.; 12];
        let peak = magnitude.iter().copied().fold(0., f32::max);
        for i in 1..magnitude.len() - 1 {
            let [l, m, r] = [magnitude[i - 1], magnitude[i], magnitude[i + 1]];
            if m <= l || m <= r || m < (peak * 0.07).max(1e-5) {
                continue;
            }
            let offset = 0.5 * (l - r) / (l - 2. * m + r);
            let hz = (i as f32 + offset) * rate as f32 / size as f32;
            if (65.0..=2500.).contains(&hz) {
                let pitch = (69. + 12. * (hz / 440.).log2()).round() as usize;
                profile[pitch % 12] += m;
            }
        }
        let total: f32 = profile.iter().sum();
        if total > 0. {
            for x in &mut profile {
                *x /= total;
            }
        }
        profiles.push(profile);
    }
    let mut candidates = Vec::new();
    for i in 3..profiles.len().saturating_sub(3) {
        let average = |slice: &[[f32; 12]]| {
            let mut v = [0.; 12];
            for row in slice {
                for (x, y) in v.iter_mut().zip(row) {
                    *x += y;
                }
            }
            v
        };
        let left = average(&profiles[i - 3..i]);
        let right = average(&profiles[i..i + 3]);
        if left.iter().sum::<f32>() < 2.5 || right.iter().sum::<f32>() < 2.5 {
            continue;
        }
        let novelty = 1. - similarity(&left, &right);
        if novelty > 0.20 {
            candidates.push((novelty, i * hop + size / 2 - hop / 2));
        }
    }
    candidates.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
    let min = rate as usize / 7;
    let mut cuts = std::collections::BTreeSet::from([0, frames.len()]);
    for (_, i) in candidates {
        if cuts.range(..i).next_back().is_some_and(|x| i - x >= min)
            && cuts.range(i..).next().is_some_and(|x| x - i >= min)
        {
            cuts.insert(i);
        }
    }
    if cuts.len() < 3 {
        return Err("Sin cambios de acorde claros en este rango".into());
    }
    Ok(cuts.into_iter().collect())
}
