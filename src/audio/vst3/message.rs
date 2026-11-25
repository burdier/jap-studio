//! Objetos estándar de mensajes VST3 para el intercambio entre controlador y editor.
#![allow(non_snake_case)]
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_void},
    ptr,
    sync::Mutex,
};
use vst3::Steinberg::Vst::IAttributeList_::AttrID;
use vst3::{
    Class, ComPtr, ComWrapper,
    Steinberg::{Vst::*, *},
};

pub(super) struct Message {
    id: Mutex<CString>,
    attributes: ComPtr<IAttributeList>,
}
impl Message {
    pub fn new() -> Self {
        Self {
            id: Mutex::default(),
            attributes: ComWrapper::new(Attributes::default()).to_com_ptr().unwrap(),
        }
    }
}
impl Class for Message {
    type Interfaces = (IMessage,);
}
impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        self.id.lock().unwrap().as_ptr()
    }
    unsafe fn setMessageID(&self, id: FIDString) {
        if !id.is_null() {
            *self.id.lock().unwrap() = unsafe { CStr::from_ptr(id) }.to_owned();
        }
    }
    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_ptr()
    }
}
enum Value {
    Int(i64),
    Float(f64),
    String(Vec<u16>),
    Binary(Vec<u8>),
}
#[derive(Default)]
pub(super) struct Attributes(Mutex<HashMap<Vec<u8>, Value>>);
impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}
impl Attributes {
    fn set(&self, id: AttrID, value: Value) -> tresult {
        if id.is_null() {
            return kInvalidArgument;
        }
        self.0
            .lock()
            .unwrap()
            .insert(unsafe { CStr::from_ptr(id) }.to_bytes().into(), value);
        kResultOk
    }
    fn read(&self, id: AttrID, f: impl FnOnce(&Value) -> bool) -> tresult {
        if id.is_null() {
            return kInvalidArgument;
        }
        if self
            .0
            .lock()
            .unwrap()
            .get(unsafe { CStr::from_ptr(id) }.to_bytes())
            .is_some_and(f)
        {
            kResultOk
        } else {
            kResultFalse
        }
    }
}
impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, id: AttrID, v: i64) -> tresult {
        self.set(id, Value::Int(v))
    }
    unsafe fn getInt(&self, id: AttrID, out: *mut i64) -> tresult {
        if out.is_null() {
            return kInvalidArgument;
        }
        self.read(id, |v| {
            if let Value::Int(x) = v {
                unsafe { *out = *x };
                true
            } else {
                false
            }
        })
    }
    unsafe fn setFloat(&self, id: AttrID, v: f64) -> tresult {
        self.set(id, Value::Float(v))
    }
    unsafe fn getFloat(&self, id: AttrID, out: *mut f64) -> tresult {
        if out.is_null() {
            return kInvalidArgument;
        }
        self.read(id, |v| {
            if let Value::Float(x) = v {
                unsafe { *out = *x };
                true
            } else {
                false
            }
        })
    }
    unsafe fn setString(&self, id: AttrID, s: *const u16) -> tresult {
        if s.is_null() {
            return kInvalidArgument;
        }
        for n in 0..(1 << 20) {
            if unsafe { *s.add(n) } == 0 {
                return self.set(
                    id,
                    Value::String(unsafe { std::slice::from_raw_parts(s, n) }.to_vec()),
                );
            }
        }
        kInvalidArgument
    }
    unsafe fn getString(&self, id: AttrID, out: *mut u16, bytes: u32) -> tresult {
        if out.is_null() || bytes < 2 {
            return kInvalidArgument;
        }
        self.read(id, |v| {
            if let Value::String(x) = v {
                let n = x.len().min(bytes as usize / 2 - 1);
                unsafe {
                    ptr::copy_nonoverlapping(x.as_ptr(), out, n);
                    *out.add(n) = 0;
                }
                true
            } else {
                false
            }
        })
    }
    unsafe fn setBinary(&self, id: AttrID, data: *const c_void, bytes: u32) -> tresult {
        if bytes == 0 {
            return self.set(id, Value::Binary(Vec::new()));
        }
        if data.is_null() || bytes > (64 << 20) {
            return kInvalidArgument;
        }
        self.set(
            id,
            Value::Binary(
                unsafe { std::slice::from_raw_parts(data.cast::<u8>(), bytes as usize) }.to_vec(),
            ),
        )
    }
    unsafe fn getBinary(&self, id: AttrID, out: *mut *const c_void, bytes: *mut u32) -> tresult {
        if out.is_null() || bytes.is_null() {
            return kInvalidArgument;
        }
        self.read(id, |v| {
            if let Value::Binary(x) = v {
                unsafe {
                    *out = x.as_ptr().cast();
                    *bytes = x.len() as u32;
                }
                true
            } else {
                false
            }
        })
    }
}
