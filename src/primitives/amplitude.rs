use fundsp::prelude64::{AudioUnit, Shared, U2, follow, multipass, shared, split, var};

use super::{
    ParameterId, ParameterSpec, Primitive, PrimitiveError, PrimitiveInfo, ProcessSpec,
    SignalProperty, StereoFrame,
};

const GAIN: ParameterSpec = ParameterSpec {
    id: ParameterId(0),
    key: "gain",
    name: "Ganancia",
    unit: "x",
    min: 0.0,
    max: 2.0,
    default: 1.0,
};

const INFO: PrimitiveInfo = PrimitiveInfo {
    id: "elevate.amplitude.volume",
    name: "Volume",
    properties: &[SignalProperty::Amplitude],
    parameters: &[GAIN],
};

pub struct Volume {
    gain: f32,
    bypassed: bool,
    effective_gain: Shared,
    processor: Box<dyn AudioUnit>,
}

impl Volume {
    pub const GAIN: ParameterId = GAIN.id;
    pub const SMOOTHING_SECONDS: f32 = 0.005;

    pub fn new(specification: ProcessSpec) -> Self {
        Self::build(specification, GAIN.default)
    }

    pub fn with_gain(specification: ProcessSpec, gain: f32) -> Result<Self, PrimitiveError> {
        GAIN.validate(gain)?;
        Ok(Self::build(specification, gain))
    }

    fn build(specification: ProcessSpec, gain: f32) -> Self {
        let effective_gain = shared(gain);
        // Un único suavizador alimenta ambos canales para conservar la imagen estéreo.
        let smoothed_gain =
            var(&effective_gain) >> follow(Self::SMOOTHING_SECONDS) >> split::<U2>();
        let mut processor: Box<dyn AudioUnit> = Box::new(multipass::<U2>() * smoothed_gain);
        processor.set_sample_rate(specification.sample_rate());
        processor.allocate();
        Self {
            gain,
            bypassed: false,
            effective_gain,
            processor,
        }
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn set_gain(&mut self, gain: f32) -> Result<(), PrimitiveError> {
        GAIN.validate(gain)?;
        self.gain = gain;
        if !self.bypassed {
            self.effective_gain.set_value(gain);
        }
        Ok(())
    }
}

impl Primitive for Volume {
    fn info(&self) -> &'static PrimitiveInfo {
        &INFO
    }

    fn parameter(&self, parameter: ParameterId) -> Result<f32, PrimitiveError> {
        if parameter == Self::GAIN {
            Ok(self.gain())
        } else {
            Err(PrimitiveError::UnknownParameter(parameter))
        }
    }

    fn set_parameter(&mut self, parameter: ParameterId, value: f32) -> Result<(), PrimitiveError> {
        if parameter == Self::GAIN {
            self.set_gain(value)
        } else {
            Err(PrimitiveError::UnknownParameter(parameter))
        }
    }

    fn prepare(&mut self, specification: ProcessSpec) {
        self.processor.set_sample_rate(specification.sample_rate());
        self.processor.allocate();
        self.reset();
    }

    fn process(&mut self, frames: &mut [StereoFrame]) {
        for frame in frames {
            let [left, right] = *frame;
            let (left, right) = self.processor.filter_stereo(left, right);
            *frame = [left, right];
        }
    }

    fn reset(&mut self) {
        self.processor.reset();
    }

    fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    fn set_bypassed(&mut self, bypassed: bool) {
        self.bypassed = bypassed;
        // Volver a ganancia unitaria mediante el mismo suavizado evita un salto al hacer bypass.
        self.effective_gain
            .set_value(if bypassed { 1.0 } else { self.gain });
    }
}
