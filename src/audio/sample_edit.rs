//! Preparación del PCM editado fuera de RT. La pista y la exportación reciben el mismo recurso.
use super::{AudioClip, AudioFile};
use elevate::musical::sample_edit::{Bell, Process, SampleEdit};
use std::{f64::consts::TAU, sync::Arc};
#[derive(Clone, Default)]
pub(crate) struct Processing {
    pub edit: SampleEdit,
    original: Option<Arc<AudioFile>>,
}
impl AudioClip {
    pub fn original_file(&self) -> &Arc<AudioFile> {
        self.processing.original.as_ref().unwrap_or(&self.file)
    }
}
// Biquad de campana RBJ. La curva dibujada usa los mismos coeficientes que el audio.
pub(crate) struct Coefficients {
    b: [f64; 3],
    a: [f64; 2],
}
impl Coefficients {
    pub fn bell(p: Bell, rate: u32) -> Self {
        let w = TAU * p.hz.min(0.49 * f64::from(rate)) / f64::from(rate);
        let amp = 10f64.powf(if p.enabled { p.db / 40. } else { 0. });
        let alpha = w.sin() / (2. * p.q);
        let a0 = 1. + alpha / amp;
        Self {
            b: [
                (1. + alpha * amp) / a0,
                -2. * w.cos() / a0,
                (1. - alpha * amp) / a0,
            ],
            a: [-2. * w.cos() / a0, (1. - alpha / amp) / a0],
        }
    }
    pub fn db(&self, hz: f64, rate: u32) -> f64 {
        let w = TAU * hz.min(0.49 * f64::from(rate)) / f64::from(rate);
        let power = |[a, b, c]: [f64; 3]| {
            (a + b * w.cos() + c * (2. * w).cos()).powi(2)
                + (b * w.sin() + c * (2. * w).sin()).powi(2)
        };
        10. * (power(self.b) / power([1., self.a[0], self.a[1]])).log10()
    }
    fn apply(&self, frames: &mut [[f32; 2]]) {
        let mut z = [[0f64; 2]; 2];
        for pair in frames {
            for (sample, z) in pair.iter_mut().zip(&mut z) {
                let x = f64::from(*sample);
                let y = self.b[0] * x + z[0];
                z[0] = self.b[1] * x - self.a[0] * y + z[1];
                z[1] = self.b[2] * x - self.a[1] * y;
                *sample = y as f32;
            }
        }
    }
}
impl Processing {
    pub fn prepare(
        original: Arc<AudioFile>,
        edit: SampleEdit,
        semitones: i8,
    ) -> Result<(Arc<AudioFile>, Self), String> {
        edit.validate()?;
        let cents = i16::from(semitones) * 100 + i16::from(edit.fine_cents);
        if cents == 0 && edit.ranges.is_empty() && (!edit.bell.enabled || edit.bell.db == 0.) {
            return Ok((
                original,
                Self {
                    edit,
                    original: None,
                },
            ));
        }
        let mut frames = original.frames.clone();
        for range in &edit.ranges {
            let part = frames
                .get_mut(range.first..range.end)
                .ok_or("La selección excede la duración de la fuente")?;
            match range.op {
                Process::ChordStart | Process::ChordNext => {
                    part.reverse();
                    let len = part.len();
                    let ramp = (original.sample_rate as usize / 500).min(len / 2).max(1);
                    for (i, frame) in part.iter_mut().enumerate() {
                        let gain = i.min(len - 1 - i).min(ramp) as f32 / ramp as f32;
                        for sample in frame {
                            *sample *= gain;
                        }
                    }
                }
                Process::Reverse => part.reverse(),
                Process::Normalize => {
                    let peak = part.iter().flatten().fold(0f32, |m, x| m.max(x.abs()));
                    if peak > 0. {
                        for x in part.iter_mut().flatten() {
                            *x = (*x / peak) * 0.99;
                        }
                    }
                }
                Process::FadeOut => {
                    let n = part.len().saturating_sub(1);
                    for (i, pair) in part.iter_mut().enumerate() {
                        let gain = (n - i) as f32 / n.max(1) as f32;
                        for x in pair {
                            *x *= gain
                        }
                    }
                }
                Process::FadeIn => {
                    let n = part.len().saturating_sub(1);
                    for (i, pair) in part.iter_mut().enumerate() {
                        let gain = i as f32 / n.max(1) as f32;
                        for x in pair {
                            *x *= gain
                        }
                    }
                }
                Process::Silence => {
                    for x in part.iter_mut().flatten() {
                        *x = 0.0;
                    }
                }
            }
        }
        if cents != 0 {
            frames = super::sample_pitch::transpose(&frames, original.sample_rate, cents)?;
        }
        if edit.bell.enabled && edit.bell.db != 0. {
            Coefficients::bell(edit.bell, original.sample_rate).apply(&mut frames)
        }
        let file = AudioFile::from_frames(
            original.name.clone(),
            original.source_path.clone(),
            original.sample_rate,
            frames,
        );
        Ok((
            Arc::new(file),
            Self {
                edit,
                original: Some(original),
            },
        ))
    }
}
