//! IBStream temporal para sincronizar el estado inicial del controlador estándar.
#![allow(non_snake_case)]
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
};
use vst3::{Class, Steinberg::*};
const LIMIT: usize = 64 * 1024 * 1024;
#[derive(Default)]
pub(super) struct Stream {
    bytes: RefCell<Vec<u8>>,
    position: Cell<usize>,
}
impl Class for Stream {
    type Interfaces = (IBStream,);
}
impl IBStreamTrait for Stream {
    unsafe fn read(&self, buffer: *mut c_void, count: i32, read: *mut i32) -> tresult {
        if !read.is_null() {
            unsafe {
                *read = 0;
            }
        }
        if count < 0 || (count > 0 && buffer.is_null()) {
            return kInvalidArgument;
        }
        let bytes = self.bytes.borrow();
        let len = (count as usize).min(bytes.len().saturating_sub(self.position.get()));
        if len > 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr().add(self.position.get()),
                    buffer.cast(),
                    len,
                );
            }
        }
        self.position.set(self.position.get() + len);
        if !read.is_null() {
            unsafe {
                *read = len as i32;
            }
        }
        if len == count as usize {
            kResultOk
        } else {
            kResultFalse
        }
    }
    unsafe fn write(&self, buffer: *mut c_void, count: i32, written: *mut i32) -> tresult {
        if !written.is_null() {
            unsafe {
                *written = 0;
            }
        }
        if count < 0 || (count > 0 && buffer.is_null()) {
            return kInvalidArgument;
        }
        let Some(end) = self
            .position
            .get()
            .checked_add(count as usize)
            .filter(|end| *end <= LIMIT)
        else {
            return kOutOfMemory;
        };
        let mut bytes = self.bytes.borrow_mut();
        let length = end.max(bytes.len());
        bytes.resize(length, 0);
        if count > 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    buffer.cast::<u8>(),
                    bytes.as_mut_ptr().add(self.position.get()),
                    count as usize,
                );
            }
        }
        self.position.set(end);
        if !written.is_null() {
            unsafe {
                *written = count;
            }
        }
        kResultOk
    }
    unsafe fn seek(&self, offset: i64, mode: i32, result: *mut i64) -> tresult {
        let base = match mode {
            0 => 0,
            1 => self.position.get() as i64,
            2 => self.bytes.borrow().len() as i64,
            _ => return kInvalidArgument,
        };
        let Some(position) = base
            .checked_add(offset)
            .filter(|p| *p >= 0 && *p <= LIMIT as i64)
        else {
            return kInvalidArgument;
        };
        self.position.set(position as usize);
        if !result.is_null() {
            unsafe {
                *result = position;
            }
        }
        kResultOk
    }
    unsafe fn tell(&self, result: *mut i64) -> tresult {
        if result.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            *result = self.position.get() as i64;
        }
        kResultOk
    }
}
