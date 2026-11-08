//! Efecto visual con particula-rs + fastrand. Nunca participa del tiempo musical.
use super::DesignSystem;
use particula_rs::{NullParticleEmitter, Particle, ParticleSystem, VecParticleSystem};
use std::time::Instant;

const MAX_PARTICLES: usize = 512;

#[derive(Clone, Copy)]
pub(super) struct Spark {
    pub beat: f64,
    pub drift: f64,
    pub y: f64,
    pub age: f64,
    pub life: f64,
    pub size: f32,
    velocity: [f64; 2],
    gravity: f64,
}

impl Particle for Spark {
    type Coordinate = [f64; 2];
    fn get_position(&self) -> Self::Coordinate {
        [self.beat, self.y]
    }
    fn update(&mut self, dt: f64) {
        self.age += dt;
        self.drift += self.velocity[0] * dt;
        self.y += self.velocity[1] * dt + 0.5 * self.gravity * dt * dt;
        self.velocity[1] += self.gravity * dt;
    }
    fn is_alive(&self) -> bool {
        self.age < self.life
    }
}

#[derive(Default)]
pub(super) struct ParticleTrail {
    system: VecParticleSystem<Spark, NullParticleEmitter<Spark>>,
    previous_beat: Option<f64>,
    previous_frame: Option<Instant>,
    emission: f64,
}

impl ParticleTrail {
    pub fn snapshot(&self) -> Vec<Spark> {
        self.system.iter_particles().copied().collect()
    }

    pub fn advance(
        &mut self,
        beat: Option<f64>,
        height: f64,
        style: &DesignSystem,
        visible: bool,
    ) -> bool {
        let now = Instant::now();
        let dt = self
            .previous_frame
            .replace(now)
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        let previous = std::mem::replace(&mut self.previous_beat, beat);
        if !visible || dt > 0.25 {
            self.system = VecParticleSystem::default();
            self.emission = 0.0;
            return false;
        }
        self.system.update(dt);
        if let (Some(from), Some(to)) = (previous, beat)
            && to > from
            && to - from < 1.0
            && height > 0.0
        {
            self.emission += dt * f64::from(style.playhead_particle_rate);
            let count = self.emission.floor() as usize;
            self.emission -= count as f64;
            let available = MAX_PARTICLES.saturating_sub(self.system.iter_particles().count());
            for _ in 0..count.min(available) {
                self.system.add_particle(Spark {
                    beat: from + (to - from) * fastrand::f64(),
                    drift: 0.0,
                    y: height * fastrand::f64(),
                    age: 0.0,
                    life: f64::from(style.playhead_particle_life) * (0.65 + 0.35 * fastrand::f64()),
                    size: style.playhead_particle_size * (0.7 + 0.6 * fastrand::f32()),
                    velocity: [
                        -f64::from(style.playhead_particle_speed) * (0.4 + fastrand::f64()),
                        (fastrand::f64() - 0.65) * 40.0,
                    ],
                    gravity: f64::from(style.playhead_particle_gravity),
                });
            }
        } else {
            self.emission = 0.0;
        }
        self.system.iter_particles().next().is_some()
    }
}
