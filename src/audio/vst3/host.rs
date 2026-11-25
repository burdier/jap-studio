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
        for (out, code) in buffer.iter_mut().zip("Jap Studio".encode_utf16()) {
            *out = code;
        }
        unsafe {
            *name = buffer;
        }
        kResultOk
    }
    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        object: *mut *mut std::ffi::c_void,
    ) -> tresult {
        if object.is_null() || cid.is_null() || iid.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            *object = std::ptr::null_mut();
            if *cid != *iid {
                return kNoInterface;
            }
            *object = if *cid == IMessage_iid {
                vst3::ComWrapper::new(super::message::Message::new())
                    .to_com_ptr::<IMessage>()
                    .unwrap()
                    .into_raw()
                    .cast()
            } else if *cid == IAttributeList_iid {
                vst3::ComWrapper::new(super::message::Attributes::default())
                    .to_com_ptr::<IAttributeList>()
                    .unwrap()
                    .into_raw()
                    .cast()
            } else {
                return kNoInterface;
            };
        }
        kResultOk
    }
}
