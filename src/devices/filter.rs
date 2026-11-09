//! Contrato headless del filtro estéreo de instrumento, compartido por live y export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterMode {
    LowPass,
    HighPass,
    BandPass,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterSettings {
    pub enabled: bool,
    pub mode: FilterMode,
    pub cutoff: f32,
    pub resonance: f32,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: FilterMode::LowPass,
            cutoff: 12_000.,
            resonance: 0.707,
        }
    }
}
