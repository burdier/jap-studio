//! Contrato de exportación headless. El render y los codecs viven en audio/export.
pub mod action;
mod command;
mod preset;

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Wav,
    Aiff,
    Flac,
    Mp3,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Aiff => "aiff",
            Self::Flac => "flac",
            Self::Mp3 => "mp3",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitDepth {
    Int16,
    Int24,
    Int32,
    Float32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Master,
    Individuals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mp3Mode {
    Cbr(u16),
    Vbr(u8),
}

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub name: String,
    pub formats: Vec<Format>,
    pub target: Target,
    /// Índices de pista base cero. Vacío significa todas.
    pub tracks: Vec<usize>,
    pub directory: PathBuf,
    pub sample_rate: u32,
    pub bits: BitDepth,
    pub mp3: Mp3Mode,
    pub mono: bool,
    pub start_beat: f64,
    pub length_beats: Option<f64>,
    pub tail_seconds: f64,
    /// None: Utility solo en master. Some: decisión explícita del usuario.
    pub master_fx: Option<bool>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            name: "export".into(),
            formats: vec![Format::Wav],
            target: Target::Master,
            tracks: Vec::new(),
            directory: "exports".into(),
            sample_rate: 48_000,
            bits: BitDepth::Int24,
            mp3: Mp3Mode::Cbr(320),
            mono: false,
            start_beat: 0.0,
            length_beats: None,
            tail_seconds: 0.0,
            master_fx: None,
        }
    }
}

impl ExportOptions {
    pub fn includes_track(&self, track: usize) -> bool {
        self.tracks.is_empty() || self.tracks.contains(&track)
    }

    pub fn includes_master_fx(&self) -> bool {
        self.master_fx.unwrap_or(self.target == Target::Master)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || matches!(self.name.as_str(), "." | "..")
            || self.name.contains(['/', '\\', '\0'])
        {
            return Err("Nombre inválido; usa -out para elegir la carpeta".into());
        }
        if self.formats.is_empty() {
            return Err("Elige al menos un formato".into());
        }
        if !(8_000..=384_000).contains(&self.sample_rate) {
            return Err("Frecuencia admitida: 8000–384000 Hz".into());
        }
        if !self.start_beat.is_finite()
            || self.start_beat < 0.0
            || !self.tail_seconds.is_finite()
            || self.tail_seconds < 0.0
            || self
                .length_beats
                .is_some_and(|n| !n.is_finite() || n <= 0.0)
        {
            return Err("Rango inválido: inicio/cola ≥ 0 y longitud > 0, valores finitos".into());
        }
        if self.formats.contains(&Format::Flac)
            && !matches!(self.bits, BitDepth::Int16 | BitDepth::Int24)
        {
            return Err("FLAC en este backend admite -bits 16 o 24".into());
        }
        if self.formats.contains(&Format::Mp3) {
            if ![
                8_000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
            ]
            .contains(&self.sample_rate)
            {
                return Err("MP3 requiere 8/11.025/12/16/22.05/24/32/44.1/48 kHz; exporta PCM a mayor frecuencia por separado".into());
            }
            match self.mp3 {
                Mp3Mode::Vbr(q) if q > 9 => return Err("VBR: calidad 0 (mejor) a 9".into()),
                Mp3Mode::Cbr(kbps) => {
                    let rates: &[u16] = if self.sample_rate >= 32_000 {
                        &[
                            32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
                        ]
                    } else if self.sample_rate >= 16_000 {
                        &[8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]
                    } else {
                        &[8, 16, 24, 32, 40, 48, 56, 64]
                    };
                    if !rates.contains(&kbps) {
                        return Err(format!(
                            "Bitrate incompatible con {} Hz; válidos: {rates:?}",
                            self.sample_rate
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
