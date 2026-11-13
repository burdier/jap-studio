//! Pistas del proyecto. Sin voces, dispositivos ni coordenadas de pantalla.
//! La asignación de instrumento es estado musical; la UI solo la presenta.
use super::{fx::FxChain, model::InstrumentId};
use std::{
    borrow::Cow,
    sync::atomic::{AtomicU32, Ordering},
};
mod batch;
mod management;
mod mix;
pub use mix::{EffectiveMix, UNITY_LEVEL, effective_mix};
mod snapshot;
mod validation;
pub use batch::{
    TrackBatch, TrackBatchEdit, TrackBatchError, TrackBatchResult, TrackRef, apply_track_batch,
};
pub use management::{TrackEdit, edit_tracks};
pub use snapshot::{TrackMix, TrackSnapshot, TrackSnapshotEntry};
pub use validation::{validate_track_layout, validate_track_structure};

pub const TRACK_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(pub u32);
static NEXT_TRACK_ID: AtomicU32 = AtomicU32::new(9);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Drum,
    Synth,
    Effect,
    Audio,
    Group,
}

impl TrackKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Drum => "Drum",
            Self::Synth => "Synth",
            Self::Effect => "Effect",
            Self::Audio => "Audio",
            Self::Group => "Grupo",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub name: Cow<'static, str>,
    pub kind: TrackKind,
    /// Instrumento asignado. El host conserva una instancia de piano y una de sampler.
    pub instrument: Option<InstrumentId>,
    pub muted: bool,
    pub solo: bool,
    pub parent: Option<TrackId>,
    /// Topología FX recursiva; datos de proyecto, sin instancias DSP.
    pub chain: FxChain,
}

/// Identidad de plugin. La variante VST3 reserva el contrato; todavía no existe su host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackDevice {
    Hay { catalog_id: String },
    Vst3 { class_id: String },
}

impl Track {
    pub fn new(name: impl Into<String>, kind: TrackKind) -> Self {
        Self {
            id: TrackId(NEXT_TRACK_ID.fetch_add(1, Ordering::Relaxed)),
            name: Cow::Owned(name.into()),
            kind,
            instrument: None,
            muted: false,
            solo: false,
            parent: None,
            chain: FxChain::new(),
        }
    }
    /// Evita colisiones al volver a insertar pistas después de abrir un proyecto.
    pub fn reserve_restored_id(id: TrackId) -> Result<(), String> {
        let next = id.0.checked_add(1).ok_or("Se agotaron los IDs de pista")?;
        NEXT_TRACK_ID.fetch_max(next, Ordering::Relaxed);
        Ok(())
    }
    pub fn accepts_clips(&self) -> bool {
        self.kind != TrackKind::Group
    }

    pub fn is_audible(&self, solo_active: bool) -> bool {
        !self.muted && (!solo_active || self.solo)
    }
}

pub fn has_solo(tracks: &[Track]) -> bool {
    tracks.iter().any(|track| track.solo)
}

pub const DEMO_TRACKS: [Track; TRACK_COUNT] = [
    track(1, "Kick", TrackKind::Drum),
    track(2, "Snare", TrackKind::Drum),
    track(3, "Hi-Hat", TrackKind::Drum),
    track(4, "Tom", TrackKind::Drum),
    track(5, "Bass", TrackKind::Synth),
    track(6, "Keys", TrackKind::Synth),
    track(7, "FX", TrackKind::Effect),
    track(8, "Vox", TrackKind::Audio),
];

const fn track(id: u32, name: &'static str, kind: TrackKind) -> Track {
    Track {
        id: TrackId(id),
        name: Cow::Borrowed(name),
        kind,
        instrument: None,
        muted: false,
        solo: false,
        parent: None,
        chain: FxChain::new(),
    }
}

/// Asigna un instrumento a una pista y lo retira de cualquier otra.
/// Devuelve la pista anterior cuando el instrumento se movió de sitio.
pub fn assign_instrument(
    tracks: &mut [Track],
    index: usize,
    instrument: InstrumentId,
) -> Option<usize> {
    if index >= tracks.len() {
        return None;
    }
    let previous = tracks
        .iter()
        .position(|track| track.instrument == Some(instrument));
    if let Some(previous) = previous {
        tracks[previous].instrument = None;
    }
    tracks[index].instrument = Some(instrument);
    previous.filter(|&previous| previous != index)
}

#[cfg(test)]
mod tests {
    use super::{DEMO_TRACKS, TRACK_COUNT, assign_instrument, has_solo};
    use crate::musical::model::InstrumentId;
    use std::collections::HashSet;

    #[test]
    fn demo_tracks_have_unique_ids_and_a_free_instrument_slot() {
        assert_eq!(DEMO_TRACKS.len(), TRACK_COUNT);
        let mut ids = HashSet::new();
        for track in DEMO_TRACKS {
            assert!(ids.insert(track.id), "ID duplicado: {:?}", track.id);
            assert!(track.instrument.is_none());
        }
    }

    #[test]
    fn assigning_moves_the_single_instrument_between_tracks() {
        let mut tracks = DEMO_TRACKS;
        let piano = InstrumentId(1);
        assert_eq!(assign_instrument(&mut tracks, 1, piano), None);
        assert_eq!(tracks[1].instrument, Some(piano));
        assert_eq!(assign_instrument(&mut tracks, 4, piano), Some(1));
        assert_eq!(tracks[1].instrument, None);
        assert_eq!(tracks[4].instrument, Some(piano));
        assert_eq!(assign_instrument(&mut tracks, 4, piano), None);
        assert_eq!(tracks[4].instrument, Some(piano));
        assert_eq!(assign_instrument(&mut tracks, TRACK_COUNT, piano), None);
    }

    #[test]
    fn mute_and_solo_define_a_reproducible_audibility_policy() {
        let mut tracks = DEMO_TRACKS;
        tracks[1].muted = true;
        tracks[4].solo = true;
        assert!(!tracks[0].is_audible(has_solo(&tracks)));
        assert!(!tracks[1].is_audible(has_solo(&tracks)));
        assert!(tracks[4].is_audible(has_solo(&tracks)));
    }
}
