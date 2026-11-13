//! Plan FX inmutable publicado fuera de RT; instancias independientes por FxId.
mod component;
mod plan;

use super::publication::Publication;
use super::vst3::{EditorHandle, VstEngine};
pub(crate) use component::ComponentEffect;
pub(crate) use plan::{Plan, compile, contains_component, contains_vst, needs_rack};
use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) struct Effect {
    engine: UnsafeCell<VstEngine>,
    busy: AtomicBool,
    pub failed: AtomicBool,
    pub editor: EditorHandle,
    pub rate: u32,
    pub name: String,
}
// Solo process accede al motor mediante exclusión CAS sin espera; el editor tiene
// su hilo propietario. UI jamás lee/escribe el procesador ni sus buffers.
unsafe impl Send for Effect {}
unsafe impl Sync for Effect {}
impl Effect {
    pub fn prepare(
        path: &std::path::Path,
        class: &str,
        config: &super::device::AudioConfig,
    ) -> Result<Arc<Self>, String> {
        let device = config.device(false)?;
        let setup = super::output::choose_output_config(&device, config)?;
        Self::load(path, class, setup.config.sample_rate)
    }
    pub fn load(path: &std::path::Path, class: &str, rate: u32) -> Result<Arc<Self>, String> {
        let engine = VstEngine::load_effect(path, rate, class)?;
        Ok(Arc::new(Self {
            name: engine.name.clone(),
            editor: engine.editor.clone(),
            engine: UnsafeCell::new(engine),
            busy: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            rate,
        }))
    }
    pub fn process(&self, frames: &mut [[f32; 2]]) {
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.failed.store(true, Ordering::Relaxed);
            frames.fill([0.; 2]);
            return;
        }
        let engine = unsafe { &mut *self.engine.get() };
        engine.process_effect(frames);
        if engine.failed {
            self.failed.store(true, Ordering::Relaxed);
        }
        self.busy.store(false, Ordering::Release);
    }
}
pub(crate) struct Rack {
    stream_rates: std::sync::Mutex<std::collections::BTreeSet<u32>>,
    plan: Publication<Plan>,
    pub rate: u32,
}
impl Rack {
    pub fn new(plan: Plan, rate: u32) -> Arc<Self> {
        Arc::new(Self {
            stream_rates: std::sync::Mutex::new(Default::default()),
            plan: Publication::new(plan),
            rate,
        })
    }
    pub fn publish(&self, plan: Plan) {
        let rates = self.stream_rates.lock().unwrap();
        for &rate in rates.iter() {
            plan.prepare_rate(rate);
        }
        self.plan.publish(plan);
    }
    fn prepare_rate(&self, rate: u32) {
        let mut rates = self.stream_rates.lock().unwrap();
        rates.insert(rate);
        self.plan.read(|plan| plan.prepare_rate(rate));
    }
    fn process(&self, frames: &mut [[f32; 2]], rate: u32) {
        self.plan.read(|plan| plan.process(frames, rate));
    }
}
/// Cada fuente recibe su ruta; nunca libera planes ni plugins durante process.
pub(crate) struct Routing(Publication<(Option<Arc<Rack>>, u32)>);
impl Routing {
    pub fn new() -> Self {
        Self(Publication::new((None, 0)))
    }
    pub fn set(&self, rack: Option<Arc<Rack>>, rate: u32) {
        let same = self.0.read(|(old, old_rate)| {
            *old_rate == rate
                && match (old, &rack) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    _ => false,
                }
        });
        if !same {
            if let Some(rack) = &rack {
                rack.prepare_rate(rate);
            }
            self.0.publish((rack, rate));
        }
    }
    pub fn process(&self, frames: &mut [[f32; 2]]) {
        self.0.read(|(rack, rate)| {
            if let Some(rack) = rack {
                // Cero = sin VST3: el analizador recibe la frecuencia real del stream.
                if rack.rate == 0 || rack.rate == *rate {
                    rack.process(frames, *rate);
                } else {
                    frames.fill([0.; 2]);
                }
            }
        });
    }
}
