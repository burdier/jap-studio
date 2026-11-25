//! Exportación offline: snapshot → bloques mezclados → codecs C. No importa GPUI/CPAL.
mod files;
mod job;
mod lame;
mod mastering;
pub(crate) use mastering::LoudnessSummary;
mod native;
mod plugins;
pub(crate) use plugins::Plugins;
mod render;
mod sndfile;
mod soxr;
#[cfg(test)]
mod tests;

use super::{TrackInput, drums::DrumInput};
use elevate::{
    devices::utility_slot::UtilityState,
    musical::{time::Meter, track::TrackId},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32},
        mpsc::Receiver,
    },
};

pub(crate) use job::start;

use super::playback::{Stem, StemBuffers};
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub inputs: Vec<TrackInput>,
    pub drums: Option<DrumInput>,
    pub tracks: Vec<(TrackId, String)>,
    pub utility: UtilityState,
    pub master_fx_gain: f32,
    pub master_rack: Option<Arc<super::effects::Rack>>,
    pub bpm: f64,
    pub meter: Meter,
}
#[derive(Clone, Debug)]
pub(crate) struct Report {
    pub paths: Vec<PathBuf>,
    /// Muestras de salida por encima de ±1 antes de convertir; float conserva headroom.
    pub overs: u64,
    /// Medición EBU R128 del resultado exportado (LUFS integrado, True Peak, LRA y ajustes).
    pub loudness: LoudnessSummary,
}

pub(crate) struct ExportJob {
    pub result: Receiver<Result<Report, String>>,
    pub progress: Arc<AtomicU32>,
    pub cancel: Arc<AtomicBool>,
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(crate) use soxr::resample;
