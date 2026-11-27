//! Dispositivos internos: descriptor/fábrica públicos, DSP preparado fuera del callback.
pub mod audio_component;
pub mod filter;
pub mod fx_gain;
pub mod sampler;
pub mod spectrum;
pub mod synth;
pub mod utility;
pub mod utility_slot;
#[cfg(feature = "ui")]
pub mod vital_import;

use crate::{ComponentError, ParameterId, ParameterSpec, ProcessSpec, StereoFrame};

pub trait Device: Send {
    fn set_parameter(&mut self, parameter: ParameterId, value: f32) -> Result<(), ComponentError>;
    fn set_bypassed(&mut self, bypassed: bool) -> Result<(), ComponentError>;
    fn process(&mut self, frames: &mut [StereoFrame]);
    fn reset(&mut self);
}

pub struct DeviceDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub parameters: &'static [ParameterSpec],
    /// Solo construir/preparar desde el hilo de control.
    pub create: fn(ProcessSpec) -> Result<Box<dyn Device>, ComponentError>,
}

pub fn find(id: &str) -> Option<&'static DeviceDescriptor> {
    match id {
        "hay.utility" => Some(&utility::DESCRIPTOR),
        spectrum::ID => Some(&spectrum::DESCRIPTOR),
        _ => None,
    }
}
