//! Cambios de parámetros a offset cero por segmento musical; sin asignaciones RT.
#![allow(non_snake_case)]
use std::{cell::Cell, ptr};
use vst3::{
    Class, ComPtr, ComWrapper,
    Steinberg::{Vst::*, *},
};
pub(super) struct Point {
    id: Cell<ParamID>,
    value: Cell<f64>,
}
impl Class for Point {
    type Interfaces = (IParamValueQueue,);
}
impl IParamValueQueueTrait for Point {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id.get()
    }
    unsafe fn getPointCount(&self) -> i32 {
        1
    }
    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut f64) -> tresult {
        if index != 0 || offset.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        unsafe {
            *offset = 0;
            *value = self.value.get();
        }
        kResultOk
    }
    unsafe fn addPoint(&self, _: i32, value: f64, index: *mut i32) -> tresult {
        if !index.is_null() {
            unsafe {
                *index = 0;
            }
        }
        self.value.set(value);
        kResultOk
    }
}
pub(super) struct Changes {
    points: Vec<(ComWrapper<Point>, ComPtr<IParamValueQueue>)>,
    count: Cell<usize>,
}
impl Changes {
    pub fn new(capacity: usize) -> Self {
        Self {
            points: (0..capacity)
                .map(|_| {
                    let point = ComWrapper::new(Point {
                        id: Cell::new(0),
                        value: Cell::new(0.),
                    });
                    let interface = point.to_com_ptr().unwrap();
                    (point, interface)
                })
                .collect(),
            count: Cell::new(0),
        }
    }
    pub fn send_latest(&self, queue: &mut rtrb::Producer<(u32, f64)>) {
        for (point, _) in &self.points[..self.count.get()] {
            let _ = queue.push((point.id.get(), point.value.get()));
        }
    }
    pub fn clear(&self) {
        self.count.set(0);
    }
    pub fn set(&self, id: ParamID, value: f64) {
        let mut index = 0;
        let queue = unsafe { self.addParameterData(&id, &mut index) };
        if !queue.is_null() {
            self.points[index as usize].0.value.set(value.clamp(0., 1.));
        }
    }
}
impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}
impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        self.count.get() as i32
    }
    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        if index < 0 || index as usize >= self.count.get() {
            return ptr::null_mut();
        }
        self.points[index as usize].1.as_ptr()
    }
    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut i32,
    ) -> *mut IParamValueQueue {
        if id.is_null() || index.is_null() {
            return ptr::null_mut();
        }
        let id = unsafe { *id };
        let found = self.points[..self.count.get()]
            .iter()
            .position(|(point, _)| point.id.get() == id);
        let slot = found.unwrap_or(self.count.get());
        if slot >= self.points.len() {
            return ptr::null_mut();
        }
        if found.is_none() {
            self.count.set(slot + 1);
            self.points[slot].0.id.set(id);
        }
        unsafe {
            *index = slot as i32;
        }
        self.points[slot].1.as_ptr()
    }
}
