//! Adaptadores de audio del prototipo. No importar GPUI desde este módulo.
//! Arrangement y Session comparten mezcla y reloj; la audición live conserva adaptadores propios.

pub(crate) mod analyzer;
mod assets;
pub(crate) mod audio_to_midi;
pub(crate) mod chord_slices;
mod clip_source;
pub(crate) mod crossfade;
pub(crate) mod device;
pub(crate) mod device_probe;
pub(crate) mod drums;
pub(crate) mod export;
pub(crate) mod fundamental;
mod fx_gain;
pub(crate) mod input_monitor;
pub(crate) mod metronome;
mod midi_in;
mod output;
pub(crate) mod piano;
mod playback;
mod publication;
pub(crate) mod recorder;
pub(crate) mod sample_analysis;
pub(crate) mod sample_edit;
mod sample_pitch;
pub(crate) mod sample_preview;
pub(crate) mod sample_warp;
pub(crate) mod sampler;
mod stereo_pan;
pub(crate) mod synth;
pub(crate) mod vst3;

pub(crate) use assets::{AudioClip, AudioFile, PEAK_FRAMES};
pub(crate) use midi_in::MidiHardwareIn;
pub(crate) use playback::{
    ChannelMeter, HiloBranchInput, MidiTrackInput, Player, PreparedPlayback, TrackInput,
    apply_preplay,
};

pub(crate) mod effects;
