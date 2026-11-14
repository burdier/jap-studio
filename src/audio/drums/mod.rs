//! Audición provisional de percusión; comparte el Mix y su transporte por frames.
mod mix;
mod voice;

pub(crate) use mix::{DrumInput, PreparedDrums};
