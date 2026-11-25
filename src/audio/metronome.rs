//! Metrónomo sample-accurate y pre-cuenta (count-in) en el hilo de audio.
//! Sin GPUI, sin reservas en el callback y sin contaminar la medición LUFS del máster.
use elevate::{StereoFrame, musical::time::Meter};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Estado compartido lock-free entre la interfaz y el callback de audio.
pub(crate) struct MetronomeControl {
    enabled: AtomicBool,
    gain_bits: AtomicU32,
    count_in_bars: AtomicU32,
    count_in_remaining: AtomicU32,
    pulse_seq: AtomicU32,
    pulse_accent: AtomicBool,
    keep_running: AtomicBool,
}

impl Default for MetronomeControl {
    fn default() -> Self {
        Self::new()
    }
}

impl MetronomeControl {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            gain_bits: AtomicU32::new(0.75_f32.to_bits()),
            count_in_bars: AtomicU32::new(1),
            count_in_remaining: AtomicU32::new(0),
            pulse_seq: AtomicU32::new(0),
            pulse_accent: AtomicBool::new(false),
            keep_running: AtomicBool::new(false),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Release);
    }

    pub fn toggle(&self) -> bool {
        let next = !self.is_enabled();
        self.set_enabled(next);
        next
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain_bits.load(Ordering::Acquire)).clamp(0.0, 1.5)
    }

    pub fn set_gain(&self, gain: f32) {
        let clamped = if gain.is_finite() {
            gain.clamp(0.0, 1.5)
        } else {
            0.75
        };
        self.gain_bits.store(clamped.to_bits(), Ordering::Release);
    }

    pub fn count_in_bars(&self) -> u32 {
        self.count_in_bars.load(Ordering::Acquire).min(2)
    }

    pub fn set_count_in_bars(&self, bars: u32) {
        self.count_in_bars.store(bars.min(2), Ordering::Release);
    }

    pub fn cycle_count_in(&self) -> u32 {
        let next = match self.count_in_bars() {
            0 => 1,
            1 => 2,
            _ => 0,
        };
        self.set_count_in_bars(next);
        next
    }

    pub fn count_in_remaining(&self) -> u32 {
        self.count_in_remaining.load(Ordering::Acquire)
    }

    pub fn set_count_in_remaining(&self, beats: u32) {
        self.count_in_remaining.store(beats, Ordering::Release);
    }

    pub fn clear_count_in(&self) {
        self.count_in_remaining.store(0, Ordering::Release);
    }

    pub fn keep_running(&self) -> bool {
        self.keep_running.load(Ordering::Acquire)
    }

    pub fn set_keep_running(&self, keep: bool) {
        self.keep_running.store(keep, Ordering::Release);
    }

    pub fn pulse_state(&self) -> (u32, bool) {
        (
            self.pulse_seq.load(Ordering::Acquire),
            self.pulse_accent.load(Ordering::Acquire),
        )
    }

    fn notify_pulse(&self, accent: bool) {
        self.pulse_accent.store(accent, Ordering::Release);
        self.pulse_seq.fetch_add(1, Ordering::Release);
    }
}

/// Generador RT de clic tipo woodblock de estudio + seguimiento exacto de pulsos.
pub(crate) struct MetronomeSynth {
    sample_rate: f32,
    frames_per_pulse: f64,
    pulses_per_bar: u64,
    /// Pre-cuenta pendiente antes de arrancar el reloj del Arrangement.
    count_in_total_frames: u64,
    count_in_total_pulses: u64,
    count_in_elapsed: u64,
    last_count_in_pulse: Option<u64>,
    /// Último índice de pulso disparado en el reloj del Arrangement.
    last_transport_pulse: Option<u64>,
    last_transport_frame: Option<u64>,
    /// Estado de la voz activa del woodblock (cero allocaciones).
    voice_sample: u32,
    voice_len: u32,
    voice_accent: bool,
    phase1: f32,
    phase2: f32,
}

impl MetronomeSynth {
    pub fn new(sample_rate: u32, beats_per_second: f64, meter: Meter, count_in_bars: u32) -> Self {
        let rate = sample_rate.max(1);
        let bps = if beats_per_second.is_finite() && beats_per_second > 0.0 {
            beats_per_second
        } else {
            2.0
        };
        let pulses_per_bar = u64::from(meter.numerator()).max(1);
        let beat_unit_quarters = 4.0 / f64::from(meter.denominator().max(1));
        let frames_per_pulse = ((f64::from(rate) / bps) * beat_unit_quarters).max(1.0);
        let count_in_total_pulses = u64::from(count_in_bars.min(2)) * pulses_per_bar;
        let count_in_total_frames =
            (count_in_total_pulses as f64 * frames_per_pulse).round() as u64;
        Self {
            sample_rate: rate as f32,
            frames_per_pulse,
            pulses_per_bar,
            count_in_total_frames,
            count_in_total_pulses,
            count_in_elapsed: 0,
            last_count_in_pulse: None,
            last_transport_pulse: None,
            last_transport_frame: None,
            voice_sample: 0,
            voice_len: 0,
            voice_accent: false,
            phase1: 0.0,
            phase2: 0.0,
        }
    }

    pub fn in_count_in(&self) -> bool {
        self.count_in_elapsed < self.count_in_total_frames
    }

    /// Actualiza el espaciado de pulsos en vivo sin reiniciar el stream de audio.
    pub fn set_bpm(&mut self, bpm: f64, meter: Meter) {
        if !bpm.is_finite() || bpm <= 0.0 {
            return;
        }
        let bps = bpm / 60.0;
        let beat_unit_quarters = 4.0 / f64::from(meter.denominator().max(1));
        self.frames_per_pulse = ((f64::from(self.sample_rate) / bps) * beat_unit_quarters).max(1.0);
        self.last_transport_pulse = None;
    }

    /// Consume la porción del bloque correspondiente a pre-cuenta y devuelve cuántos frames ocupó.
    /// Durante la pre-cuenta el clic suena aunque el metrónomo normal esté apagado.
    pub fn render_count_in(
        &mut self,
        output: &mut [StereoFrame],
        ctrl: &MetronomeControl,
    ) -> usize {
        if !self.in_count_in() {
            ctrl.set_count_in_remaining(0);
            return 0;
        }
        let remaining_frames = self.count_in_total_frames - self.count_in_elapsed;
        let count = (output.len() as u64).min(remaining_frames) as usize;
        let gain = ctrl.gain();
        for frame in &mut output[..count] {
            let at = self.count_in_elapsed;
            let pulse = (at as f64 / self.frames_per_pulse).floor() as u64;
            if self.last_count_in_pulse != Some(pulse) && pulse < self.count_in_total_pulses {
                self.last_count_in_pulse = Some(pulse);
                let accent = pulse % self.pulses_per_bar == 0;
                self.trigger(accent);
                ctrl.notify_pulse(accent);
                let left = self.count_in_total_pulses.saturating_sub(pulse) as u32;
                ctrl.set_count_in_remaining(left);
            }
            let s = self.next_sample() * gain;
            frame[0] = (frame[0] + s).clamp(-1.0, 1.0);
            frame[1] = (frame[1] + s).clamp(-1.0, 1.0);
            self.count_in_elapsed += 1;
        }
        if !self.in_count_in() {
            ctrl.set_count_in_remaining(0);
        }
        count
    }

    /// Suma el clic del metrónomo sobre un segmento contiguo del reloj del Arrangement `[start_frame, start_frame + frames.len())`.
    pub fn mix_segment(
        &mut self,
        start_frame: u64,
        frames: &mut [StereoFrame],
        ctrl: &MetronomeControl,
    ) {
        let enabled = ctrl.is_enabled();
        let gain = if enabled { ctrl.gain() } else { 0.0 };
        let jumped = self
            .last_transport_frame
            .is_some_and(|prev| prev != start_frame);

        for (offset, frame) in frames.iter_mut().enumerate() {
            let at = start_frame.saturating_add(offset as u64);
            let pulse = (at as f64 / self.frames_per_pulse).floor() as u64;
            let should_trigger = if offset == 0 && jumped {
                // Tras un seek o salto de loop, disparar solo si aterrizamos al borde de un pulso
                let pulse_start = (pulse as f64 * self.frames_per_pulse).round() as u64;
                at.abs_diff(pulse_start) <= 1
            } else {
                match self.last_transport_pulse {
                    None => {
                        let pulse_start = (pulse as f64 * self.frames_per_pulse).round() as u64;
                        at.abs_diff(pulse_start) <= 1
                    }
                    Some(prev) => pulse != prev,
                }
            };
            self.last_transport_pulse = Some(pulse);
            if should_trigger && enabled {
                let accent = pulse % self.pulses_per_bar == 0;
                self.trigger(accent);
                ctrl.notify_pulse(accent);
            }
            let sample = self.next_sample() * gain;
            if sample != 0.0 {
                frame[0] = (frame[0] + sample).clamp(-1.0, 1.0);
                frame[1] = (frame[1] + sample).clamp(-1.0, 1.0);
            }
        }
        self.last_transport_frame = Some(start_frame.saturating_add(frames.len() as u64));
    }

    fn trigger(&mut self, accent: bool) {
        self.voice_accent = accent;
        self.voice_sample = 0;
        let ms = if accent { 24.0 } else { 18.0 };
        self.voice_len = ((self.sample_rate * ms) / 1000.0).round().max(16.0) as u32;
        self.phase1 = 0.0;
        self.phase2 = 0.0;
    }

    fn next_sample(&mut self) -> f32 {
        if self.voice_sample >= self.voice_len {
            return 0.0;
        }
        let n = self.voice_sample as f32;
        let sr = self.sample_rate;
        let (f1, f2, tau_s, level) = if self.voice_accent {
            (1520.0_f32, 3040.0_f32, 0.0065_f32, 0.72_f32)
        } else {
            (1040.0_f32, 2080.0_f32, 0.0052_f32, 0.48_f32)
        };
        let attack_samples = (sr * 0.0005).max(1.0);
        let attack = (n / attack_samples).min(1.0);
        let decay = (-(n / sr) / tau_s).exp();
        let env = attack * decay * level;

        let out = (self.phase1.sin() * 0.80 + self.phase2.sin() * 0.20) * env;
        self.phase1 = (self.phase1 + TAU * f1 / sr) % TAU;
        self.phase2 = (self.phase2 + TAU * f2 / sr) % TAU;
        self.voice_sample += 1;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_in_renders_exact_bar_frames_across_arbitrary_block_sizes() {
        let ctrl = MetronomeControl::new();
        let meter = Meter::new(4, 4).unwrap();
        let mut synth = MetronomeSynth::new(48_000, 2.0, meter, 1); // 120 BPM = 24000 frames/beat -> 96000 frames/bar
        assert!(synth.in_count_in());

        let mut total_consumed = 0usize;
        let mut peak = 0.0_f32;
        let mut block = [[0.0_f32; 2]; 73];
        while synth.in_count_in() {
            block.fill([0.0; 2]);
            let consumed = synth.render_count_in(&mut block, &ctrl);
            assert!(consumed > 0);
            for &[l, r] in &block[..consumed] {
                assert!(l.is_finite() && r.is_finite());
                peak = peak.max(l.abs()).max(r.abs());
            }
            total_consumed += consumed;
        }
        assert_eq!(total_consumed, 96_000);
        assert_eq!(ctrl.count_in_remaining(), 0);
        assert_eq!(ctrl.pulse_state().0, 4);
        assert!(peak > 0.2 && peak <= 1.0);
    }

    #[test]
    fn transport_metronome_accents_downbeats_in_3_4() {
        let ctrl = MetronomeControl::new();
        ctrl.set_enabled(true);
        let meter = Meter::new(3, 4).unwrap();
        let mut synth = MetronomeSynth::new(48_000, 2.0, meter, 0);
        let mut block = [[0.0_f32; 2]; 256];
        let mut accents = Vec::new();
        let mut last_seq = 0;

        // 2 compases exactos de 3/4 a 120 BPM = 6 negras = [0, 144 000) frames
        for start in (0..144_000u64).step_by(256) {
            let len = (144_000 - start).min(256) as usize;
            block[..len].fill([0.0; 2]);
            synth.mix_segment(start, &mut block[..len], &ctrl);
            let (seq, accent) = ctrl.pulse_state();
            if seq != last_seq {
                accents.push(accent);
                last_seq = seq;
            }
        }
        assert_eq!(accents, vec![true, false, false, true, false, false]);
    }
}
