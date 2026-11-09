//! Sampler headless sfizz. La preparación del instrumento se hace fuera del callback.
mod asset;
mod engine;
mod native;
mod sfz;

pub(crate) use asset::SampleAsset;
use elevate::devices::sampler::SamplerParams;
pub(crate) use engine::SfizzEngine;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct SamplerSnapshot {
    pub asset: Arc<SampleAsset>,
    pub params: SamplerParams,
}
