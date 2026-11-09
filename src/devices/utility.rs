//! Utility v1: componente con un nodo Volume. No contiene algoritmos DSP propios.
use super::{Device, DeviceDescriptor};
use crate::{
    Component, ComponentError, ControlEvent, NodeId, ParameterId, ProcessSpec, StereoFrame,
    primitives::amplitude::Volume,
};

pub const DESCRIPTOR: DeviceDescriptor = DeviceDescriptor {
    id: "hay.utility",
    name: "Hay Utility",
    parameters: Volume::INFO.parameters,
    create: |spec| Ok(Box::new(Utility::new(spec)?)),
};

pub struct Utility {
    component: Component,
    volume: NodeId,
}

impl Utility {
    pub const GAIN: ParameterId = Volume::GAIN;

    pub fn new(spec: ProcessSpec) -> Result<Self, ComponentError> {
        let mut component = Component::new(DESCRIPTOR.name, spec);
        let volume = component.add(Box::new(Volume::new(spec)))?;
        Ok(Self { component, volume })
    }
}

impl Device for Utility {
    fn set_parameter(&mut self, parameter: ParameterId, value: f32) -> Result<(), ComponentError> {
        self.component.apply(ControlEvent::SetParameter {
            node: self.volume,
            parameter,
            value,
        })
    }

    fn set_bypassed(&mut self, bypassed: bool) -> Result<(), ComponentError> {
        self.component.apply(ControlEvent::SetBypassed {
            node: self.volume,
            bypassed,
        })
    }

    fn process(&mut self, frames: &mut [StereoFrame]) {
        self.component.process(frames);
    }

    fn reset(&mut self) {
        self.component.reset();
    }
}
