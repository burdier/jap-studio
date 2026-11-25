//! Pool de hilos dedicado para audio RT: pre-creado en `Mix::prepare`, cero allocaciones
//! ni mutex en el camino caliente, barrera estricta por worker (`wake_epoch`/`done_epoch`)
//! y apagado limpio en `Drop`.
use std::{
    hint,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
};

const MAX_WORKERS: usize = 6;
const WORKER_SPIN_ITERS: usize = 64;

struct JobSlot {
    trampoline: fn(*const (), usize),
    data: *const (),
}

#[repr(align(64))]
struct WorkerGate {
    wake_epoch: AtomicU64,
    done_epoch: AtomicU64,
}

impl WorkerGate {
    fn new() -> Self {
        Self {
            wake_epoch: AtomicU64::new(0),
            done_epoch: AtomicU64::new(0),
        }
    }
}

struct PoolShared {
    job: AtomicPtr<JobSlot>,
    task_count: AtomicUsize,
    next_task: AtomicUsize,
    shutdown: AtomicBool,
    gates: Vec<WorkerGate>,
}

unsafe impl Send for PoolShared {}
unsafe impl Sync for PoolShared {}

impl PoolShared {
    fn run_tasks(&self) {
        let count = self.task_count.load(Ordering::Relaxed);
        let slot_ptr = self.job.load(Ordering::Acquire);
        if slot_ptr.is_null() {
            return;
        }
        // SAFETY: `execute` mantiene `JobSlot` vivo hasta que todos los workers despiertos
        // publican `done_epoch == next_epoch`.
        let slot = unsafe { &*slot_ptr };
        loop {
            let idx = self.next_task.fetch_add(1, Ordering::Relaxed);
            if idx >= count {
                break;
            }
            (slot.trampoline)(slot.data, idx);
        }
    }
}

pub(super) struct AudioPool {
    shared: Arc<PoolShared>,
    workers: Vec<JoinHandle<()>>,
    epoch: AtomicU64,
}

impl AudioPool {
    pub fn new(track_count: usize) -> Option<Self> {
        if track_count < 2 {
            return None;
        }
        let cores = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let worker_count = cores
            .saturating_sub(1)
            .min(track_count - 1)
            .min(MAX_WORKERS);
        if worker_count == 0 {
            return None;
        }
        let gates = (0..worker_count).map(|_| WorkerGate::new()).collect();
        let shared = Arc::new(PoolShared {
            job: AtomicPtr::new(std::ptr::null_mut()),
            task_count: AtomicUsize::new(0),
            next_task: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            gates,
        });
        let mut workers = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let shared = shared.clone();
            let handle = thread::Builder::new()
                .name(format!("hay-audio-worker-{index}"))
                .spawn(move || worker_loop(&shared, index))
                .ok()?;
            workers.push(handle);
        }
        Some(Self {
            shared,
            workers,
            epoch: AtomicU64::new(0),
        })
    }

    /// Ejecuta `f(0..count)` en paralelo entre los workers y el hilo llamador.
    /// Cero allocaciones, sin mutex; regresa solo cuando todos los workers despiertos
    /// terminaron y salieron completamente de `run_tasks()`.
    pub fn execute<F>(&self, count: usize, f: F)
    where
        F: Fn(usize) + Sync,
    {
        if count <= 1 {
            if count == 1 {
                f(0);
            }
            return;
        }
        fn trampoline<F: Fn(usize) + Sync>(data: *const (), idx: usize) {
            // SAFETY: puntero válido durante toda la llamada a `execute`.
            let f = unsafe { &*(data as *const F) };
            f(idx);
        }
        let mut slot = JobSlot {
            trampoline: trampoline::<F>,
            data: (&f as *const F).cast(),
        };
        let next_epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        let wake = self.workers.len().min(count - 1);

        self.shared.task_count.store(count, Ordering::Relaxed);
        self.shared.next_task.store(0, Ordering::Relaxed);
        self.shared
            .job
            .store(&mut slot as *mut JobSlot, Ordering::Release);

        for i in 0..wake {
            self.shared.gates[i]
                .wake_epoch
                .store(next_epoch, Ordering::Release);
            self.workers[i].thread().unpark();
        }

        // El hilo RT también roba tareas para no quedar ocioso.
        self.shared.run_tasks();

        // Barrera estricta: esperar a que cada worker despierto confirme haber salido de `run_tasks`.
        for i in 0..wake {
            while self.shared.gates[i].done_epoch.load(Ordering::Acquire) != next_epoch {
                hint::spin_loop();
            }
        }
        self.shared
            .job
            .store(std::ptr::null_mut(), Ordering::Release);
    }
}

impl Drop for AudioPool {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        let stop_epoch = self.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        for (i, worker) in self.workers.iter().enumerate() {
            self.shared.gates[i]
                .wake_epoch
                .store(stop_epoch, Ordering::Release);
            worker.thread().unpark();
        }
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

fn worker_loop(shared: &PoolShared, index: usize) {
    let gate = &shared.gates[index];
    let mut seen_epoch = 0u64;
    loop {
        let mut woken = gate.wake_epoch.load(Ordering::Acquire);
        if woken == seen_epoch {
            let mut spins = 0;
            while spins < WORKER_SPIN_ITERS {
                hint::spin_loop();
                woken = gate.wake_epoch.load(Ordering::Acquire);
                if woken != seen_epoch {
                    break;
                }
                spins += 1;
            }
            if woken == seen_epoch {
                if shared.shutdown.load(Ordering::Acquire) {
                    break;
                }
                thread::park();
                continue;
            }
        }
        seen_epoch = woken;
        if shared.shutdown.load(Ordering::Acquire) {
            break;
        }
        shared.run_tasks();
        gate.done_epoch.store(seen_epoch, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn audio_pool_executes_all_tasks_across_multiple_epochs() {
        let Some(pool) = AudioPool::new(8) else {
            return;
        };
        for epoch in 0..1000 {
            let count = (epoch % 16) + 1;
            let hits: Vec<AtomicUsize> = (0..count).map(|_| AtomicUsize::new(0)).collect();
            pool.execute(count, |idx| {
                hits[idx].fetch_add(1, Ordering::Relaxed);
            });
            for (idx, h) in hits.iter().enumerate() {
                assert_eq!(h.load(Ordering::Relaxed), 1, "epoch {epoch} idx {idx}");
            }
        }
    }
}
