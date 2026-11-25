//! Kit, sampler y bus de percusión para el mezclador de clips.
mod controls;
mod kit;
mod live;
mod mix;
mod sampler;
mod voice;

pub(crate) use controls::DrumBlock;
pub(crate) use live::LiveDrums;
pub(crate) use mix::{DrumInput, DrumPreview, DrumScore, PreparedDrums};
