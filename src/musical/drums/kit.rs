//! Piezas y capas del kit. Las rutas aún no representan muestras decodificadas.
use crate::{
    devices::sampler::{ATTACK, ROOT, SamplerParams},
    musical::{model::InstrumentId, track::TrackMix},
};
use std::{collections::HashSet, path::Path};

// El motor reserva un conjunto acotado de voces y controles por kit.
pub const MAX_DRUM_PIECES: usize = 64;
// Los proyectos anteriores usaban 1–4 para las voces sintetizadas.
pub const LAST_LEGACY_DRUM_ID: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrumKitId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct DrumKit {
    pub id: DrumKitId,
    pub pieces: Vec<DrumPiece>,
    pub bus: DrumMix,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrumPiece {
    pub id: InstrumentId,
    pub name: String,
    pub layers: Vec<DrumLayer>,
    pub sampler: SamplerParams,
    pub choke_group: Option<u16>,
    pub mix: DrumMix,
    /// Desfase de tiempo relativo a la rejilla en ticks (`-120..=120`, ±1 semicorchea).
    pub timing_offset: i16,
    /// Modo de desfase temporal: fijo, alterno en cada nota (groove/swing) o aleatorio.
    pub timing_mode: OffsetMode,
}

impl DrumPiece {
    pub const MAX_TIMING_OFFSET: i16 = 120;

    pub fn base_note(&self) -> u8 {
        self.sampler.get(ROOT) as u8
    }

    /// Calcula el desplazamiento temporal efectivo en ticks de un golpe según el modo
    /// de timing de la pieza. Compartido entre el motor de audio y el feedback visual.
    pub fn timing_delta_for(
        piece_id: InstrumentId,
        timing_offset: i16,
        timing_mode: OffsetMode,
        note_start_tick: i64,
        note_index: usize,
    ) -> i64 {
        if timing_offset == 0 {
            return 0;
        }
        let offset = i64::from(timing_offset);
        match timing_mode {
            OffsetMode::Fixed => offset,
            OffsetMode::EachNote => {
                if note_index % 2 == 1 {
                    offset
                } else {
                    0
                }
            }
            OffsetMode::Random => {
                let mag = offset.abs();
                if mag == 0 {
                    0
                } else {
                    let seed = (u64::from(piece_id.0) << 32)
                        ^ (note_start_tick as u64).wrapping_mul(2_654_435_761)
                        ^ (note_index as u64).wrapping_mul(1_013_904_223);
                    let mixed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                    let span = (mag * 2 + 1) as u64;
                    ((mixed >> 16) % span) as i64 - mag
                }
            }
        }
    }

    pub fn timing_delta(&self, note_start_tick: i64, note_index: usize) -> i64 {
        Self::timing_delta_for(
            self.id,
            self.timing_offset,
            self.timing_mode,
            note_start_tick,
            note_index,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OffsetMode {
    #[default]
    Fixed,
    EachNote,
    Random,
}

impl OffsetMode {
    pub fn next(self) -> Self {
        match self {
            Self::Fixed => Self::EachNote,
            Self::EachNote => Self::Random,
            Self::Random => Self::Fixed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fixed => "Fijo",
            Self::EachNote => "Cada nota",
            Self::Random => "Random",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Fixed => "Fij",
            Self::EachNote => "Nota",
            Self::Random => "Rnd",
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::Fixed => 0,
            Self::EachNote => 1,
            Self::Random => 2,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::EachNote,
            2 => Self::Random,
            _ => Self::Fixed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumLayer {
    pub source: String,
    /// Ambos extremos se incluyen; la velocidad MIDI cero se reserva a NoteOff.
    pub velocity_min: u8,
    pub velocity_max: u8,
    /// Parte del tramo reproducible que se salta al atacar, en milésimas.
    pub start_offset: u16,
    /// Comportamiento del offset por golpe: fijo, progresivo en cada nota o aleatorio.
    pub offset_mode: OffsetMode,
}

impl DrumLayer {
    pub const MAX_START_OFFSET: u16 = 999;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrumMix {
    pub fader: TrackMix,
    pub muted: bool,
    pub solo: bool,
}

impl Default for DrumMix {
    fn default() -> Self {
        Self {
            fader: TrackMix { level: 75, pan: 50 },
            muted: false,
            solo: false,
        }
    }
}

impl DrumMix {
    fn validate(self) -> bool {
        (0..=100).contains(&self.fader.level) && (0..=100).contains(&self.fader.pan)
    }
}

impl Default for DrumKit {
    fn default() -> Self {
        Self {
            id: DrumKitId(1),
            pieces: Vec::new(),
            bus: DrumMix::default(),
        }
    }
}

impl DrumKit {
    pub fn add_sample(&mut self, source: String) -> Result<InstrumentId, String> {
        if self.pieces.len() >= MAX_DRUM_PIECES {
            return Err(format!("El kit admite hasta {MAX_DRUM_PIECES} piezas"));
        }
        if source.trim().is_empty() || source.contains('\0') {
            return Err("Ruta de muestra inválida".into());
        }
        let next = self
            .pieces
            .iter()
            .map(|piece| piece.id.0)
            .fold(LAST_LEGACY_DRUM_ID, u32::max)
            .checked_add(1)
            .filter(|&id| id < u32::MAX)
            .ok_or("IDs de pieza agotados")?;
        let name = Path::new(&source)
            .file_stem()
            .and_then(|name| name.to_str())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("Muestra")
            .to_owned();
        let id = InstrumentId(next);
        let mut sampler = SamplerParams::default();
        sampler.apply_drum_preset();
        sampler.play_full_sample();
        // Transparente por defecto (igual que la preescucha del Explorer): sin recorte de agudos ni ataque suavizado.
        sampler.filter = false;
        sampler.set(ATTACK, 0.0);
        sampler.keytrack = true;
        sampler.set(ROOT, 36.0);
        self.pieces.push(DrumPiece {
            id,
            name,
            layers: vec![DrumLayer {
                source,
                velocity_min: 1,
                velocity_max: 127,
                start_offset: 0,
                offset_mode: OffsetMode::Fixed,
            }],
            sampler,
            choke_group: None,
            mix: DrumMix::default(),
            timing_offset: 0,
            timing_mode: OffsetMode::Fixed,
        });
        Ok(id)
    }

    /// Duplica una pieza del kit justo debajo de la original.
    /// Con `keep_layers = false` crea una pista sin muestra (lista para soltar otro audio).
    pub fn clone_piece(&mut self, lane: usize, keep_layers: bool) -> Result<InstrumentId, String> {
        if self.pieces.len() >= MAX_DRUM_PIECES {
            return Err(format!("El kit admite hasta {MAX_DRUM_PIECES} piezas"));
        }
        let source = self
            .pieces
            .get(lane)
            .ok_or("La pieza ya no existe")?
            .clone();
        let next = self
            .pieces
            .iter()
            .map(|piece| piece.id.0)
            .fold(LAST_LEGACY_DRUM_ID, u32::max)
            .checked_add(1)
            .filter(|&id| id < u32::MAX)
            .ok_or("IDs de pieza agotados")?;
        let id = InstrumentId(next);
        let mut cloned = source;
        cloned.id = id;
        cloned.name = if keep_layers {
            format!("{} (copia)", cloned.name)
        } else {
            format!("{} (MIDI)", cloned.name)
        };
        if !keep_layers {
            cloned.layers.clear();
        }
        self.pieces.insert(lane + 1, cloned);
        Ok(id)
    }

    pub fn remove_piece(&mut self, lane: usize) -> Result<InstrumentId, String> {
        if lane >= self.pieces.len() {
            return Err("La pieza ya no existe".into());
        }
        Ok(self.pieces.remove(lane).id)
    }

    pub fn validate(&self) -> Result<HashSet<InstrumentId>, String> {
        if self.id.0 == 0 || !self.bus.validate() {
            return Err("Kit o bus de batería inválido".into());
        }
        let mut ids = HashSet::new();
        for piece in &self.pieces {
            if piece.id.0 == 0
                || piece.id.0 == u32::MAX
                || !ids.insert(piece.id)
                || piece.name.trim().is_empty()
                || !piece.mix.validate()
                || piece.choke_group == Some(0)
            {
                return Err("Pieza o mezcla de batería inválida".into());
            }
            for layer in &piece.layers {
                if layer.source.trim().is_empty()
                    || layer.source.contains('\0')
                    || layer.velocity_min == 0
                    || layer.velocity_min > layer.velocity_max
                    || layer.velocity_max > 127
                    || layer.start_offset > DrumLayer::MAX_START_OFFSET
                {
                    return Err("Capa de batería inválida".into());
                }
            }
        }
        Ok(ids)
    }
}
