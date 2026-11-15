//! Puntero inmersivo en GPU (particula-rs + fastrand). Nunca interviene en el tiempo musical.
use gpui::{Bounds, IntoElement, Pixels, Point, Styled, canvas, fill, point, px, rgba, size};
use particula_rs::{NullParticleEmitter, Particle, ParticleSystem, VecParticleSystem};
use std::time::{Duration, Instant};

const MAX_CURSOR_PARTICLES: usize = 640;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CursorFxMode {
    #[default]
    Fire,
    Embers,
    NeonComet,
    Starlight,
    Off,
}

impl CursorFxMode {
    pub const ALL: [Self; 5] = [
        Self::Fire,
        Self::Embers,
        Self::NeonComet,
        Self::Starlight,
        Self::Off,
    ];

    pub fn from_token(token: &str) -> Self {
        match token.trim().to_ascii_lowercase().as_str() {
            "fire" | "fuego" | "fueguito" | "flame" => Self::Fire,
            "embers" | "sparks" | "brasas" | "chispas" => Self::Embers,
            "neon" | "comet" | "cometa" | "cyber" => Self::NeonComet,
            "stars" | "starlight" | "estelas" | "polvo" => Self::Starlight,
            "off" | "none" | "native" | "nativo" => Self::Off,
            _ => Self::Fire,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fire => "Fueguito",
            Self::Embers => "Chispas",
            Self::NeonComet => "Cometa Neón",
            Self::Starlight => "Polvo Estelar",
            Self::Off => "Nativo",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Self::Fire => "🔥",
            Self::Embers => "✨",
            Self::NeonComet => "☄",
            Self::Starlight => "✦",
            Self::Off => "○",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Embers => "sparks",
            Self::NeonComet => "neon",
            Self::Starlight => "stars",
            Self::Off => "off",
        }
    }
}

#[derive(Clone, Copy)]
struct CursorParticle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    gravity: f64,
    turbulence: f64,
    phase: f64,
    age: f64,
    life: f64,
    size: f32,
    tint: f32,
}

impl Particle for CursorParticle {
    type Coordinate = [f64; 2];

    fn get_position(&self) -> Self::Coordinate {
        [self.x, self.y]
    }

    fn update(&mut self, dt: f64) {
        self.age += dt;
        let wobble = (self.age * 14.0 + self.phase).sin() * self.turbulence;
        self.x += (self.vx + wobble) * dt;
        self.y += self.vy * dt + 0.5 * self.gravity * dt * dt;
        self.vy += self.gravity * dt;
        self.vx *= (1.0 - 1.8 * dt).max(0.2);
    }

    fn is_alive(&self) -> bool {
        self.age < self.life
    }
}

pub(super) struct CursorFx {
    mode: CursorFxMode,
    system: VecParticleSystem<CursorParticle, NullParticleEmitter<CursorParticle>>,
    cursor: Option<[f64; 2]>,
    last_move: Option<Instant>,
    last_frame: Option<Instant>,
    carry: f64,
}

impl CursorFx {
    pub fn new(mode: CursorFxMode) -> Self {
        Self {
            mode,
            system: VecParticleSystem::default(),
            cursor: None,
            last_move: None,
            last_frame: None,
            carry: 0.0,
        }
    }

    pub fn mode(&self) -> CursorFxMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: CursorFxMode) {
        self.mode = mode;
        if mode == CursorFxMode::Off {
            self.system = VecParticleSystem::default();
            self.carry = 0.0;
        }
    }

    pub fn on_mouse_move(&mut self, position: Point<Pixels>) {
        let next = [
            f64::from(f32::from(position.x)),
            f64::from(f32::from(position.y)),
        ];
        let prev = self.cursor.replace(next);
        self.last_move = Some(Instant::now());
        if self.mode == CursorFxMode::Off {
            return;
        }
        if let Some(from) = prev {
            let dx = next[0] - from[0];
            let dy = next[1] - from[1];
            let dist = (dx * dx + dy * dy).sqrt();
            let steps = ((dist / 3.5).ceil() as usize).clamp(1, 14);
            for i in 0..steps {
                let t = (i + 1) as f64 / steps as f64;
                let pos = [from[0] + dx * t, from[1] + dy * t];
                self.spawn_one(pos, [-dx * 1.6, -dy * 1.6], 1.0);
            }
        } else {
            self.spawn_one(next, [0.0, 0.0], 1.0);
        }
    }

    pub fn on_mouse_down(&mut self, position: Point<Pixels>) {
        let pos = [
            f64::from(f32::from(position.x)),
            f64::from(f32::from(position.y)),
        ];
        self.cursor = Some(pos);
        self.last_move = Some(Instant::now());
        if self.mode == CursorFxMode::Off {
            return;
        }
        let burst = match self.mode {
            CursorFxMode::Fire => 22,
            CursorFxMode::Embers => 28,
            CursorFxMode::NeonComet => 18,
            CursorFxMode::Starlight => 16,
            CursorFxMode::Off => 0,
        };
        for _ in 0..burst {
            let angle = fastrand::f64() * std::f64::consts::TAU;
            let speed = 35.0 + fastrand::f64() * 135.0;
            self.spawn_one(pos, [angle.cos() * speed, angle.sin() * speed], 1.35);
        }
    }

    pub fn advance(&mut self) -> bool {
        if self.mode == CursorFxMode::Off {
            self.last_frame = None;
            return false;
        }
        let now = Instant::now();
        let dt = self
            .last_frame
            .replace(now)
            .map(|prev| now.duration_since(prev).as_secs_f64())
            .unwrap_or(1.0 / 60.0)
            .min(0.08);
        self.system.update(dt);

        let recently_active = self
            .last_move
            .is_some_and(|t| now.duration_since(t) < Duration::from_secs(4));
        if let Some(pos) = self.cursor
            && recently_active
        {
            let idle_rate = match self.mode {
                CursorFxMode::Fire => 95.0,
                CursorFxMode::Embers => 18.0,
                CursorFxMode::NeonComet => 32.0,
                CursorFxMode::Starlight => 26.0,
                CursorFxMode::Off => 0.0,
            };
            self.carry += dt * idle_rate;
            let count = self.carry.floor() as usize;
            self.carry -= count as f64;
            for _ in 0..count.min(8) {
                self.spawn_one(pos, [0.0, 0.0], 1.0);
            }
        }
        self.system.iter_particles().next().is_some()
    }

    fn spawn_one(&mut self, pos: [f64; 2], inherit_vel: [f64; 2], scale: f32) {
        if self.system.iter_particles().count() >= MAX_CURSOR_PARTICLES {
            return;
        }
        let jitter_x = (fastrand::f64() - 0.5) * 6.0;
        let jitter_y = (fastrand::f64() - 0.5) * 6.0;
        let particle = match self.mode {
            CursorFxMode::Fire => CursorParticle {
                x: pos[0] + jitter_x,
                y: pos[1] + jitter_y,
                vx: inherit_vel[0] * 0.25 + (fastrand::f64() - 0.5) * 42.0,
                vy: inherit_vel[1] * 0.20 - (38.0 + fastrand::f64() * 78.0),
                gravity: -165.0,
                turbulence: 34.0 + fastrand::f64() * 32.0,
                phase: fastrand::f64() * std::f64::consts::TAU,
                age: 0.0,
                life: 0.28 + fastrand::f64() * 0.36,
                size: (4.2 + fastrand::f32() * 4.6) * scale,
                tint: fastrand::f32(),
            },
            CursorFxMode::Embers => CursorParticle {
                x: pos[0] + jitter_x * 0.6,
                y: pos[1] + jitter_y * 0.6,
                vx: inherit_vel[0] * 0.4 + (fastrand::f64() - 0.5) * 130.0,
                vy: inherit_vel[1] * 0.3 - fastrand::f64() * 95.0,
                gravity: 320.0,
                turbulence: 12.0,
                phase: fastrand::f64() * std::f64::consts::TAU,
                age: 0.0,
                life: 0.30 + fastrand::f64() * 0.42,
                size: (2.2 + fastrand::f32() * 2.8) * scale,
                tint: fastrand::f32(),
            },
            CursorFxMode::NeonComet => CursorParticle {
                x: pos[0] + jitter_x * 0.5,
                y: pos[1] + jitter_y * 0.5,
                vx: inherit_vel[0] * 0.35 + (fastrand::f64() - 0.5) * 26.0,
                vy: inherit_vel[1] * 0.35 + (fastrand::f64() - 0.5) * 26.0,
                gravity: 0.0,
                turbulence: 18.0,
                phase: fastrand::f64() * std::f64::consts::TAU,
                age: 0.0,
                life: 0.24 + fastrand::f64() * 0.28,
                size: (3.6 + fastrand::f32() * 3.8) * scale,
                tint: fastrand::f32(),
            },
            CursorFxMode::Starlight => CursorParticle {
                x: pos[0] + jitter_x * 1.5,
                y: pos[1] + jitter_y * 1.5,
                vx: (fastrand::f64() - 0.5) * 24.0,
                vy: (fastrand::f64() - 0.5) * 24.0 - 8.0,
                gravity: -14.0,
                turbulence: 10.0,
                phase: fastrand::f64() * std::f64::consts::TAU,
                age: 0.0,
                life: 0.45 + fastrand::f64() * 0.55,
                size: (2.4 + fastrand::f32() * 3.0) * scale,
                tint: fastrand::f32(),
            },
            CursorFxMode::Off => return,
        };
        self.system.add_particle(particle);
    }

    pub fn render(&self) -> Option<impl IntoElement> {
        if self.mode == CursorFxMode::Off {
            return None;
        }
        let particles: Vec<CursorParticle> = self.system.iter_particles().copied().collect();
        if particles.is_empty() && self.cursor.is_none() {
            return None;
        }
        let mode = self.mode;
        let cursor = self.cursor;
        Some(
            canvas(
                move |_, _, _| (),
                move |bounds, (), window, _| {
                    if let Some([cx_pos, cy_pos]) = cursor {
                        let tip = point(
                            bounds.left() + px(cx_pos as f32),
                            bounds.top() + px(cy_pos as f32),
                        );
                        let (aura_rgb, core_rgb) = match mode {
                            CursorFxMode::Fire | CursorFxMode::Embers => (0xff5e00, 0xfff6c2),
                            CursorFxMode::NeonComet => (0x64d9ca, 0xd98bc7),
                            CursorFxMode::Starlight => (0x5b9cf6, 0xffffff),
                            CursorFxMode::Off => (0x000000, 0x000000),
                        };
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(tip.x - px(9.0), tip.y - px(9.0)),
                                    size(px(18.0), px(18.0)),
                                ),
                                rgba((aura_rgb << 8) | 0x34),
                            )
                            .corner_radii(px(9.0)),
                        );
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(tip.x - px(3.5), tip.y - px(3.5)),
                                    size(px(7.0), px(7.0)),
                                ),
                                rgba((core_rgb << 8) | 0xc8),
                            )
                            .corner_radii(px(3.5)),
                        );
                    }
                    for p in &particles {
                        let progress = (p.age / p.life).clamp(0.0, 1.0) as f32;
                        let fade = (1.0 - progress).powf(1.25);
                        let (rgb_hex, alpha) = particle_color(mode, progress, fade, p.tint, p.age);
                        let radius = (p.size * (1.0 - progress * 0.45)).max(1.2);
                        let center = point(
                            bounds.left() + px(p.x as f32),
                            bounds.top() + px(p.y as f32),
                        );
                        let glow_r = radius * 1.9;
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(center.x - px(glow_r), center.y - px(glow_r)),
                                    size(px(glow_r * 2.0), px(glow_r * 2.0)),
                                ),
                                rgba((rgb_hex << 8) | (alpha / 4).max(8)),
                            )
                            .corner_radii(px(glow_r)),
                        );
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    point(center.x - px(radius), center.y - px(radius)),
                                    size(px(radius * 2.0), px(radius * 2.0)),
                                ),
                                rgba((rgb_hex << 8) | alpha),
                            )
                            .corner_radii(px(radius)),
                        );
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

fn particle_color(mode: CursorFxMode, progress: f32, fade: f32, tint: f32, age: f64) -> (u32, u32) {
    match mode {
        CursorFxMode::Fire => {
            let rgb_hex = if progress < 0.22 {
                0xfff4b8
            } else if progress < 0.52 {
                if tint > 0.45 { 0xff7b14 } else { 0xff9e22 }
            } else if progress < 0.80 {
                0xd92806
            } else {
                0x42241d
            };
            (rgb_hex, (fade * 235.0) as u32)
        }
        CursorFxMode::Embers => {
            let rgb_hex = if progress < 0.30 {
                0xffef99
            } else if progress < 0.70 {
                0xff8811
            } else {
                0xc41e09
            };
            (rgb_hex, (fade * 240.0) as u32)
        }
        CursorFxMode::NeonComet => {
            let rgb_hex = if tint < 0.5 { 0x64d9ca } else { 0xd98bc7 };
            (rgb_hex, (fade * 220.0) as u32)
        }
        CursorFxMode::Starlight => {
            let twinkle = (((age * 18.0).sin() as f32) * 0.35 + 0.65).clamp(0.2, 1.0);
            let rgb_hex = if tint < 0.35 {
                0xffffff
            } else if tint < 0.70 {
                0x7ab8ff
            } else {
                0xd98bc7
            };
            (rgb_hex, (fade * twinkle * 230.0) as u32)
        }
        CursorFxMode::Off => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::{CursorFx, CursorFxMode};
    use gpui::{point, px};

    #[test]
    fn cursor_fx_parses_tokens_and_emits_particles_when_moved() {
        assert_eq!(CursorFxMode::from_token("fueguito"), CursorFxMode::Fire);
        assert_eq!(CursorFxMode::from_token("sparks"), CursorFxMode::Embers);
        assert_eq!(CursorFxMode::from_token("neon"), CursorFxMode::NeonComet);
        assert_eq!(CursorFxMode::from_token("stars"), CursorFxMode::Starlight);
        assert_eq!(CursorFxMode::from_token("off"), CursorFxMode::Off);

        let mut fx = CursorFx::new(CursorFxMode::Fire);
        fx.on_mouse_move(point(px(120.0), px(80.0)));
        fx.on_mouse_move(point(px(150.0), px(95.0)));
        assert!(fx.advance());

        fx.set_mode(CursorFxMode::Off);
        assert!(!fx.advance());
    }
}
