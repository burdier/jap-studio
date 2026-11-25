//! Contrato estándar editor → procesador; no define controles ni mapeos específicos.
#![allow(non_snake_case)]
use std::sync::{
    Mutex,
    atomic::{AtomicI32, AtomicU64, Ordering},
};
use vst3::{
    Class,
    Steinberg::{Vst::*, *},
};
pub(crate) type Edit = (u64, ParamID, f64);
static GLOBAL_TOUCH_SEQ: AtomicU64 = AtomicU64::new(1);
pub(super) struct EditQueue {
    sender: rtrb::Producer<Edit>,
    sequence: u64,
    latest: std::collections::BTreeMap<ParamID, (u64, f64)>,
    last_touched: Option<(u64, ParamID, f64)>,
}
impl Handler {
    pub(crate) fn new(sender: rtrb::Producer<Edit>) -> Self {
        Self {
            edits: Mutex::new(EditQueue {
                sender,
                sequence: 0,
                latest: Default::default(),
                last_touched: None,
            }),
            restart: AtomicI32::new(0),
            applied: Default::default(),
        }
    }
    pub(super) fn revision(&self) -> u64 {
        self.edits.lock().map_or(0, |q| q.sequence)
    }
    pub(super) fn last_touched(&self) -> Option<(u64, ParamID, f64)> {
        self.edits.lock().ok().and_then(|q| q.last_touched)
    }
    /// Envía un cambio originado en el host (MIDI Learn, automatización o resync) al procesador RT
    /// sin sobrescribir el último parámetro tocado manualmente en la ventana del plugin.
    pub(super) fn push_host_edit(&self, id: ParamID, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let ok = self.edits.lock().is_ok_and(|mut queue| {
            let seq = queue.sequence + 1;
            let value = value.clamp(0., 1.);
            if queue.sender.push((seq, id, value)).is_err() {
                return false;
            }
            queue.sequence = seq;
            queue.latest.insert(id, (seq, value));
            true
        });
        if !ok {
            self.restart.fetch_or(4, Ordering::Relaxed);
        }
        ok
    }
    pub(super) fn pending(&self, applied: u64) -> Result<Vec<(ParamID, f64)>, String> {
        let queue = self.edits.lock().map_err(|e| e.to_string())?;
        let mut edits: Vec<_> = queue
            .latest
            .iter()
            .filter(|(_, (seq, _))| *seq > applied)
            .map(|(&id, &(seq, value))| (seq, id, value))
            .collect();
        edits.sort_unstable_by_key(|e| e.0);
        Ok(edits
            .into_iter()
            .map(|(_, id, value)| (id, value))
            .collect())
    }
}
pub(crate) struct Handler {
    edits: Mutex<EditQueue>,
    pub applied: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub restart: AtomicI32,
}
impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}
impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn performEdit(&self, id: ParamID, value: f64) -> tresult {
        if !value.is_finite() {
            return kInvalidArgument;
        }
        if self.edits.lock().is_ok_and(|mut queue| {
            let seq = queue.sequence + 1;
            let value = value.clamp(0., 1.);
            if queue.sender.push((seq, id, value)).is_err() {
                return false;
            }
            queue.sequence = seq;
            queue.latest.insert(id, (seq, value));
            let touch_seq = GLOBAL_TOUCH_SEQ.fetch_add(1, Ordering::Relaxed);
            queue.last_touched = Some((touch_seq, id, value));
            true
        }) {
            kResultOk
        } else {
            self.restart.fetch_or(4, Ordering::Relaxed);
            kResultFalse
        }
    }
    unsafe fn endEdit(&self, _: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn restartComponent(&self, flags: i32) -> tresult {
        self.restart.fetch_or(flags, Ordering::Relaxed);
        kResultOk
    }
}
