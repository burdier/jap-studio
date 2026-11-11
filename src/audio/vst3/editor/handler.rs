//! Contrato estándar editor → procesador; no define controles ni mapeos específicos.
#![allow(non_snake_case)]
use std::sync::{
    Mutex,
    atomic::{AtomicI32, Ordering},
};
use vst3::{
    Class,
    Steinberg::{Vst::*, *},
};
pub(crate) type Edit = (ParamID, f64);
pub(crate) struct Handler {
    pub edits: Mutex<rtrb::Producer<Edit>>,
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
        if self
            .edits
            .lock()
            .is_ok_and(|mut queue| queue.push((id, value.clamp(0., 1.))).is_ok())
        {
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
