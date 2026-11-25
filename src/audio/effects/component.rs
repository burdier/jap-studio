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

struct PreparedComponent {
    processor: UnsafeCell<AudioComponentDevice>,
}
// El CAS de ComponentEffect serializa escritores; preparación publica por RCU.
unsafe impl Send for PreparedComponent {}
unsafe impl Sync for PreparedComponent {}

pub(crate) struct ComponentEffect {
    descriptor: Arc<AudioComponent>,
    processor: crate::audio::publication::Publication<
        std::collections::BTreeMap<u32, Arc<PreparedComponent>>,
    >,
    preparation: std::sync::Mutex<()>,
    active_rate: AtomicU32,
    controls: Vec<(ParameterId, AtomicU32)>,
    busy: AtomicBool,
}

// process es el único escritor del procesador; la UI solo publica valores atómicos.
unsafe impl Send for ComponentEffect {}
unsafe impl Sync for ComponentEffect {}

impl ComponentEffect {
    pub fn sample_rate(&self) -> u32 {
        self.active_rate.load(Ordering::Relaxed)
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
            processor: crate::audio::publication::Publication::new(
                std::collections::BTreeMap::from([(
                    rate,
                    Arc::new(PreparedComponent {
                        processor: UnsafeCell::new(processor),
                    }),
                )]),
            ),
            active_rate: AtomicU32::new(rate),
            preparation: std::sync::Mutex::new(()),
            controls,
            busy: AtomicBool::new(false),
        }))
    }

    pub fn prepare_rate(&self, rate: u32) {
        if self.processor.read(|p| p.contains_key(&rate)) {
            return;
        }
        let _guard = self.preparation.lock().unwrap();
        if self.processor.read(|p| p.contains_key(&rate)) {
            return;
        }
        // rate proviene de una configuración CPAL válida; el descriptor ya se validó.
        let spec = ProcessSpec::new(rate as f64).expect("Frecuencia de salida válida");
        let mut processor = AudioComponentDevice::new(self.descriptor.clone(), spec)
            .expect("Descriptor preparado previamente");
        for (id, value) in &self.controls {
            processor
                .set_parameter(*id, f32::from_bits(value.load(Ordering::Acquire)))
                .expect("Control validado");
        }
        use elevate::devices::Device;
        processor.reset();
        let mut prepared = self.processor.read(|p| p.clone());
        prepared.insert(
            rate,
            Arc::new(PreparedComponent {
                processor: UnsafeCell::new(processor),
            }),
        );
        self.processor.publish(prepared);
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

    pub fn process(&self, frames: &mut [[f32; 2]], rate: u32) {
        self.process_automated(frames, rate, None);
    }

    pub fn process_automated(
        &self,
        frames: &mut [[f32; 2]],
        rate: u32,
        automated: Option<(
            &elevate::musical::track_automation::TrackAutomation,
            elevate::musical::fx::FxId,
            f64,
        )>,
    ) {
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            frames.fill([0.0; 2]);
            return;
        }
        self.processor.read(|variants| {
            let Some(prepared) = variants.get(&rate) else {
                frames.fill([0.; 2]);
                return;
            };
            self.active_rate.store(rate, Ordering::Relaxed);
            let processor = unsafe { &mut *prepared.processor.get() };
            for (id, value) in &self.controls {
                let effective = if let Some((auto, fx_id, beat)) = automated
                    && let Some(norm) = auto.evaluate_fx_param(fx_id, id.0, beat)
                    && let Some(spec) = self.descriptor.controls.iter().find(|c| c.id == id.0)
                {
                    spec.from_fraction(norm)
                } else {
                    f32::from_bits(value.load(Ordering::Acquire))
                };
                let _ = processor.set_parameter(*id, effective);
            }
            processor.process(frames);
        });
        self.busy.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::effects::{Plan, Rack, Routing};
    use elevate::{Primitive, primitives::filter::Filter};
    #[test]
    fn routing_prepares_real_rate_preserves_controls_and_does_not_reset_on_republish() {
        let descriptor = Arc::new(
            elevate::scripting::audio_components::parse(include_str!(
                "../../../assets/scripts/components/audio/filter.lua"
            ))
            .unwrap(),
        );
        let effect = ComponentEffect::prepare(descriptor, 48000).unwrap();
        effect.set(0, 300.).unwrap();
        let id = elevate::musical::fx::FxId(1);
        let rack = Rack::new(Plan::Component(id, effect.clone()), 0);
        let routing = Routing::new();
        routing.set(Some(rack.clone()), 96000);

        let mut native = Filter::new(ProcessSpec::new(96000.).unwrap());
        native.set_parameter(ParameterId(0), 300.).unwrap();
        native.reset();
        let mut actual: Vec<_> = (0..8192)
            .map(|i| [(i as f32 * 0.2).sin() * 0.1, 0.])
            .collect();
        let mut expected = actual.clone();
        native.process(&mut expected);
        routing.process(&mut actual);
        assert_eq!(effect.sample_rate(), 96000);
        assert_eq!(actual, expected);
        let cancelled = Routing::new();
        cancelled.set(Some(rack.clone()), 44100); // preparación cancelada de otra salida
        effect.set(0, 800.).unwrap();
        native.set_parameter(ParameterId(0), 800.).unwrap();
        rack.publish(Plan::Component(id, effect.clone()));
        routing.set(Some(rack.clone()), 96000);
        actual.fill([0.1, 0.]);
        expected.clone_from(&actual);
        native.process(&mut expected);
        routing.process(&mut actual);
        assert_eq!(actual, expected);
        // Insertar otro FX debe prepararlo para la salida activa, aunque un worker
        // cancelado haya sido el último en registrar una frecuencia en este rack.
        let fresh = ComponentEffect::prepare(effect.descriptor().clone(), 48000).unwrap();
        fresh.set(0, 800.).unwrap();
        rack.publish(Plan::Component(id, fresh.clone()));
        native.reset();
        actual.fill([0.1, 0.]);
        expected.clone_from(&actual);
        native.process(&mut expected);
        routing.process(&mut actual);
        assert_eq!(fresh.sample_rate(), 96000);
        assert_eq!(actual, expected);
    }
}
