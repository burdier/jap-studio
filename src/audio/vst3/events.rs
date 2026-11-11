//! Lista COM de eventos, preasignada y usada exclusivamente por process().
#![allow(non_snake_case)]
use std::cell::UnsafeCell;
use vst3::{
    Class,
    Steinberg::{Vst::*, *},
};

pub(super) struct Events(UnsafeCell<Vec<Event>>);
impl Events {
    pub fn new() -> Self {
        Self(UnsafeCell::new(Vec::with_capacity(4096)))
    }
    pub fn clear(&self) {
        unsafe {
            (&mut *self.0.get()).clear();
        }
    }
    pub fn push(&self, event: Event) -> bool {
        let events = unsafe { &mut *self.0.get() };
        if events.len() == events.capacity() {
            return false;
        }
        events.push(event);
        true
    }
}
impl Class for Events {
    type Interfaces = (IEventList,);
}
impl IEventListTrait for Events {
    unsafe fn getEventCount(&self) -> i32 {
        unsafe { (&*self.0.get()).len() as i32 }
    }
    unsafe fn getEvent(&self, index: i32, event: *mut Event) -> tresult {
        if event.is_null() {
            return kInvalidArgument;
        }
        let Some(value) = (unsafe { &*self.0.get() }).get(index as usize) else {
            return kInvalidArgument;
        };
        unsafe {
            event.write(*value);
        }
        kResultOk
    }
    unsafe fn addEvent(&self, event: *mut Event) -> tresult {
        if event.is_null() {
            return kInvalidArgument;
        }
        if self.push(unsafe { *event }) {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

pub(super) fn note(channel: u8, pitch: u8, velocity: u8, on: bool) -> Event {
    Event {
        busIndex: 0,
        sampleOffset: 0,
        ppqPosition: 0.,
        flags: 0,
        r#type: if on { 0 } else { 1 },
        __field0: if on {
            Event__type0 {
                noteOn: NoteOnEvent {
                    channel: channel as i16,
                    pitch: pitch as i16,
                    tuning: 0.,
                    velocity: velocity as f32 / 127.,
                    length: 0,
                    noteId: -1,
                },
            }
        } else {
            Event__type0 {
                noteOff: NoteOffEvent {
                    channel: channel as i16,
                    pitch: pitch as i16,
                    tuning: 0.,
                    velocity: velocity as f32 / 127.,
                    noteId: -1,
                },
            }
        },
    }
}
