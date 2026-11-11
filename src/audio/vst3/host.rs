//! Contexto mínimo de host VST3. No anuncia interfaces que no implementa.
#![allow(non_snake_case)]
use vst3::{
    Class,
    Steinberg::{Vst::*, *},
};
pub(super) struct Host;
impl Class for Host {
    type Interfaces = (IHostApplication,);
}
impl IHostApplicationTrait for Host {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        let mut buffer = [0; 128];
        for (out, code) in buffer.iter_mut().zip("Hay DAW".encode_utf16()) {
            *out = code;
        }
        unsafe {
            *name = buffer;
        }
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        _: *mut TUID,
        _: *mut TUID,
        object: *mut *mut std::ffi::c_void,
    ) -> tresult {
        if !object.is_null() {
            unsafe {
                *object = std::ptr::null_mut();
            }
        }
        kNoInterface
    }
}
