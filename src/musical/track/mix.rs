//! Resolución de mezcla por pista, compartida por controles UI y preparación de audio.
use super::{Track, has_solo};
pub const UNITY_LEVEL: i32 = 75;
#[derive(Debug, Clone, Copy)]
pub struct EffectiveMix {
    pub audible: bool,
    pub gain: f32,
    pub pan: f32,
}
pub fn effective_mix(tracks: &[Track], levels: &[i32], pans: &[i32], index: usize) -> EffectiveMix {
    let Some(track) = tracks.get(index) else {
        return EffectiveMix {
            audible: false,
            gain: 0.,
            pan: 0.,
        };
    };
    let parent = track
        .parent
        .and_then(|id| tracks.iter().position(|t| t.id == id));
    let parent_track = parent.map(|i| &tracks[i]);
    let audible = !track.muted
        && parent_track.is_none_or(|p| !p.muted)
        && (!has_solo(tracks) || track.solo || parent_track.is_some_and(|p| p.solo));
    let level = |i: usize| {
        levels.get(i).copied().unwrap_or(UNITY_LEVEL).clamp(0, 100) as f32 / UNITY_LEVEL as f32
    };
    let pan = |i: usize| (pans.get(i).copied().unwrap_or(50).clamp(0, 100) - 50) as f32 / 50.;
    EffectiveMix {
        audible,
        gain: if audible {
            (level(index) * parent.map_or(1., level)).clamp(0., 2.)
        } else {
            0.
        },
        pan: (pan(index) + parent.map_or(0., pan)).clamp(-1., 1.),
    }
}
