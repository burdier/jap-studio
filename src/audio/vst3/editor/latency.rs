//! Exclusión entre process y los cambios de estado exigidos por kLatencyChanged.
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::time::Duration;

const RENDERING: u8 = 1;
const RESTARTING: u8 = 2;
const FAILED: u8 = 4;

#[derive(Default)]
pub(in super::super) struct ProcessorLatency {
    access: AtomicU8,
    samples: AtomicU32,
    changed: AtomicBool,
}
pub(in super::super) struct RenderGuard<'a>(&'a AtomicU8);
impl Drop for RenderGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_and(!RENDERING, Ordering::Release);
    }
}
impl ProcessorLatency {
    pub(super) fn set_initial(&self, samples: u32) {
        self.samples.store(samples, Ordering::Release);
    }
    pub(super) fn samples(&self) -> u32 {
        self.samples.load(Ordering::Acquire)
    }
    pub(super) fn take_change(&self) -> bool {
        self.changed.swap(false, Ordering::AcqRel)
    }
    // Play nunca espera; el worker offline sí puede esperar y verificar su plan fijo.
    pub(in super::super) fn enter(
        &self,
        export_latency: Option<u32>,
    ) -> Result<Option<RenderGuard<'_>>, ()> {
        loop {
            match self
                .access
                .compare_exchange(0, RENDERING, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => {
                    let guard = RenderGuard(&self.access);
                    if export_latency.is_some_and(|expected| expected != self.samples()) {
                        return Err(());
                    }
                    return Ok(Some(guard));
                }
                Err(FAILED) => return Err(()),
                Err(_) if export_latency.is_none() => return Ok(None),
                Err(_) => std::thread::sleep(Duration::from_micros(100)),
            }
        }
    }
    // Solo el hilo propietario entra aquí. Impide que empiece otro bloque y espera
    // fuera del callback a que termine el que ya estaba dentro del procesador.
    pub(super) fn restart(
        &self,
        activate: impl FnOnce() -> Result<u32, String>,
    ) -> Result<(), String> {
        if self.access.fetch_or(RESTARTING, Ordering::AcqRel) & FAILED != 0 {
            self.access.store(FAILED, Ordering::Release);
            return Err("El VST3 no pudo reactivar su procesador".into());
        }
        while self.access.load(Ordering::Acquire) & RENDERING != 0 {
            std::thread::sleep(Duration::from_micros(100));
        }
        match activate() {
            Ok(samples) => {
                if self.samples.swap(samples, Ordering::AcqRel) != samples {
                    self.changed.store(true, Ordering::Release);
                }
                self.access.store(0, Ordering::Release);
                Ok(())
            }
            Err(error) => {
                self.access.store(FAILED, Ordering::Release);
                Err(error)
            }
        }
    }
}
