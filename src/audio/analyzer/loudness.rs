//! Medición EBU R128 / ITU-R BS.1770-4 estéreo mediante `ebur128` (True Peak 4× + LUFS M/S/I + LRA).
use ebur128::{EbuR128, Mode};
use elevate::StereoFrame;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MasterTelemetry {
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub true_peak_hold: [f32; 2],
    pub momentary: Option<f64>,
    pub short_term: Option<f64>,
    pub integrated: Option<f64>,
    pub lra: Option<f64>,
}

pub(crate) struct Loudness {
    ebu: EbuR128,
    rate: u32,
    peak: [f32; 2],
    rms: [f32; 2],
    decay_per_frame: f32,
}

impl Loudness {
    pub fn new(rate: u32) -> Self {
        let safe_rate = rate.max(8_000);
        let mode = Mode::M | Mode::S | Mode::I | Mode::LRA | Mode::TRUE_PEAK | Mode::SAMPLE_PEAK;
        let ebu = EbuR128::new(2, safe_rate, mode).expect("EbuR128 estéreo válido");
        Self {
            ebu,
            rate: safe_rate,
            peak: [0.0; 2],
            rms: [0.0; 2],
            decay_per_frame: 1.0 / (safe_rate as f32 * 0.35),
        }
    }

    pub fn push(&mut self, frame: StereoFrame) {
        self.push_block(&[frame]);
    }

    pub fn push_block(&mut self, frames: &[StereoFrame]) {
        if frames.is_empty() {
            return;
        }
        let mut interleaved = [0.0_f32; 512];
        let mut offset = 0;
        while offset < frames.len() {
            let chunk_frames = (frames.len() - offset).min(256);
            let mut sq = [0.0_f32; 2];
            for (i, frame) in frames[offset..offset + chunk_frames].iter().enumerate() {
                let l = if frame[0].is_finite() { frame[0] } else { 0.0 };
                let r = if frame[1].is_finite() { frame[1] } else { 0.0 };
                interleaved[i * 2] = l;
                interleaved[i * 2 + 1] = r;
                sq[0] += l * l;
                sq[1] += r * r;
            }
            let _ = self.ebu.add_frames_f32(&interleaved[..chunk_frames * 2]);
            let decay = (1.0 - self.decay_per_frame * chunk_frames as f32).max(0.0);
            for ch in 0..2 {
                let block_tp = self.ebu.prev_true_peak(ch as u32).unwrap_or(0.0) as f32;
                let block_rms = (sq[ch] / chunk_frames as f32).sqrt();
                self.peak[ch] = block_tp.max(self.peak[ch] * decay);
                self.rms[ch] = block_rms.max(self.rms[ch] * decay);
            }
            offset += chunk_frames;
        }
    }

    pub fn reset(&mut self) {
        self.ebu.reset();
        self.peak = [0.0; 2];
        self.rms = [0.0; 2];
    }

    pub fn levels(&self) -> [Option<f64>; 2] {
        [
            finite_lufs(self.ebu.loudness_momentary().ok()),
            finite_lufs(self.ebu.loudness_shortterm().ok()),
        ]
    }

    pub fn telemetry(&self) -> MasterTelemetry {
        let hold_l = self.ebu.true_peak(0).unwrap_or(0.0) as f32;
        let hold_r = self.ebu.true_peak(1).unwrap_or(0.0) as f32;
        MasterTelemetry {
            peak: self.peak,
            rms: self.rms,
            true_peak_hold: [hold_l, hold_r],
            momentary: finite_lufs(self.ebu.loudness_momentary().ok()),
            short_term: finite_lufs(self.ebu.loudness_shortterm().ok()),
            integrated: finite_lufs(self.ebu.loudness_global().ok()),
            lra: self
                .ebu
                .loudness_range()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0),
        }
    }

    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }
}

fn finite_lufs(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v > -120.0)
}
