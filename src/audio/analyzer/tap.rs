//! Copia acotada, sin espera, desde cualquier punto del plan FX. Nunca modifica audio.
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    cell::UnsafeCell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const BLOCK: usize = 256;
const QUEUE: usize = 64;
pub(crate) struct Samples {
    pub frames: [[f32; 2]; BLOCK],
    pub len: usize,
    pub rate: u32,
    pub epoch: u64,
}
pub(crate) struct Tap {
    producer: UnsafeCell<Producer<Samples>>,
    busy: AtomicBool,
    epoch: AtomicU64,
    active: AtomicBool,
    pub dropped: AtomicU64,
}
// El productor solo se toca dentro del CAS; el consumidor pertenece al hilo visual.
// Ante dos fuentes simultáneas se omite la captura, nunca se espera ni altera la señal.
unsafe impl Send for Tap {}
unsafe impl Sync for Tap {}
impl Tap {
    pub fn new() -> (Arc<Self>, Consumer<Samples>) {
        let (producer, consumer) = RingBuffer::new(QUEUE);
        (
            Arc::new(Self {
                producer: UnsafeCell::new(producer),
                busy: AtomicBool::new(false),
                epoch: AtomicU64::new(0),
                active: AtomicBool::new(false),
                dropped: AtomicU64::new(0),
            }),
            consumer,
        )
    }
    pub fn activate(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }
    pub fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
    }
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }
    pub fn capture(&self, frames: &[[f32; 2]], rate: u32) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let producer = unsafe { &mut *self.producer.get() };
        let epoch = self.epoch();
        for chunk in frames.chunks(BLOCK) {
            let mut samples = Samples {
                frames: [[0.; 2]; BLOCK],
                len: chunk.len(),
                rate,
                epoch,
            };
            samples.frames[..chunk.len()].copy_from_slice(chunk);
            if producer.push(samples).is_err() {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
        self.busy.store(false, Ordering::Release);
    }
}
