//! Pistas del proyecto. Sin voces, dispositivos ni coordenadas de pantalla.
//! La asignación de instrumento es estado musical; la UI solo la presenta.
use super::model::InstrumentId;

pub const TRACK_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Drum,
    Synth,
    Effect,
    Audio,
}

impl TrackKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Drum => "Drum",
            Self::Synth => "Synth",
            Self::Effect => "Effect",
            Self::Audio => "Audio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Track {
    pub id: TrackId,
    pub name: &'static str,
    pub kind: TrackKind,
    /// Instrumento asignado. El prototipo solo tiene una instancia de piano.
    pub instrument: Option<InstrumentId>,
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
        name,
        kind,
        instrument: None,
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
    use super::{DEMO_TRACKS, TRACK_COUNT, assign_instrument};
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
}
