//! Plan FX inmutable publicado fuera de RT; instancias independientes por FxId.
mod component;
mod convolver;
mod crossover;
pub(super) use crossover::CrossoverHistory;
mod pdc;
pub(crate) use pdc::DelayLine as CompensationDelay;
mod plan;

use super::publication::Publication;
use super::vst3::{EditorHandle, TransportContext, VstEngine};
pub(crate) use component::ComponentEffect;
pub(crate) use plan::{Plan, compile, contains_vst, needs_rack};
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
    pub tail_frames: u64,
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
    pub fn restore_project(
        path: std::path::PathBuf,
        class: String,
        preset: elevate::musical::fx::VstPreset,
        config: &super::device::AudioConfig,
    ) -> Result<Arc<Self>, String> {
        let device = config.device(false)?;
        let rate = super::output::choose_output_config(&device, config)?
            .config
            .sample_rate;
        let state = super::vst3::PluginState::from_effect_preset(path, class, &preset)?;
        let effect = Self::from_engine(state.instantiate(rate)?, rate);
        effect.editor.watch_state(Some(preset))?;
        Ok(effect)
    }
    pub fn load(path: &std::path::Path, class: &str, rate: u32) -> Result<Arc<Self>, String> {
        let engine = VstEngine::load_effect(path, rate, class)?;
        Ok(Self::from_engine(engine, rate))
    }
    pub fn from_engine(engine: VstEngine, rate: u32) -> Arc<Self> {
        Arc::new(Self {
            tail_frames: engine.effect_tail_frames(rate),
            name: engine.name.clone(),
            editor: engine.editor.clone(),
            engine: UnsafeCell::new(engine),
            busy: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            rate,
        })
    }
    pub fn process(&self, frames: &mut [[f32; 2]]) {
        self.process_timed(frames, None);
    }
    pub fn process_timed(&self, frames: &mut [[f32; 2]], time: Option<TransportContext>) {
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
        if time.is_some() {
            engine.process_effect_timed(frames, time);
        } else {
            engine.process_effect(frames);
        }
        if engine.failed {
            self.failed.store(true, Ordering::Relaxed);
        }
        self.busy.store(false, Ordering::Release);
    }
}
// El rack publica el plan y la memoria de retardo correspondiente como una sola versión
// por stream. Recompilar un fader conserva las muestras pendientes de esa rama.
struct StreamPlan {
    plan: Arc<Plan>,
    delays: convolver::StreamDsp,
}
struct RackStream {
    rack: Arc<Rack>,
    rate: u32,
    current: Publication<StreamPlan>,
}
pub(crate) struct Rack {
    streams: std::sync::Mutex<Vec<std::sync::Weak<RackStream>>>,
    plan: Publication<Arc<Plan>>,
    pub rate: u32,
}
impl Rack {
    pub fn new(plan: Plan, rate: u32) -> Arc<Self> {
        Arc::new(Self {
            streams: std::sync::Mutex::new(Vec::new()),
            plan: Publication::new(Arc::new(plan)),
            rate,
        })
    }
    pub fn latency_samples(&self) -> u64 {
        self.plan.read(|plan| plan.latency_samples())
    }
    pub fn tail_frames(&self) -> u64 {
        self.plan.read(|plan| plan.tail_frames())
    }
    pub fn failed(&self) -> bool {
        self.plan.read(|plan| plan.failed())
    }
    pub fn set_band_monitor(
        &self,
        chain: &elevate::musical::fx::FxChain,
        solo: Option<elevate::musical::fx::FxId>,
        draft: Option<(elevate::musical::fx::FxId, u16)>,
    ) {
        self.plan
            .read(|plan| plan.set_band_monitor(chain, solo, draft));
    }
    pub fn publish(&self, plan: Plan) {
        let mut streams = self.streams.lock().unwrap();
        let plan = Arc::new(plan);
        streams.retain(|weak| {
            let Some(stream) = weak.upgrade() else {
                return false;
            };
            plan.prepare_rate(stream.rate);
            let delays = stream
                .current
                .read(|old| convolver::StreamDsp::prepare(&plan, Some(&old.delays), stream.rate));
            stream.current.publish(StreamPlan {
                plan: plan.clone(),
                delays,
            });
            true
        });
        self.plan.publish(plan);
    }
    fn subscribe(self: &Arc<Self>, rate: u32) -> Arc<RackStream> {
        let mut streams = self.streams.lock().unwrap();
        let plan = self.plan.read(Arc::clone);
        plan.prepare_rate(rate);
        let delays = convolver::StreamDsp::prepare(&plan, None, rate);
        let stream = Arc::new(RackStream {
            rack: self.clone(),
            rate,
            current: Publication::new(StreamPlan { plan, delays }),
        });
        streams.retain(|weak| weak.strong_count() > 0);
        streams.push(Arc::downgrade(&stream));
        stream
    }
}
/// Cada fuente recibe su ruta; nunca libera planes ni plugins durante process.
pub(crate) struct Routing(Publication<Option<Arc<RackStream>>>);
impl Routing {
    pub fn new() -> Self {
        Self(Publication::new(None))
    }
    pub fn tail_frames(&self) -> u64 {
        self.0.read(|stream| {
            stream
                .as_ref()
                .map_or(0, |stream| stream.rack.tail_frames())
        })
    }
    pub fn set(&self, rack: Option<Arc<Rack>>, rate: u32) {
        let same = self.0.read(|old| match (old, &rack) {
            (None, None) => true,
            (Some(a), Some(b)) => a.rate == rate && Arc::ptr_eq(&a.rack, b),
            _ => false,
        });
        if !same {
            self.0.publish(rack.map(|rack| rack.subscribe(rate)));
        }
    }
    #[cfg(test)]
    pub fn process(&self, frames: &mut [[f32; 2]]) {
        self.process_timed(frames, None, &mut CrossoverHistory::new());
    }
    pub fn process_timed(
        &self,
        frames: &mut [[f32; 2]],
        time: Option<TransportContext>,
        history: &mut CrossoverHistory,
    ) {
        self.0.read(|stream| {
            if let Some(stream) = stream {
                if stream.rack.rate == 0 || stream.rack.rate == stream.rate {
                    stream.current.read(|current| {
                        current.plan.process_timed(
                            frames,
                            stream.rate,
                            time,
                            history,
                            &current.delays,
                        )
                    });
                } else {
                    frames.fill([0.; 2]);
                }
            }
        });
    }
}
