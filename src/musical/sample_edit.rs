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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleEdit {
    pub fades: [ClipFade; 2],
    pub crossfade: Option<Crossfade>,
    /// Fragmentos consecutivos de un cruce comparten su curva hasta separarlos.
    pub crossfade_root: Option<super::clip::ClipId>,
    pub bell: Bell,
    pub fine_cents: i8,
    pub ranges: Vec<RangeEdit>,
}
impl SampleEdit {
    pub const LIMIT: usize = 256;
    pub fn same_pcm(&self, other: &Self) -> bool {
        self.bell == other.bell
            && self.fine_cents == other.fine_cents
            && self.ranges == other.ranges
    }

    pub fn validate(&self) -> Result<(), String> {
        let b = self.bell;
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
