//! Ediciones no destructivas por instancia. Los rangos usan frames de la fuente original.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bell {
    pub hz: f64,
    pub db: f64,
    pub q: f64,
    pub enabled: bool,
}
impl Default for Bell {
    fn default() -> Self {
        Self {
            hz: 1000.,
            db: 0.,
            q: 1.,
            enabled: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Process {
    FadeOut,
    Reverse,
    Normalize,
    FadeIn,
    Silence,
    ChordStart,
    ChordNext,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeEdit {
    pub first: usize,
    pub end: usize,
    pub op: Process,
}
/// La referencia une dos instancias del Arrangement; el PCM original sigue compartido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossfade {
    pub from: super::clip::ClipId,
    pub center: u8,
}

/// Intervalo en microsegundos de la fuente, compartido por las dos mitades al dividir.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClipFade {
    pub start_us: i64,
    pub end_us: i64,
    pub bend: i8,
}
impl ClipFade {
    pub fn active(self) -> bool {
        self.end_us > self.start_us
    }
    pub fn tension(self) -> f32 {
        2f32.powf(-f32::from(self.bend) / 25.)
    }
    pub fn curve(t: f32, tension: f32) -> f32 {
        let t = t.clamp(0., 1.);
        t / (t + tension * (1. - t))
    }
}
/// Algoritmo de Warp (estiramiento temporal) estilo Ableton Live por clip de audio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WarpMode {
    #[default]
    Off,
    /// Cambia velocidad y tono juntos (como vinilo/cinta; cero artefactos).
    RePitch,
    /// Preserva los golpes (transientes) y estira/acorta entre ellos (ideal para baterías).
    Beats,
    /// Phase Vocoder con bloqueo de fase estéreo (mantiene tono en acordes/voces).
    Complex,
}

impl WarpMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::RePitch => "Re-Pitch",
            Self::Beats => "Beats",
            Self::Complex => "Complex",
        }
    }

    pub fn code(self) -> i64 {
        match self {
            Self::Off => 0,
            Self::RePitch => 1,
            Self::Beats => 2,
            Self::Complex => 3,
        }
    }

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::Off),
            1 => Some(Self::RePitch),
            2 => Some(Self::Beats),
            3 => Some(Self::Complex),
            _ => None,
        }
    }
}

/// Configuración de Warp no destructivo por instancia de clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpSettings {
    pub mode: WarpMode,
    /// BPM original del archivo de audio (`20..=300`).
    pub original_bpm: f32,
    /// Multiplicador de velocidad (`0.5` = :2 mitad, `1.0` = normal, `2.0` = ×2 doble).
    pub rate: f32,
}

impl Default for WarpSettings {
    fn default() -> Self {
        Self {
            mode: WarpMode::Off,
            original_bpm: 120.0,
            rate: 1.0,
        }
    }
}

impl WarpSettings {
    /// Factor de duración resultante (`> 1.0` alarga el clip, `< 1.0` lo acorta).
    pub fn stretch_factor(self, project_bpm: f64) -> f64 {
        if self.mode == WarpMode::Off {
            return 1.0;
        }
        let orig = f64::from(self.original_bpm).clamp(20.0, 300.0);
        let proj = project_bpm.clamp(20.0, 360.0);
        let rate = f64::from(self.rate).clamp(0.25, 4.0);
        (orig / (proj * rate)).clamp(0.25, 4.0)
    }

    pub fn is_active(self, project_bpm: f64) -> bool {
        self.mode != WarpMode::Off && (self.stretch_factor(project_bpm) - 1.0).abs() > 1e-4
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleEdit {
    pub fades: [ClipFade; 2],
    pub crossfade: Option<Crossfade>,
    /// Fragmentos consecutivos de un cruce comparten su curva hasta separarlos.
    pub crossfade_root: Option<super::clip::ClipId>,
    pub bell: Bell,
    pub fine_cents: i8,
    pub warp: WarpSettings,
    pub ranges: Vec<RangeEdit>,
}
impl SampleEdit {
    pub const LIMIT: usize = 256;
    pub fn same_pcm(&self, other: &Self) -> bool {
        self.bell == other.bell
            && self.fine_cents == other.fine_cents
            && self.warp == other.warp
            && self.ranges == other.ranges
    }

    pub fn validate(&self) -> Result<(), String> {
        let b = self.bell;
        let w = self.warp;
        if self
            .fades
            .iter()
            .any(|f| f.start_us < 0 || f.end_us < f.start_us || !(-100..=100).contains(&f.bend))
            || self
                .crossfade
                .is_some_and(|x| !(1..=99).contains(&x.center))
            || !(-100..=100).contains(&self.fine_cents)
            || !(20.0..=20000.).contains(&b.hz)
            || !(-18.0..=18.).contains(&b.db)
            || !(0.25..=8.).contains(&b.q)
            || !w.original_bpm.is_finite()
            || !(20.0..=300.0).contains(&w.original_bpm)
            || !w.rate.is_finite()
            || !(0.25..=4.0).contains(&w.rate)
            || self.ranges.len() > Self::LIMIT
            || self
                .ranges
                .iter()
                .any(|r| r.first >= r.end || r.end > i64::MAX as usize)
        {
            return Err("Edición de muestra inválida".into());
        }
        Ok(())
    }
}
