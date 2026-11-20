//! Percusión sintetizada de audición. Sustituir por un sampler en el siguiente incremento.
//! Sin asignaciones ni E/S: el callback solo reutiliza este banco fijo de voces.
use elevate::musical::model::InstrumentId;

const MAX_VOICES: usize = 64;
const TAU: f64 = std::f64::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Kick,
    Snare,
    Hat,
    Clap,
}

impl Kind {
    fn from_instrument(instrument: InstrumentId) -> Self {
        match instrument.0 {
            1 => Self::Kick,
            2 => Self::Snare,
            4 => Self::Clap,
            _ => Self::Hat,
        }
    }

    fn life(self) -> f64 {
        match self {
            Self::Kick => 0.55,
            Self::Snare => 0.35,
            Self::Hat => 0.16,
            Self::Clap => 0.30,
        }
    }

    fn level(self) -> f64 {
        match self {
            Self::Kick => 0.95,
            Self::Snare => 0.62,
            Self::Hat => 0.42,
            Self::Clap => 0.52,
        }
    }

    fn pan(self) -> f64 {
        match self {
            Self::Hat => 0.12,
            Self::Clap => -0.08,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    kind: Option<Kind>,
    age: f64,
    phase: f64,
    noise: u32,
    filter: f64,
    amplitude: f64,
    pitch: f64,
}

impl Voice {
    const SILENT: Self = Self {
        kind: None,
        age: 0.0,
        phase: 0.0,
        noise: 1,
        filter: 0.0,
        amplitude: 0.0,
        pitch: 1.0,
    };

    fn trigger(&mut self, instrument: InstrumentId, velocity: u8, seed: u64, pitch: f64) {
        let kind = Kind::from_instrument(instrument);
        let normalized = f64::from(velocity) / 127.0;
        self.kind = Some(kind);
        self.age = 0.0;
        self.phase = 0.0;
        self.noise = (seed as u32) | 1;
        self.filter = 0.0;
        self.amplitude = kind.level() * normalized.sqrt();
        self.pitch = pitch;
    }

    fn next(&mut self, dt: f64) -> [f32; 2] {
        let Some(kind) = self.kind else {
            return [0.0; 2];
        };
        let age = self.age;
        let noise = self.noise();
        let sample = match kind {
            Kind::Kick => {
                let frequency = 45.0 + 95.0 * (-age * 32.0).exp();
                self.phase = (self.phase + TAU * frequency * dt * self.pitch) % TAU;
                let body = self.phase.sin() * (-age * 9.0).exp();
                let click = if age < 0.004 { noise * 0.35 } else { 0.0 };
                (body + click) * self.amplitude
            }
            Kind::Snare => {
                let tone = (TAU * 185.0 * age * self.pitch).sin();
                (noise * 0.75 + tone * 0.25) * (-age * 22.0).exp() * self.amplitude
            }
            Kind::Hat => {
                let high = noise - self.filter * 0.92;
                self.filter = noise;
                high * (-age * 65.0).exp() * self.amplitude
            }
            Kind::Clap => {
                let burst = |start: f64| {
                    if age >= start {
                        (-(age - start) * 90.0).exp()
                    } else {
                        0.0
                    }
                };
                let envelope = (burst(0.0) + burst(0.011) * 0.8 + burst(0.022) * 0.7) * 0.5
                    + (-age * 16.0).exp() * 0.35;
                let high = noise - self.filter * 0.85;
                self.filter = noise;
                high * envelope * self.amplitude
            }
        };
        self.age += dt;
        if self.age >= kind.life() {
            self.kind = None;
        }
        let pan = kind.pan();
        [(sample * (1.0 - pan)) as f32, (sample * (1.0 + pan)) as f32]
    }

    fn noise(&mut self) -> f64 {
        let mut value = self.noise;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.noise = value;
        f64::from(value) / f64::from(u32::MAX) * 2.0 - 1.0
    }
}

/// Banco fijo de voces. Roba la voz más antigua cuando las 64 están ocupadas.
pub(super) struct DrumKit {
    voices: [Voice; MAX_VOICES],
    seed: u32,
}

impl DrumKit {
    pub fn new() -> Self {
        Self {
            voices: [Voice::SILENT; MAX_VOICES],
            seed: 0x9e37_79b9,
        }
    }

    #[cfg(test)]
    pub fn trigger(&mut self, instrument: InstrumentId, velocity: u8, note: u64) {
        self.trigger_pitched(instrument, velocity, note, 1.0);
    }

    pub fn trigger_pitched(
        &mut self,
        instrument: InstrumentId,
        velocity: u8,
        note: u64,
        pitch: f64,
    ) {
        if velocity == 0 {
            return;
        }
        let index = self
            .voices
            .iter()
            .position(|voice| voice.kind.is_none())
            .unwrap_or_else(|| {
                let mut oldest = 0;
                let mut age = 0.0;
                for (index, voice) in self.voices.iter().enumerate() {
                    if let Some(kind) = voice.kind {
                        let normalized = voice.age / kind.life();
                        if normalized > age {
                            age = normalized;
                            oldest = index;
                        }
                    }
                }
                oldest
            });
        self.seed = self.seed.wrapping_add(0x9e37_79b9);
        self.voices[index].trigger(instrument, velocity, note ^ u64::from(self.seed), pitch);
    }

    /// Escribe el mix en `frames`. `dt` es 1/sample_rate; no usa tiempo de pared.
    pub fn render(&mut self, frames: &mut [[f32; 2]], dt: f64) {
        frames.fill([0.0; 2]);
        for voice in self.voices.iter_mut().filter(|voice| voice.kind.is_some()) {
            for frame in &mut *frames {
                let sample = voice.next(dt);
                frame[0] += sample[0];
                frame[1] += sample[1];
                if voice.kind.is_none() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DrumKit;
    use elevate::musical::model::InstrumentId;

    #[test]
    fn synthesized_kit_is_finite_audible_and_releases_its_voices() {
        let mut kit = DrumKit::new();
        for instrument in 1..=4 {
            kit.trigger(InstrumentId(instrument), 100, u64::from(instrument));
        }
        let mut block = [[0.0_f32; 2]; 512];
        kit.render(&mut block, 1.0 / 48_000.0);
        assert!(block.iter().flatten().all(|sample| sample.is_finite()));
        assert!(block.iter().flatten().any(|sample| sample.abs() > 1e-6));

        let mut tail = [[0.0_f32; 2]; 48_000];
        kit.render(&mut tail, 1.0 / 48_000.0);
        assert!(
            tail[47_000..]
                .iter()
                .flatten()
                .all(|sample| sample.abs() < 1e-6)
        );
    }
}
