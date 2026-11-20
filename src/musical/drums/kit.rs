//! Piezas y capas del kit. Las rutas aún no representan muestras decodificadas.
use crate::{
    devices::sampler::{ROOT, SamplerParams},
    musical::{model::InstrumentId, track::TrackMix},
};
use std::{collections::HashSet, path::Path};

// El motor reserva un conjunto acotado de voces y controles por kit.
pub const MAX_DRUM_PIECES: usize = 64;

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
}

impl DrumPiece {
    fn new(id: InstrumentId, name: String, note: u8, layers: Vec<DrumLayer>) -> Self {
        let mut sampler = SamplerParams::default();
        sampler.apply_drum_preset();
        sampler.keytrack = true;
        sampler.choke = false;
        sampler.set(ROOT, f32::from(note));
        Self {
            id,
            name,
            layers,
            sampler,
            choke_group: None,
            mix: DrumMix::default(),
        }
    }

    pub fn base_note(&self) -> u8 {
        self.sampler.get(ROOT) as u8
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrumLayer {
    pub source: String,
    /// Ambos extremos se incluyen; la velocidad MIDI cero se reserva a NoteOff.
    pub velocity_min: u8,
    pub velocity_max: u8,
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
        let pieces = [
            (1, "Kick", 36_u8),
            (2, "Snare", 38),
            (3, "Hi-Hat", 42),
            (4, "Clap", 39),
        ]
        .into_iter()
        .map(|(id, name, note)| DrumPiece::new(InstrumentId(id), name.into(), note, Vec::new()))
        .collect();
        Self {
            id: DrumKitId(1),
            pieces,
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
            .max()
            .unwrap_or(0)
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
        self.pieces.push(DrumPiece::new(
            id,
            name,
            36,
            vec![DrumLayer {
                source,
                velocity_min: 1,
                velocity_max: 127,
            }],
        ));
        Ok(id)
    }

    pub(super) fn validate(&self) -> Result<HashSet<InstrumentId>, String> {
        if self.id.0 == 0 || self.pieces.is_empty() || !self.bus.validate() {
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
                {
                    return Err("Capa de batería inválida".into());
                }
            }
        }
        Ok(ids)
    }
}
