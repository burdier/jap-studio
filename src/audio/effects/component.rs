//! Instancia de componente Lua preparada fuera de RT y compartida con un plan FX.
use elevate::{
    ParameterId, ProcessSpec,
    devices::{Device, audio_component::AudioComponentDevice},
    scripting::audio_components::AudioComponent,
};
use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

pub(crate) struct ComponentEffect {
    descriptor: Arc<AudioComponent>,
    sample_rate: u32,
    processor: UnsafeCell<AudioComponentDevice>,
    controls: Vec<(ParameterId, AtomicU32)>,
    busy: AtomicBool,
}

// process es el único escritor del procesador; la UI solo publica valores atómicos.
unsafe impl Send for ComponentEffect {}
unsafe impl Sync for ComponentEffect {}

impl ComponentEffect {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn descriptor(&self) -> &Arc<AudioComponent> {
        &self.descriptor
    }
    pub fn prepare(descriptor: Arc<AudioComponent>, rate: u32) -> Result<Arc<Self>, String> {
        let spec = ProcessSpec::new(rate as f64).map_err(|e| e.to_string())?;
        let processor =
            AudioComponentDevice::new(descriptor.clone(), spec).map_err(|e| e.to_string())?;
        let controls = descriptor
            .controls
            .iter()
            .map(|c| (ParameterId(c.id), AtomicU32::new(c.default.to_bits())))
            .collect();
        Ok(Arc::new(Self {
            descriptor,
            sample_rate: rate,
            processor: UnsafeCell::new(processor),
            controls,
            busy: AtomicBool::new(false),
        }))
    }

    pub fn set(&self, parameter: u32, value: f32) -> Result<(), String> {
        let control = self
            .controls
            .iter()
            .find(|(id, _)| id.0 == parameter)
            .ok_or("Control inexistente")?;
        let spec = self
            .descriptor
            .controls
            .iter()
            .find(|c| c.id == parameter)
            .ok_or("Control inexistente")?;
        if !spec.accepts(value) {
            return Err("Valor de control fuera de rango".into());
        }
        control.1.store(value.to_bits(), Ordering::Release);
        Ok(())
    }

    pub fn process(&self, frames: &mut [[f32; 2]]) {
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            frames.fill([0.0; 2]);
            return;
        }
        let processor = unsafe { &mut *self.processor.get() };
        for (id, value) in &self.controls {
            let _ = processor.set_parameter(*id, f32::from_bits(value.load(Ordering::Acquire)));
        }
        processor.process(frames);
        self.busy.store(false, Ordering::Release);
    }
}
