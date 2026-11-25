//! Convolver portátil: IR estéreo PCM16 (hasta 10 s) + presets de espacio/cabina y parámetros.
use std::sync::Arc;

pub const ID: &str = "hey.convolver";
pub const MODELS: [&str; 4] = ["Room", "Hall", "Plate", "Cab 4×12"];
pub const MAX_IR_SECONDS: usize = 10;

pub const MIC_PARAMETER: u32 = 1;
pub const MIX_PARAMETER: u32 = 2;
pub const GAIN_PARAMETER: u32 = 3;
pub const PREDELAY_PARAMETER: u32 = 4;
pub const DECAY_PARAMETER: u32 = 5;

pub const MIC_POSITIONS: [&str; 3] = ["Brillante", "Natural", "Cálido"];

pub fn mic_index(value: Option<&f32>) -> Result<usize, String> {
    let value = value.copied().unwrap_or(1.);
    MIC_POSITIONS
        .iter()
        .enumerate()
        .position(|(index, _)| index as f32 == value)
        .ok_or_else(|| "Tono de convolución inválido".to_owned())
}

pub fn mix_value(value: Option<&f32>, has_ir: bool, model: usize) -> f32 {
    let default = if has_ir || model == 3 { 1.0 } else { 0.35 };
    value.copied().unwrap_or(default).clamp(0.0, 1.0)
}

pub fn gain_db_value(value: Option<&f32>) -> f32 {
    value.copied().unwrap_or(0.0).clamp(-12.0, 12.0)
}

pub fn predelay_ms_value(value: Option<&f32>) -> f32 {
    value.copied().unwrap_or(0.0).clamp(0.0, 120.0)
}

pub fn decay_value(value: Option<&f32>) -> f32 {
    value.copied().unwrap_or(1.0).clamp(0.1, 1.0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct CabinetImpulse {
    pub name: String,
    pub rate: u32,
    pub pcm: Arc<[u8]>,
}

impl CabinetImpulse {
    pub fn new(name: String, rate: u32, pcm: Vec<u8>) -> Result<Self, String> {
        if !(8000..=192000).contains(&rate)
            || name.len() > 255
            || pcm.is_empty()
            || pcm.len() % 4 != 0
            || pcm.len() > rate as usize * 4 * MAX_IR_SECONDS
        {
            return Err(format!(
                "IR inválida: usa un WAV de hasta {MAX_IR_SECONDS} s entre 8 y 192 kHz"
            ));
        }
        Ok(Self {
            name,
            rate,
            pcm: pcm.into(),
        })
    }

    pub fn duration_seconds(&self) -> f64 {
        (self.pcm.len() / 4) as f64 / f64::from(self.rate.max(1))
    }
}
