//! Filtro SVF estéreo de FunDSP. Estado y suavizado propios por instancia.
use super::*;
use fundsp::prelude64::{
    AudioUnit, bandpass, bandpass_hz, highpass, highpass_hz, lowpass, lowpass_hz,
};

const PARAMS: [ParameterSpec; 3] = [
    ParameterSpec {
        id: ParameterId(0),
        key: "cutoff",
        name: "Frecuencia",
        unit: "Hz",
        min: 20.,
        max: 20_000.,
        default: 1_000.,
    },
    ParameterSpec {
        id: ParameterId(1),
        key: "resonance",
        name: "Resonancia",
        unit: "Q",
        min: 0.5,
        max: 10.,
        default: 0.707,
    },
    ParameterSpec {
        id: ParameterId(2),
        key: "mode",
        name: "Tipo",
        unit: "tipo",
        min: 0.,
        max: 2.,
        default: 0.,
    },
];
const INFO: PrimitiveInfo = PrimitiveInfo {
    id: "elevate.filter.svf",
    name: "Filter",
    properties: &[SignalProperty::Frequency],
    parameters: &PARAMS,
};

pub struct Filter {
    processors: [[Box<dyn AudioUnit>; 3]; 2],
    values: [f32; 3],
    current: [f32; 2],
    weights: [f32; 3],
    rate: f64,
    slew: f32,
    bypassed: bool,
}
impl Filter {
    pub const INFO: &'static PrimitiveInfo = &INFO;
    pub fn new(spec: ProcessSpec) -> Self {
        let processors = std::array::from_fn(|_| {
            [
                Box::new(lowpass()) as Box<dyn AudioUnit>,
                Box::new(highpass()),
                Box::new(bandpass()),
            ]
        });
        let mut result = Self {
            processors,
            values: [1000., 0.707, 0.],
            current: [1000., 0.707],
            weights: [1., 0., 0.],
            rate: spec.sample_rate(),
            slew: 0.,
            bypassed: false,
        };
        result.prepare(spec);
        result
    }
    pub fn validate(id: ParameterId, value: f32) -> Result<(), PrimitiveError> {
        let spec = PARAMS
            .get(id.0 as usize)
            .ok_or(PrimitiveError::UnknownParameter(id))?;
        spec.validate(value)?;
        if id.0 == 2 && value.fract() != 0. {
            return Err(PrimitiveError::InvalidValue {
                parameter: id,
                value,
                min: 0.,
                max: 2.,
            });
        }
        Ok(())
    }
    /// Respuesta estacionaria de la misma implementación DSP; solo fuera de RT.
    pub fn response(rate: f64, values: [f32; 3]) -> Vec<(f32, f32)> {
        let cutoff = values[0].min(rate as f32 * 0.45);
        let q = values[1];
        let mut unit: Box<dyn AudioUnit> = match values[2] as u32 {
            1 => Box::new(highpass_hz(cutoff, q)),
            2 => Box::new(bandpass_hz(cutoff, q)),
            _ => Box::new(lowpass_hz(cutoff, q)),
        };
        unit.set_sample_rate(rate);
        (0..160)
            .map(|i| {
                let f = 20. * 1000_f64.powf(i as f64 / 159.);
                let db = if f < rate * 0.5 {
                    unit.response_db(0, f).unwrap_or(-60.) as f32
                } else {
                    -60.
                };
                (i as f32 / 159., db.clamp(-60., 24.))
            })
            .collect()
    }
}
impl Primitive for Filter {
    fn info(&self) -> &'static PrimitiveInfo {
        &INFO
    }
    fn parameter(&self, id: ParameterId) -> Result<f32, PrimitiveError> {
        self.values
            .get(id.0 as usize)
            .copied()
            .ok_or(PrimitiveError::UnknownParameter(id))
    }
    fn set_parameter(&mut self, id: ParameterId, value: f32) -> Result<(), PrimitiveError> {
        Self::validate(id, value)?;
        self.values[id.0 as usize] = value;
        Ok(())
    }
    fn prepare(&mut self, spec: ProcessSpec) {
        self.rate = spec.sample_rate();
        self.slew = (1. - (-1. / (self.rate * 0.005)).exp()) as f32;
        for unit in self.processors.iter_mut().flatten() {
            unit.set_sample_rate(self.rate);
            unit.allocate();
        }
        self.reset();
    }
    fn process(&mut self, frames: &mut [StereoFrame]) {
        for frame in frames {
            for (current, target) in self
                .current
                .iter_mut()
                .zip([self.values[0].min(self.rate as f32 * 0.45), self.values[1]])
            {
                *current += (target - *current) * self.slew;
            }
            for (mode, weight) in self.weights.iter_mut().enumerate() {
                *weight += ((if mode == self.values[2] as usize {
                    1.
                } else {
                    0.
                }) - *weight)
                    * self.slew;
            }
            for (input, channel) in frame.iter_mut().zip(&mut self.processors) {
                let mut wet = 0.;
                for (unit, weight) in channel.iter_mut().zip(self.weights) {
                    let mut out = [0.];
                    unit.tick(&[*input, self.current[0], self.current[1]], &mut out);
                    wet += out[0] * weight;
                }
                if !self.bypassed {
                    *input = wet;
                }
            }
        }
    }
    fn reset(&mut self) {
        for unit in self.processors.iter_mut().flatten() {
            unit.reset();
        }
        self.current = [self.values[0].min(self.rate as f32 * 0.45), self.values[1]];
        self.weights = std::array::from_fn(|i| if i == self.values[2] as usize { 1. } else { 0. });
    }
    fn is_bypassed(&self) -> bool {
        self.bypassed
    }
    fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }
}
