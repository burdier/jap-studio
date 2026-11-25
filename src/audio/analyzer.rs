//! Análisis fuera de RT. El callback solo entrega muestras a una cola acotada.
pub(crate) mod loudness;
pub(crate) mod tap;
use realfft::{RealFftPlanner, RealToComplex, num_complex::Complex32};
use std::{collections::VecDeque, sync::Arc};

pub(crate) const BANDS: usize = 96;
const WINDOW: usize = 2048;
const HISTORY: usize = 128;

struct Frame {
    log: [f32; BANDS],
    linear: [f32; BANDS],
}
pub(crate) struct SpectrumAnalyzer {
    fft: Arc<dyn RealToComplex<f32>>,
    input: [Vec<f32>; 2],
    output: Vec<Complex32>,
    scratch: Vec<Complex32>,
    power: Vec<f32>,
    window: Vec<f32>,
    filled: usize,
    sample_rate: f32,
    bands: [f32; BANDS],
    wave_min: [f32; BANDS],
    wave_max: [f32; BANDS],
    history: VecDeque<Frame>,
    peak: [f32; 2],
    rms: [f32; 2],
}
impl SpectrumAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(WINDOW);
        Self {
            input: [fft.make_input_vec(), fft.make_input_vec()],
            output: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            fft,
            power: vec![0.; WINDOW / 2 + 1],
            window: (0..WINDOW)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (WINDOW - 1) as f32).cos())
                .collect(),
            filled: 0,
            sample_rate: sample_rate.max(1) as f32,
            bands: [0.; BANDS],
            wave_min: [0.; BANDS],
            wave_max: [0.; BANDS],
            history: VecDeque::with_capacity(HISTORY),
            peak: [0.; 2],
            rms: [0.; 2],
        }
    }
    pub fn push_stereo(&mut self, frame: [f32; 2]) {
        for (channel, sample) in frame.into_iter().enumerate() {
            self.input[channel][self.filled] = if sample.is_finite() { sample } else { 0. };
        }
        self.filled += 1;
        if self.filled == WINDOW {
            self.analyze();
            self.filled = 0;
        }
    }
    pub fn bands(&self) -> &[f32; BANDS] {
        &self.bands
    }
    pub fn waveform(&self) -> (&[f32; BANDS], &[f32; BANDS]) {
        (&self.wave_min, &self.wave_max)
    }
    pub fn history(&self, linear: bool) -> impl Iterator<Item = &[f32]> {
        self.history.iter().map(move |frame| {
            if linear {
                frame.linear.as_slice()
            } else {
                frame.log.as_slice()
            }
        })
    }
    pub fn history_capacity(&self) -> usize {
        HISTORY
    }
    pub fn history_seconds(&self) -> f32 {
        HISTORY as f32 * WINDOW as f32 / self.sample_rate
    }
    pub fn upper_hz(&self) -> f32 {
        (self.sample_rate * 0.5).min(20_000.).max(21.)
    }
    pub fn levels(&self) -> ([f32; 2], [f32; 2]) {
        (self.peak, self.rms)
    }
    pub fn silence(&mut self) {
        self.filled = 0;
        self.bands.fill(0.);
        self.wave_min.fill(0.);
        self.wave_max.fill(0.);
        self.peak.fill(0.);
        self.rms.fill(0.);
    }
    pub fn clear(&mut self) {
        self.filled = 0;
        self.bands.fill(0.);
        self.wave_min.fill(0.);
        self.wave_max.fill(0.);
        self.history.clear();
        self.peak.fill(0.);
        self.rms.fill(0.);
    }
    fn analyze(&mut self) {
        for band in 0..BANDS {
            let start = band * WINDOW / BANDS;
            let end = (band + 1) * WINDOW / BANDS;
            self.wave_min[band] = self
                .input
                .iter()
                .flat_map(|v| v[start..end].iter())
                .copied()
                .fold(0., f32::min);
            self.wave_max[band] = self
                .input
                .iter()
                .flat_map(|v| v[start..end].iter())
                .copied()
                .fold(0., f32::max);
        }
        self.power.fill(0.);
        for channel in 0..2 {
            let samples = &mut self.input[channel];
            self.peak[channel] = samples.iter().map(|s| s.abs()).fold(0., f32::max);
            self.rms[channel] = (samples.iter().map(|s| s * s).sum::<f32>() / WINDOW as f32).sqrt();
            for (sample, window) in samples.iter_mut().zip(&self.window) {
                *sample *= window;
            }
            if self
                .fft
                .process_with_scratch(samples, &mut self.output, &mut self.scratch)
                .is_err()
            {
                return;
            }
            // Energía estéreo: una señal en contrafase no desaparece como en una suma mono.
            for (power, value) in self.power.iter_mut().zip(&self.output) {
                *power += value.norm_sqr() * 0.5;
            }
        }
        let upper = self.upper_hz();
        let mut frame = Frame {
            log: [0.; BANDS],
            linear: [0.; BANDS],
        };
        for band in 0..BANDS {
            let a = band as f32 / BANDS as f32;
            let b = (band + 1) as f32 / BANDS as f32;
            frame.log[band] = self.level(20. * (upper / 20.).powf(a), 20. * (upper / 20.).powf(b));
            frame.linear[band] = self.level(upper * a, upper * b);
            self.bands[band] = self.bands[band] * 0.6 + frame.log[band] * 0.4;
        }
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(frame);
    }
    fn level(&self, low: f32, high: f32) -> f32 {
        let bin = |hz: f32| (hz * WINDOW as f32 / self.sample_rate) as usize;
        let start = bin(low).max(1).min(self.power.len() - 1);
        let end = bin(high).max(start + 1).min(self.power.len());
        let peak = self.power[start..end]
            .iter()
            .copied()
            .fold(0., f32::max)
            .sqrt();
        let db = 20. * (peak * 4. / WINDOW as f32).max(1e-6).log10();
        ((db + 80.) / 80.).clamp(0., 1.)
    }
}
