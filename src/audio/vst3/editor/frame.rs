//! Frame y run loop VST3 Linux; todos los callbacks se ejecutan en el hilo del editor.
#![allow(non_snake_case)]
use super::x11::X11;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
use vst3::{
    Class, ComPtr, ComRef,
    Steinberg::{Linux::*, *},
};
struct Timer {
    handler: ComPtr<ITimerHandler>,
    interval: Duration,
    next: Instant,
}
pub(super) struct Frame {
    pub x11: Rc<X11>,
    pub size: Cell<(i32, i32)>,
    files: RefCell<Vec<(i32, ComPtr<IEventHandler>)>>,
    timers: RefCell<Vec<Timer>>,
}
impl Frame {
    pub fn new(x11: Rc<X11>, size: (i32, i32)) -> Self {
        Self {
            x11,
            size: Cell::new(size),
            files: RefCell::new(Vec::new()),
            timers: RefCell::new(Vec::new()),
        }
    }
    pub fn tick(&self) {
        let callbacks: Vec<_> = self
            .files
            .borrow()
            .iter()
            .filter_map(|(fd, handler)| {
                let mut poll = libc::pollfd {
                    fd: *fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                if unsafe { libc::poll(&mut poll, 1, 0) } > 0 && poll.revents & libc::POLLIN != 0 {
                    Some((*fd, handler.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (fd, handler) in callbacks {
            if self
                .files
                .borrow()
                .iter()
                .any(|(f, h)| *f == fd && h.as_ptr() == handler.as_ptr())
            {
                unsafe {
                    handler.onFDIsSet(fd);
                }
            }
        }
        let now = Instant::now();
        let callbacks: Vec<_> = self
            .timers
            .borrow_mut()
            .iter_mut()
            .filter_map(|timer| {
                if timer.next > now {
                    return None;
                }
                timer.next = now + timer.interval;
                Some(timer.handler.clone())
            })
            .collect();
        for handler in callbacks {
            if self
                .timers
                .borrow()
                .iter()
                .any(|timer| timer.handler.as_ptr() == handler.as_ptr())
            {
                unsafe {
                    handler.onTimer();
                }
            }
        }
    }
}
impl Class for Frame {
    type Interfaces = (IPlugFrame, IRunLoop);
}
impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, view: *mut IPlugView, size: *mut ViewRect) -> tresult {
        let Some(view) = (unsafe { ComRef::from_raw(view) }) else {
            return kInvalidArgument;
        };
        if size.is_null() {
            return kInvalidArgument;
        }
        let rect = unsafe { *size };
        let dimensions = (rect.right - rect.left, rect.bottom - rect.top);
        if !valid(dimensions) {
            return kInvalidArgument;
        }
        if dimensions != self.size.get() {
            self.size.set(dimensions);
            self.x11.resize(dimensions.0 as u32, dimensions.1 as u32);
            return unsafe { view.onSize(size) };
        }
        kResultOk
    }
}
pub(super) fn valid(size: (i32, i32)) -> bool {
    (1..=16384).contains(&size.0) && (1..=16384).contains(&size.1)
}
impl IRunLoopTrait for Frame {
    unsafe fn registerEventHandler(&self, handler: *mut IEventHandler, fd: i32) -> tresult {
        let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
            return kInvalidArgument;
        };
        let mut files = self.files.borrow_mut();
        if fd < 0 || files.len() >= 128 {
            return kInvalidArgument;
        }
        if !files
            .iter()
            .any(|(f, h)| *f == fd && h.as_ptr() == handler.as_ptr())
        {
            files.push((fd, handler.to_com_ptr()));
        }
        kResultOk
    }
    unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
        self.files
            .borrow_mut()
            .retain(|(_, h)| h.as_ptr() != handler);
        kResultOk
    }
    unsafe fn registerTimer(&self, handler: *mut ITimerHandler, milliseconds: u64) -> tresult {
        let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
            return kInvalidArgument;
        };
        let mut timers = self.timers.borrow_mut();
        if timers.len() >= 128 {
            return kOutOfMemory;
        }
        timers.retain(|timer| timer.handler.as_ptr() != handler.as_ptr());
        let interval = Duration::from_millis(milliseconds.clamp(1, 86_400_000));
        timers.push(Timer {
            handler: handler.to_com_ptr(),
            interval,
            next: Instant::now() + interval,
        });
        kResultOk
    }
    unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
        self.timers
            .borrow_mut()
            .retain(|timer| timer.handler.as_ptr() != handler);
        kResultOk
    }
}
