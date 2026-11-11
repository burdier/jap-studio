//! Descriptor headless. El host conecta una captura al nodo; el dispositivo es transparente.
use super::{Device, DeviceDescriptor};
use crate::{ComponentError, ParameterId, PrimitiveError, StereoFrame};

pub const ID: &str = "hay.spectrum";
pub const DESCRIPTOR: DeviceDescriptor = DeviceDescriptor {
    id: ID,
    name: "Hay Spectrum",
    parameters: &[],
    create: |_| Ok(Box::new(Spectrum)),
};
struct Spectrum;
impl Device for Spectrum {
    fn set_parameter(&mut self, parameter: ParameterId, _: f32) -> Result<(), ComponentError> {
        Err(PrimitiveError::UnknownParameter(parameter).into())
    }
    fn set_bypassed(&mut self, _: bool) -> Result<(), ComponentError> {
        Ok(())
    }
    fn process(&mut self, _: &mut [StereoFrame]) {}
    fn reset(&mut self) {}
}
