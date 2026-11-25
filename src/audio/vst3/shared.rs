//! Una instancia VST3 cambia entre audición y Arrangement sin duplicar su estado.
use super::{EditorHandle, TransportContext, VstEngine};
use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Route {
    Preview = 0,
    Arrangement = 1,
}

pub(crate) struct SharedInstrument {
    engine: UnsafeCell<VstEngine>,
    busy: AtomicBool,
    route: AtomicU8,
    reset: AtomicBool,
    failed: AtomicBool,
    live_tx: std::sync::Mutex<rtrb::Producer<(u8, u8, u8)>>,
    live_rx: UnsafeCell<rtrb::Consumer<(u8, u8, u8)>>,
    pub rate: u32,
    pub tail_frames: u64,
    pub editor: EditorHandle,
}

// Cada acceso al procesador obtiene busy mediante CAS; los dos callbacks nunca
// ejecutan el mismo objeto COM a la vez. El editor vive en su hilo propietario.
unsafe impl Send for SharedInstrument {}
unsafe impl Sync for SharedInstrument {}

impl SharedInstrument {
    pub fn new(engine: VstEngine, rate: u32) -> Arc<Self> {
        let (live_tx, live_rx) = rtrb::RingBuffer::new(512);
        let tail_frames = engine.tail_frames(rate);
        Arc::new(Self {
            editor: engine.editor.clone(),
            engine: UnsafeCell::new(engine),
            busy: AtomicBool::new(false),
            route: AtomicU8::new(Route::Preview as u8),
            reset: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            live_tx: std::sync::Mutex::new(live_tx),
            live_rx: UnsafeCell::new(live_rx),
            rate,
            tail_frames,
        })
    }
    pub fn in_arrangement(&self) -> bool {
        self.route.load(Ordering::Acquire) == Route::Arrangement as u8
    }
    pub fn enter_arrangement(&self) -> Result<(), String> {
        self.route
            .compare_exchange(
                Route::Preview as u8,
                Route::Arrangement as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| "Instancia VST3 ocupada por otra reproducción".to_string())?;
        self.reset.store(true, Ordering::Release);
        Ok(())
    }
    pub fn leave_arrangement(&self) {
        self.reset.store(true, Ordering::Release);
        self.route.store(Route::Preview as u8, Ordering::Release);
    }
    pub fn queue_live_midi(&self, status: u8, a: u8, b: u8) -> bool {
        self.live_tx
            .try_lock()
            .is_ok_and(|mut queue| queue.push((status, a, b)).is_ok())
    }
    fn with_engine(&self, route: Route, operation: impl FnOnce(&mut VstEngine)) {
        if self.route.load(Ordering::Acquire) != route as u8 {
            return;
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        if self.route.load(Ordering::Acquire) == route as u8 {
            let engine = unsafe { &mut *self.engine.get() };
            if self.reset.swap(false, Ordering::AcqRel) {
                engine.panic();
            }
            let live = unsafe { &mut *self.live_rx.get() };
            for _ in 0..512 {
                let Ok((status, a, b)) = live.pop() else {
                    break;
                };
                if route == Route::Arrangement {
                    engine.midi(status, a, b);
                }
            }
            operation(engine);
            if engine.failed {
                self.failed.store(true, Ordering::Release);
            }
        }
        self.busy.store(false, Ordering::Release);
    }
    pub fn midi(&self, route: Route, status: u8, a: u8, b: u8) {
        self.with_engine(route, |engine| engine.midi(status, a, b));
    }
    pub fn panic(&self, route: Route) {
        self.with_engine(route, VstEngine::panic);
    }
    pub fn render(
        &self,
        route: Route,
        left: &mut [f32],
        right: &mut [f32],
        time: Option<TransportContext>,
    ) {
        left.fill(0.);
        right.fill(0.);
        self.with_engine(route, |engine| engine.render_timed(left, right, time));
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}

pub(crate) struct InstrumentVoice {
    pub shared: Arc<SharedInstrument>,
    pub route: Route,
}
impl InstrumentVoice {
    pub fn preview(shared: Arc<SharedInstrument>) -> Self {
        Self {
            shared,
            route: Route::Preview,
        }
    }
    pub fn arrangement(shared: Arc<SharedInstrument>) -> Self {
        Self {
            shared,
            route: Route::Arrangement,
        }
    }
    pub fn midi(&self, status: u8, a: u8, b: u8) {
        self.shared.midi(self.route, status, a, b);
    }
    pub fn note_pitch_event(
        &self,
        event: elevate::musical::scheduler::ScheduledEvent,
        reserved: u16,
    ) -> bool {
        let mut handled = false;
        self.shared.with_engine(self.route, |engine| {
            handled = engine.note_pitch_event(event, reserved);
        });
        handled
    }
    pub fn panic(&self) {
        self.shared.panic(self.route);
    }
    pub fn render(&self, left: &mut [f32], right: &mut [f32], time: Option<TransportContext>) {
        self.shared.render(self.route, left, right, time);
    }
    pub fn failed(&self) -> bool {
        self.shared.failed()
    }
}
