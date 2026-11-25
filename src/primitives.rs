use std::fmt;

pub mod amplitude;
pub mod compressor;
pub mod filter;
pub mod limiter;

pub type StereoFrame = [f32; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalProperty {
    Amplitude,
    Frequency,
    Timbre,
    Phase,
    Time,
    Spatial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParameterId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct ParameterSpec {
    pub id: ParameterId,
    pub key: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

impl ParameterSpec {
    pub fn validate(&self, value: f32) -> Result<(), PrimitiveError> {
        if !value.is_finite() || !(self.min..=self.max).contains(&value) {
            return Err(PrimitiveError::InvalidValue {
                parameter: self.id,
                value,
                min: self.min,
                max: self.max,
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PrimitiveInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub properties: &'static [SignalProperty],
    pub parameters: &'static [ParameterSpec],
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessSpec {
    sample_rate: f64,
}

impl ProcessSpec {
    pub fn new(sample_rate: f64) -> Result<Self, PrimitiveError> {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err(PrimitiveError::InvalidSampleRate(sample_rate));
        }
        Ok(Self { sample_rate })
    }

    pub fn sample_rate(self) -> f64 {
        self.sample_rate
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveError {
    InvalidSampleRate(f64),
    UnknownParameter(ParameterId),
    InvalidValue {
        parameter: ParameterId,
        value: f32,
        min: f32,
        max: f32,
    },
}

impl fmt::Display for PrimitiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSampleRate(rate) => {
                write!(formatter, "Frecuencia de muestreo inválida: {rate}")
            }
            Self::UnknownParameter(parameter) => {
                write!(formatter, "Parámetro desconocido: {}", parameter.0)
            }
            Self::InvalidValue {
                parameter,
                value,
                min,
                max,
            } => write!(
                formatter,
                "Valor {value} inválido para el parámetro {}; se espera un número finito entre {min} y {max}",
                parameter.0
            ),
        }
    }
}

impl std::error::Error for PrimitiveError {}

pub trait Primitive: Send {
    fn info(&self) -> &'static PrimitiveInfo;
    fn parameter(&self, parameter: ParameterId) -> Result<f32, PrimitiveError>;
    fn set_parameter(&mut self, parameter: ParameterId, value: f32) -> Result<(), PrimitiveError>;
    fn prepare(&mut self, specification: ProcessSpec);
    fn process(&mut self, frames: &mut [StereoFrame]);
    /// Procesa el bloque entregando una señal auxiliar de sidechain si el nodo la recibe.
    fn process_sidechain(
        &mut self,
        frames: &mut [StereoFrame],
        _sidechain: Option<&[StereoFrame]>,
    ) {
        self.process(frames);
    }
    /// Reducción de ganancia actual en dB positivos (para medidores GR de Compresor/Limiter).
    fn gain_reduction_db(&self) -> f32 {
        0.0
    }
    fn reset(&mut self);
    fn is_bypassed(&self) -> bool;
    fn set_bypassed(&mut self, bypassed: bool);
}
