//! Exportación offline: snapshot → bloques mezclados → codecs C. No importa GPUI/CPAL.
mod files;
mod job;
mod lame;
mod native;
mod render;
mod sndfile;
mod sources;
mod soxr;
#[cfg(test)]
mod tests;

use super::{AudioClip, piano::PianoSnapshot};
use elevate::{devices::utility_slot::UtilityState, musical::clip::MidiClip};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32},
        mpsc::Receiver,
    },
};

pub(crate) use job::start;

#[derive(Clone)]
pub(crate) struct TrackMix {
    pub name: String,
    pub gain: f32,
    pub fx_gain: f32,
    pub pan: f32,
}

pub(crate) struct Snapshot {
    pub audio: Vec<AudioClip>,
    pub midi: Vec<MidiClip>,
    pub piano: Option<PianoSnapshot>,
    pub sampler: Option<(Vec<usize>, super::sampler::SamplerSnapshot)>,
    pub tracks: Vec<TrackMix>,
    pub utility: UtilityState,
    pub bpm: f64,
}

#[derive(Debug)]
pub(crate) struct Report {
    pub paths: Vec<PathBuf>,
    /// Muestras de salida por encima de ±1 antes de convertir; float conserva headroom.
    pub overs: u64,
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
