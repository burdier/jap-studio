//! Adaptadores de audio del prototipo. No importar GPUI desde este módulo.
//! Los reproductores de clip, piano y percusión son excluyentes; ver docs/PROTOTYPE.md.

mod assets;
pub(crate) mod drums;
pub(crate) mod export;
mod fx_gain;
mod midi_in;
mod output;
pub(crate) mod piano;
mod playback;
pub(crate) mod sampler;
mod stereo_pan;

pub(crate) use assets::{AudioClip, AudioFile, PEAK_FRAMES};
pub(crate) use drums::DrumPlayer;
pub(crate) use midi_in::MidiHardwareIn;
pub(crate) use playback::Player;

// Adaptador temporal del reproductor de clips, hasta conectar el transporte común.
pub(crate) const BEATS_PER_SECOND: f64 = elevate::musical::time::DEFAULT_BPM / 60.0;
