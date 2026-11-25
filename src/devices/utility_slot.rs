//! Un slot de salida provisional: snapshot coherente y DSP preasignado.
//! Insertar/retirar cambia a ganancia unitaria suavizada, sin construir/destruir en RT.
use super::{
    Device,
    utility::{DESCRIPTOR, Utility},
};
use crate::{ComponentError, PrimitiveError, ProcessSpec, StereoFrame};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UtilityState {
    pub inserted: bool,
    pub gain: f32,
    pub bypassed: bool,
}

impl Default for UtilityState {
    fn default() -> Self {
        Self {
            inserted: false,
            gain: 1.0,
            bypassed: false,
        }
    }
}

#[derive(Clone, Copy)]
pub enum UtilityAction {
    Insert,
    Remove,
    SetGain(f32),
    ToggleBypass,
    Reset,
}

#[derive(Clone)]
pub struct UtilitySlot(Arc<AtomicU64>);

impl Default for UtilitySlot {
    fn default() -> Self {
        Self::new()
    }
}

impl UtilitySlot {
    pub fn new() -> Self {
        let slot = Self(Arc::new(AtomicU64::new(0)));
        slot.publish(UtilityState::default());
        slot
    }

    pub fn state(&self) -> UtilityState {
        let bits = self.0.load(Ordering::Relaxed);
        UtilityState {
            gain: f32::from_bits(bits as u32),
            inserted: bits & (1 << 32) != 0,
            bypassed: bits & (1 << 33) != 0,
        }
    }

    fn publish(&self, state: UtilityState) {
        self.0.store(
            u64::from(state.gain.to_bits())
                | (u64::from(state.inserted) << 32)
                | (u64::from(state.bypassed) << 33),
            Ordering::Relaxed,
        );
    }

    pub fn restore(&self, state: UtilityState) -> Result<(), PrimitiveError> {
        DESCRIPTOR.parameters[0].validate(state.gain)?;
        self.publish(state);
        Ok(())
    }

    /// Un único escritor (control/UI). Los clones entregados al audio solo leen.
    pub fn apply(&self, action: UtilityAction) -> Result<UtilityState, PrimitiveError> {
        let mut state = self.state();
        match action {
            UtilityAction::Insert => state.inserted = true,
            UtilityAction::Remove => state = UtilityState::default(),
            UtilityAction::Reset => {
                state.gain = 1.0;
                state.bypassed = false;
            }
            UtilityAction::SetGain(gain) => {
                DESCRIPTOR.parameters[0].validate(gain)?;
                state.inserted = true;
                state.gain = gain;
            }
            UtilityAction::ToggleBypass => {
                state.inserted = true;
                state.bypassed = !state.bypassed;
            }
        }
        self.publish(state);
        Ok(state)
    }

    pub fn prepare(&self, spec: ProcessSpec) -> Result<PreparedUtility, ComponentError> {
        let descriptor = super::find(DESCRIPTOR.id).expect("Utility registrado");
        let state = self.state();
        let mut device = (descriptor.create)(spec)?;
        device.set_parameter(Utility::GAIN, state.gain)?;
        device.set_bypassed(!state.inserted || state.bypassed)?;
        device.reset();
        Ok(PreparedUtility {
            slot: self.clone(),
            device,
            last: Some(state),
        })
    }
}

pub struct PreparedUtility {
    slot: UtilitySlot,
    device: Box<dyn Device>,
    last: Option<UtilityState>,
}

impl PreparedUtility {
    /// Trabajo acotado por bloque; sin asignaciones ni bloqueos.
    pub fn process(&mut self, frames: &mut [StereoFrame]) {
        let state = self.slot.state();
        if self.last != Some(state) {
            // El slot solo publica valores validados y el mapeo es fijo.
            let _ = self.device.set_parameter(Utility::GAIN, state.gain);
            let _ = self.device.set_bypassed(!state.inserted || state.bypassed);
            self.last = Some(state);
        }
        self.device.process(frames);
    }
}
