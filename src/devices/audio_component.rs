//! Instancia nativa de un grafo Lua validado; buffers y nodos se preparan fuera de RT.
use super::Device;
use crate::{
    ComponentError, ParameterId, ProcessSpec, StereoFrame,
    primitives::{Primitive, amplitude::Volume, filter::Filter},
    scripting::audio_components::{AudioComponent, NativeKind},
};
use std::sync::Arc;

const BLOCK: usize = 512;

pub struct AudioComponentDevice {
    descriptor: Arc<AudioComponent>,
    processors: Vec<Box<dyn Primitive>>,
    incoming: Vec<Vec<usize>>,
    buffers: Vec<Box<[StereoFrame; BLOCK]>>,
    scratch: Box<[StereoFrame; BLOCK]>,
    bypassed: bool,
}

impl AudioComponentDevice {
    pub fn new(descriptor: Arc<AudioComponent>, spec: ProcessSpec) -> Result<Self, ComponentError> {
        let count = descriptor.nodes.len();
        let mut processors = Vec::with_capacity(count);
        for node in &descriptor.nodes {
            let mut primitive: Box<dyn Primitive> = match node.kind {
                NativeKind::Volume => Box::new(Volume::new(spec)),
                NativeKind::Filter => Box::new(Filter::new(spec)),
            };
            for (id, value) in node.values.iter().enumerate() {
                primitive.set_parameter(ParameterId(id as u32), *value)?;
            }
            primitive.reset();
            processors.push(primitive);
        }
        let mut incoming = vec![Vec::new(); count + 2];
        for &(from, to) in &descriptor.edges {
            incoming[to].push(from);
        }
        let buffers = (0..=count).map(|_| Box::new([[0.0; 2]; BLOCK])).collect();
        Ok(Self {
            descriptor,
            processors,
            incoming,
            buffers,
            scratch: Box::new([[0.0; 2]; BLOCK]),
            bypassed: false,
        })
    }

    pub fn descriptor(&self) -> &AudioComponent {
        &self.descriptor
    }

    fn process_chunk(&mut self, frames: &mut [StereoFrame]) {
        let count = self.processors.len();
        self.buffers[count][..frames.len()].copy_from_slice(frames);
        for &node in &self.descriptor.order {
            if node >= count {
                continue;
            }
            let scratch = &mut self.scratch[..frames.len()];
            scratch.fill([0.0; 2]);
            for &source in &self.incoming[node] {
                for (out, input) in scratch.iter_mut().zip(self.buffers[source].iter()) {
                    out[0] += input[0];
                    out[1] += input[1];
                }
            }
            self.processors[node].process(scratch);
            self.buffers[node][..frames.len()].copy_from_slice(scratch);
        }
        frames.fill([0.0; 2]);
        for &source in &self.incoming[count + 1] {
            for (out, input) in frames.iter_mut().zip(self.buffers[source].iter()) {
                out[0] += input[0];
                out[1] += input[1];
            }
        }
    }
}

impl Device for AudioComponentDevice {
    fn set_parameter(&mut self, parameter: ParameterId, value: f32) -> Result<(), ComponentError> {
        let control = self
            .descriptor
            .controls
            .iter()
            .find(|control| control.id == parameter.0)
            .ok_or(crate::PrimitiveError::UnknownParameter(parameter))?;
        if !control.accepts(value) {
            return Err(crate::PrimitiveError::InvalidValue {
                parameter,
                value,
                min: control.min,
                max: control.max,
            }
            .into());
        }
        for &(node, parameter) in &control.targets {
            self.processors[node].set_parameter(parameter, value)?;
        }
        Ok(())
    }

    fn set_bypassed(&mut self, bypassed: bool) -> Result<(), ComponentError> {
        self.bypassed = bypassed;
        Ok(())
    }

    fn process(&mut self, frames: &mut [StereoFrame]) {
        if self.bypassed {
            return;
        }
        for chunk in frames.chunks_mut(BLOCK) {
            self.process_chunk(chunk);
        }
    }

    fn reset(&mut self) {
        for node in &mut self.processors {
            node.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::audio_components::parse;

    #[test]
    fn two_independent_instances_process_and_bypass_real_audio() {
        let source = include_str!("../../assets/scripts/components/audio/double_volume.lua");
        let descriptor = Arc::new(parse(source).unwrap());
        let spec = ProcessSpec::new(48_000.0).unwrap();
        let mut first = AudioComponentDevice::new(descriptor.clone(), spec).unwrap();
        let mut second = AudioComponentDevice::new(descriptor, spec).unwrap();
        first.set_parameter(ParameterId(0), 0.5).unwrap();
        let mut signal = vec![[0.8, -0.4]; 4096];
        first.process(&mut signal);
        assert!((signal.last().unwrap()[0] - 0.2).abs() < 0.005);
        assert!((signal.last().unwrap()[1] + 0.1).abs() < 0.005);
        let mut unchanged = vec![[0.8, -0.4]; 4096];
        second.process(&mut unchanged);
        assert!((unchanged.last().unwrap()[0] - 0.8).abs() < 0.005);
        first.set_bypassed(true).unwrap();
        let mut dry = vec![[0.8, -0.4]; 64];
        first.process(&mut dry);
        assert_eq!(dry[0], [0.8, -0.4]);
    }

    #[test]
    fn parallel_paths_sum_without_normalizing() {
        let source = include_str!("../../assets/scripts/components/audio/double_volume.lua")
            .replace(
                "{ from = \"first\", to = \"second\" },",
                "{ from = \"input\", to = \"second\" },",
            )
            .replace(
                "{ from = \"second\", to = \"output\" },",
                "{ from = \"first\", to = \"output\" },\n{ from = \"second\", to = \"output\" },",
            );
        let descriptor = Arc::new(parse(&source).unwrap());
        let mut device =
            AudioComponentDevice::new(descriptor, ProcessSpec::new(48_000.0).unwrap()).unwrap();
        let mut frames = vec![[0.25, -0.25]; 1024];
        device.process(&mut frames);
        assert!((frames.last().unwrap()[0] - 0.5).abs() < 0.005);
        assert!((frames.last().unwrap()[1] + 0.5).abs() < 0.005);
    }
}
