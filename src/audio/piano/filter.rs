//! SVF TPT estéreo: LP/HP/BP, sin reservas ni E/S. Coeficientes y bypass suavizados.
use elevate::devices::filter::{FilterMode, FilterSettings};

pub(super) struct StereoFilter {
    rate: f32,
    slew: f32,
    current: [f32; 6], // g, k, dry, LP, HP, BP
    target: [f32; 6],
    state: [[f32; 2]; 2],
}

impl StereoFilter {
    pub fn new(rate: u32, settings: FilterSettings) -> Self {
        let mut filter = Self {
            rate: rate as f32,
            slew: 1. - (-1. / (rate as f32 * 0.005)).exp(),
            current: [0.; 6],
            target: [0.; 6],
            state: [[0.; 2]; 2],
        };
        filter.set(settings);
        filter.current = filter.target;
        filter
    }

    pub fn set(&mut self, settings: FilterSettings) {
        let cutoff = if settings.cutoff.is_finite() {
            settings.cutoff
        } else {
            12_000.
        };
        let q = if settings.resonance.is_finite() {
            settings.resonance
        } else {
            0.707
        };
        let g = (std::f32::consts::PI * cutoff.clamp(20., self.rate * 0.45) / self.rate).tan();
        self.target = [g, 1. / q.clamp(0.5, 10.), 0., 0., 0., 0.];
        let output = if !settings.enabled {
            2
        } else {
            match settings.mode {
                FilterMode::LowPass => 3,
                FilterMode::HighPass => 4,
                FilterMode::BandPass => 5,
            }
        };
        self.target[output] = 1.;
    }

    pub fn process(&mut self, frames: &mut [[f32; 2]]) {
        for frame in frames {
            for (current, target) in self.current.iter_mut().zip(self.target) {
                *current += (target - *current) * self.slew;
            }
            let [g, k, dry, lp, hp, bp] = self.current;
            let a = 1. / (1. + g * (g + k));
            for (input, state) in frame.iter_mut().zip(&mut self.state) {
                let v3 = *input - state[1];
                let band = a * (state[0] + g * v3);
                let low = state[1] + g * band;
                let high = *input - k * band - low;
                state[0] = (2. * band - state[0]).clamp(-1e12, 1e12);
                state[1] = (2. * low - state[1]).clamp(-1e12, 1e12);
                for value in state {
                    if value.abs() < 1e-20 {
                        *value = 0.;
                    }
                }
                *input = dry * *input + lp * low + hp * high + bp * band;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn energy(mode: FilterMode, hz: f32) -> f32 {
        let mut filter = StereoFilter::new(
            48_000,
            FilterSettings {
                enabled: true,
                mode,
                cutoff: 1_000.,
                ..Default::default()
            },
        );
        let mut frames: Vec<_> = (0..48_000)
            .map(|i| {
                let v = (std::f32::consts::TAU * hz * i as f32 / 48_000.).sin();
                [v, 0.]
            })
            .collect();
        filter.process(&mut frames);
        assert!(frames.iter().all(|f| f[1] == 0.));
        frames[24_000..].iter().map(|f| f[0] * f[0]).sum::<f32>() / 24_000.
    }

    #[test]
    fn modes_attenuate_the_expected_frequencies_without_channel_bleed() {
        assert!(energy(FilterMode::LowPass, 100.) > 100. * energy(FilterMode::LowPass, 10_000.));
        assert!(energy(FilterMode::HighPass, 10_000.) > 100. * energy(FilterMode::HighPass, 100.));
        assert!(energy(FilterMode::BandPass, 1_000.) > 10. * energy(FilterMode::BandPass, 100.));
    }

    #[test]
    fn bypass_is_exact_and_automation_is_partition_invariant() {
        let input: Vec<_> = (0..4096).map(|i| [(i as f32 * 0.1).sin(), 0.25]).collect();
        let mut a = StereoFilter::new(44_100, FilterSettings::default());
        let mut bypass = input.clone();
        a.process(&mut bypass);
        assert_eq!(bypass, input);
        let mut b = StereoFilter::new(44_100, FilterSettings::default());
        b.process(&mut input.clone());
        let settings = FilterSettings {
            enabled: true,
            cutoff: 19_000.,
            resonance: 10.,
            ..Default::default()
        };
        a.set(settings);
        b.set(settings);
        let mut whole = input.clone();
        let mut split = input;
        a.process(&mut whole);
        for block in split.chunks_mut(37) {
            b.process(block);
        }
        assert_eq!(whole, split);
        assert!(
            whole
                .iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() < 100.)
        );
    }
}
