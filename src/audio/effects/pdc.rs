//! Compensación entera en muestras. Cada ruta tiene su propia memoria de audio.
use super::Plan;
use elevate::musical::fx::FxId;
use std::{
    cell::UnsafeCell,
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) struct Ring {
    frames: Box<[[f32; 2]]>,
    next: usize,
}

impl Ring {
    pub(crate) fn new(length: usize) -> Self {
        Self {
            frames: vec![[0.; 2]; length].into_boxed_slice(),
            next: 0,
        }
    }
    pub(crate) fn process(&mut self, mut frames: &mut [[f32; 2]]) {
        if self.frames.is_empty() {
            return;
        }
        while !frames.is_empty() {
            let count = frames.len().min(self.frames.len() - self.next);
            let (block, rest) = frames.split_at_mut(count);
            block.swap_with_slice(&mut self.frames[self.next..self.next + count]);
            self.next += count;
            if self.next == self.frames.len() {
                self.next = 0;
            }
            frames = rest;
        }
    }
}

pub(crate) struct DelayLine {
    length: usize,
    busy: AtomicBool,
    ring: UnsafeCell<Ring>,
}
// El publicador solo lee length. Únicamente el callback que adquiere busy toca ring.
unsafe impl Sync for DelayLine {}

impl DelayLine {
    pub(crate) fn new(length: usize) -> Self {
        Self {
            length,
            busy: AtomicBool::new(false),
            ring: UnsafeCell::new(Ring::new(length)),
        }
    }

    pub(crate) fn process(&self, frames: &mut [[f32; 2]]) {
        if self
            .busy
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            frames.fill([0.; 2]);
            return;
        }
        let ring = unsafe { &mut *self.ring.get() };
        ring.process(frames);
        self.busy.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct DelayBank(BTreeMap<FxId, Arc<DelayLine>>);

impl DelayBank {
    pub(super) fn prepare(plan: &Plan, previous: Option<&Self>) -> Self {
        let mut next = Self::default();
        plan.visit_delays(&mut |id, length| {
            let line = previous
                .and_then(|bank| bank.0.get(&id))
                .filter(|line| line.length == length)
                .cloned()
                .unwrap_or_else(|| Arc::new(DelayLine::new(length)));
            next.0.insert(id, line);
        });
        next
    }

    pub(super) fn process(&self, id: FxId, frames: &mut [[f32; 2]]) {
        self.0[&id].process(frames);
    }
}
