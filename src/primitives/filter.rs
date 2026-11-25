//! Filtro SVF y EQ paramétrico estéreo de FunDSP (LP, HP, BP, Notch, Bell, LowShelf, HighShelf).
//! Estado y suavizado propios por instancia; sin reservas en RT.
use super::*;
use fundsp::prelude64::{
    AudioUnit, bandpass, bandpass_hz, bell, bell_hz, highpass, highpass_hz, highshelf,
    highshelf_hz, lowpass, lowpass_hz, lowshelf, lowshelf_hz, notch, notch_hz,
};

pub const MODE_LP: usize = 0;
pub const MODE_HP: usize = 1;
pub const MODE_BP: usize = 2;
pub const MODE_NOTCH: usize = 3;
pub const MODE_BELL: usize = 4;
pub const MODE_LOW_SHELF: usize = 5;
pub const MODE_HIGH_SHELF: usize = 6;
pub const MODE_COUNT: usize = 7;

const PARAMS: [ParameterSpec; 4] = [
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
        max: 6.,
        default: 0.,
    },
    ParameterSpec {
        id: ParameterId(3),
        key: "gain",
        name: "Ganancia",
        unit: "dB",
        min: -18.,
        max: 18.,
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
    processors: [[Box<dyn AudioUnit>; MODE_COUNT]; 2],
    values: [f32; 4],
    /// Suavizados: [cutoff_hz, q, gain_linear]
    current: [f32; 3],
    weights: [f32; MODE_COUNT],
    rate: f64,
    slew: f32,
    bypassed: bool,
}

#[inline]
fn db_to_lin(db: f32) -> f32 {
    10.0_f32.powf(db.clamp(-18.0, 18.0) / 20.0)
}

impl Filter {
    pub const INFO: &'static PrimitiveInfo = &INFO;
    pub fn new(spec: ProcessSpec) -> Self {
        let processors = std::array::from_fn(|_| {
            [
                Box::new(lowpass()) as Box<dyn AudioUnit>,
                Box::new(highpass()),
                Box::new(bandpass()),
                Box::new(notch()),
                Box::new(bell()),
                Box::new(lowshelf()),
                Box::new(highshelf()),
            ]
        });
        let mut result = Self {
            processors,
            values: [1000., 0.707, 0., 0.],
            current: [1000., 0.707, 1.0],
            weights: [1., 0., 0., 0., 0., 0., 0.],
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
                max: 6.,
            });
        }
        Ok(())
    }
    /// Respuesta estacionaria clásica (LP/HP/BP) sin ganancia; mantiene compatibilidad.
    pub fn response(rate: f64, values: [f32; 3]) -> Vec<(f32, f32)> {
        Self::response_eq(rate, [values[0], values[1], values[2], 0.0])
    }

    /// Respuesta estacionaria de los 7 modos (incluidos Notch, Bell, LowShelf, HighShelf).
    pub fn response_eq(rate: f64, values: [f32; 4]) -> Vec<(f32, f32)> {
        let cutoff = values[0].clamp(20.0, rate as f32 * 0.45);
        let q = values[1].clamp(0.5, 10.0);
        let gain_lin = db_to_lin(values[3]);
        let mut unit: Box<dyn AudioUnit> = match values[2] as usize {
            MODE_HP => Box::new(highpass_hz(cutoff, q)),
            MODE_BP => Box::new(bandpass_hz(cutoff, q)),
            MODE_NOTCH => Box::new(notch_hz(cutoff, q)),
            MODE_BELL => Box::new(bell_hz(cutoff, q, gain_lin)),
            MODE_LOW_SHELF => Box::new(lowshelf_hz(cutoff, q, gain_lin)),
            MODE_HIGH_SHELF => Box::new(highshelf_hz(cutoff, q, gain_lin)),
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
        let target_mode = (self.values[2] as usize).min(MODE_COUNT - 1);
        let targets = [
            self.values[0].clamp(20.0, self.rate as f32 * 0.45),
            self.values[1].clamp(0.5, 10.0),
            db_to_lin(self.values[3]),
        ];
        for frame in frames {
            for (current, target) in self.current.iter_mut().zip(targets) {
                *current += (target - *current) * self.slew;
            }
            for (mode, weight) in self.weights.iter_mut().enumerate() {
                let target_w = if mode == target_mode { 1.0 } else { 0.0 };
                *weight += (target_w - *weight) * self.slew;
            }
            for (input, channel) in frame.iter_mut().zip(&mut self.processors) {
                let mut wet = 0.0_f32;
                for (mode, (unit, weight)) in channel.iter_mut().zip(self.weights).enumerate() {
                    if weight <= 1e-5 {
                        continue;
                    }
                    let mut out = [0.0_f32];
                    if mode >= MODE_BELL {
                        unit.tick(
                            &[*input, self.current[0], self.current[1], self.current[2]],
                            &mut out,
                        );
                    } else {
                        unit.tick(&[*input, self.current[0], self.current[1]], &mut out);
                    }
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
        self.current = [
            self.values[0].clamp(20.0, self.rate as f32 * 0.45),
            self.values[1].clamp(0.5, 10.0),
            db_to_lin(self.values[3]),
        ];
        let active = (self.values[2] as usize).min(MODE_COUNT - 1);
        self.weights = std::array::from_fn(|i| if i == active { 1. } else { 0. });
    }
    fn is_bypassed(&self) -> bool {
        self.bypassed
    }
    fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
    }
}
