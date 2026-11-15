//! Adaptadores de audio del prototipo. No importar GPUI desde este módulo.
//! Arrangement y Session comparten mezcla y reloj; la audición live conserva adaptadores propios.

pub(crate) mod analyzer;
mod assets;
mod clip_source;
pub(crate) mod device;
pub(crate) mod device_probe;
pub(crate) mod drums;
pub(crate) mod export;
mod fx_gain;
mod midi_in;
mod output;
pub(crate) mod piano;
mod playback;
mod publication;
pub(crate) mod sampler;
mod stereo_pan;
pub(crate) mod vst3;

pub(crate) use assets::{AudioClip, AudioFile, PEAK_FRAMES};
pub(crate) use midi_in::MidiHardwareIn;
pub(crate) use playback::{HiloBranchInput, MidiTrackInput, Player, PreparedPlayback, TrackInput};

pub(crate) mod effects;
