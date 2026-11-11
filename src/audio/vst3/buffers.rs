//! Buffers de todos los buses, reservados antes de procesar; solo salida principal activa.
use vst3::{
    ComPtr,
    Steinberg::{Vst::*, kResultOk},
};
pub(super) struct Buffers {
    pub inputs: Vec<AudioBusBuffers>,
    pub outputs: Vec<AudioBusBuffers>,
    input_samples: Vec<Vec<Box<[f32; 512]>>>,
    output_samples: Vec<Vec<Box<[f32; 512]>>>,
    _pointers: Vec<Vec<*mut f32>>,
}
impl Buffers {
    pub fn empty() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            input_samples: Vec::new(),
            output_samples: Vec::new(),
            _pointers: Vec::new(),
        }
    }
    pub fn prepare(component: &ComPtr<IComponent>, effect: bool) -> Result<Self, String> {
        let mut buffers = Self::empty();
        for direction in 0..=1 {
            let count = unsafe { component.getBusCount(0, direction) };
            if !(0..=16).contains(&count) {
                return Err("VST3: cantidad de buses no compatible".into());
            }
            for index in 0..count {
                let mut info: BusInfo = unsafe { std::mem::zeroed() };
                super::check(
                    unsafe { component.getBusInfo(0, direction, index, &mut info) },
                    "getBusInfo",
                )?;
                if !(1..=64).contains(&info.channelCount) {
                    return Err("VST3: número de canales no compatible".into());
                }
                if (direction == 1 || effect) && index == 0 && info.channelCount != 2 {
                    return Err("VST3: salida principal no estéreo".into());
                }
                let mut samples: Vec<_> = (0..info.channelCount)
                    .map(|_| Box::new([0.; 512]))
                    .collect();
                let mut pointers: Vec<_> = samples
                    .iter_mut()
                    .map(|channel| channel.as_mut_ptr())
                    .collect();
                let bus = AudioBusBuffers {
                    numChannels: info.channelCount,
                    silenceFlags: u64::MAX,
                    __field0: AudioBusBuffers__type0 {
                        channelBuffers32: pointers.as_mut_ptr(),
                    },
                };
                buffers._pointers.push(pointers);
                if direction == 0 {
                    buffers.input_samples.push(samples);
                    buffers.inputs.push(bus);
                } else {
                    buffers.output_samples.push(samples);
                    buffers.outputs.push(bus);
                }
                let active = index == 0 && (direction == 1 || effect);
                let result =
                    unsafe { component.activateBus(0, direction, index, u8::from(active)) };
                if active && result != kResultOk {
                    return Err("VST3: no se pudo activar la salida principal".into());
                }
            }
        }
        if effect && buffers.inputs.is_empty() {
            return Err("VST3: el efecto no tiene entrada de audio".into());
        }
        if buffers.outputs.is_empty() {
            return Err("VST3 sin salida de audio".into());
        }
        Ok(buffers)
    }
    pub fn input(&mut self, frames: &[[f32; 2]]) {
        if let Some(bus) = self.input_samples.first_mut() {
            for (i, frame) in frames.iter().enumerate() {
                bus[0][i] = frame[0];
                bus[1][i] = frame[1];
            }
            self.inputs[0].silenceFlags = 0;
        }
    }
    pub fn clear(&mut self) {
        for bus in &mut self.outputs {
            bus.silenceFlags = 0;
        }
        for bus in &mut self.output_samples {
            for channel in bus {
                channel.fill(0.);
            }
        }
    }
    pub fn copy_output(&self, left: &mut [f32], right: &mut [f32]) {
        left.copy_from_slice(&self.output_samples[0][0][..left.len()]);
        right.copy_from_slice(&self.output_samples[0][1][..right.len()]);
    }
}
