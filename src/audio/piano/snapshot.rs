use super::PianoSettings;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct PianoSnapshot {
    pub path: PathBuf,
    pub settings: PianoSettings,
    pub midi: Vec<(u8, u8, u8)>,
    pub master_volume: f32,
    pub filter: elevate::devices::filter::FilterSettings,
}
