//! Lectura de pistas y mezcla de sesión con identidad estable, sin vistas ni DSP.
use super::{Track, TrackId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackMix {
    /// Fader de 0 a 100; 75 representa unidad en el adaptador actual.
    pub level: i32,
    /// Panorama de 0 a 100; 50 es el centro.
    pub pan: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackSnapshotEntry {
    pub track: Track,
    pub mix: TrackMix,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackSnapshot {
    /// Revisión de la sesión capturada por el controlador.
    pub revision: u64,
    pub entries: Vec<TrackSnapshotEntry>,
}

impl TrackSnapshot {
    pub fn capture(
        tracks: &[Track],
        levels: &[i32],
        pans: &[i32],
        revision: u64,
    ) -> Result<Self, String> {
        if tracks.len() != levels.len() || tracks.len() != pans.len() {
            return Err("Pistas y controles de mezcla desalineados".into());
        }
        let mut ids = std::collections::HashSet::new();
        let entries = tracks
            .iter()
            .zip(levels.iter().zip(pans))
            .map(|(track, (&level, &pan))| {
                if !ids.insert(track.id) {
                    return Err("ID de pista duplicado".into());
                }
                if !(0..=100).contains(&level) || !(0..=100).contains(&pan) {
                    return Err("Nivel o panorama fuera de rango".into());
                }
                Ok(TrackSnapshotEntry {
                    track: track.clone(),
                    mix: TrackMix { level, pan },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { revision, entries })
    }

    pub fn get(&self, id: TrackId) -> Option<&TrackSnapshotEntry> {
        self.entries.iter().find(|entry| entry.track.id == id)
    }
}
