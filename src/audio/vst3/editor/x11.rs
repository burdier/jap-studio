//! ABI Xlib Linux. Conexión exclusiva del hilo del editor; biblioteca ya presente con GPUI.
use libc::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::{
    ffi::{CStr, CString},
    ptr,
    rc::Rc,
};
pub(super) type Display = c_void;
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Header {
    pub kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut Display,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Client {
    pub header: Header,
    pub window: c_ulong,
    pub message_type: c_ulong,
    pub format: c_int,
    pub data: [c_long; 5],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Configure {
    pub header: Header,
    pub event: c_ulong,
    pub window: c_ulong,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    border: c_int,
    above: c_ulong,
    override_redirect: c_int,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Key {
    pub header: Header,
    pub window: c_ulong,
    root: c_ulong,
    subwindow: c_ulong,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    pub state: c_uint,
    keycode: c_uint,
    same_screen: c_int,
}
#[repr(C)]
pub(super) union Event {
    pub kind: c_int,
    pub client: Client,
    pub configure: Configure,
    pub key: Key,
    pad: [c_long; 24],
}
pub(super) struct X11 {
    library: *mut c_void,
    pub display: *mut Display,
    pub window: c_ulong,
    pub delete: c_ulong,
    pub protocols: c_ulong,
    pub resize: unsafe extern "C" fn(*mut Display, c_ulong, c_uint, c_uint) -> c_int,
    pub map: unsafe extern "C" fn(*mut Display, c_ulong) -> c_int,
    pub flush: unsafe extern "C" fn(*mut Display) -> c_int,
    sync: unsafe extern "C" fn(*mut Display, c_int) -> c_int,
    pending: unsafe extern "C" fn(*mut Display) -> c_int,
    next: unsafe extern "C" fn(*mut Display, *mut Event) -> c_int,
    lookup: unsafe extern "C" fn(*mut Key, *mut c_char, c_int, *mut c_ulong, *mut c_void) -> c_int,
    destroy: unsafe extern "C" fn(*mut Display, c_ulong) -> c_int,
    close: unsafe extern "C" fn(*mut Display) -> c_int,
}
impl X11 {
    pub fn open(title: &str, width: u32, height: u32) -> Result<Rc<Self>, String> {
        unsafe {
            let library = libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if library.is_null() {
                return Err("No se encontró libX11 para el editor VST3".into());
            }
            let result = Self::from_library(library, title, width, height);
            if result.is_err() {
                libc::dlclose(library);
            }
            result.map(Rc::new)
        }
    }
    unsafe fn from_library(
        library: *mut c_void,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        unsafe {
            let title = CString::new(title).map_err(|e| e.to_string())?;
            let open: unsafe extern "C" fn(*const c_char) -> *mut Display =
                symbol(library, c"XOpenDisplay")?;
            let root: unsafe extern "C" fn(*mut Display) -> c_ulong =
                symbol(library, c"XDefaultRootWindow")?;
            let create: unsafe extern "C" fn(
                *mut Display,
                c_ulong,
                c_int,
                c_int,
                c_uint,
                c_uint,
                c_uint,
                c_ulong,
                c_ulong,
            ) -> c_ulong = symbol(library, c"XCreateSimpleWindow")?;
            let select: unsafe extern "C" fn(*mut Display, c_ulong, c_long) -> c_int =
                symbol(library, c"XSelectInput")?;
            let name: unsafe extern "C" fn(*mut Display, c_ulong, *const c_char) -> c_int =
                symbol(library, c"XStoreName")?;
            let atom: unsafe extern "C" fn(*mut Display, *const c_char, c_int) -> c_ulong =
                symbol(library, c"XInternAtom")?;
            let protocols: unsafe extern "C" fn(
                *mut Display,
                c_ulong,
                *mut c_ulong,
                c_int,
            ) -> c_int = symbol(library, c"XSetWMProtocols")?;
            // Resolver todo antes de abrir recursos X11 para que los errores no filtren handles.
            let mut api = Self {
                library,
                display: ptr::null_mut(),
                window: 0,
                delete: 0,
                protocols: 0,
                resize: symbol(library, c"XResizeWindow")?,
                map: symbol(library, c"XMapRaised")?,
                flush: symbol(library, c"XFlush")?,
                sync: symbol(library, c"XSync")?,
                pending: symbol(library, c"XPending")?,
                next: symbol(library, c"XNextEvent")?,
                lookup: symbol(library, c"XLookupString")?,
                destroy: symbol(library, c"XDestroyWindow")?,
                close: symbol(library, c"XCloseDisplay")?,
            };
            api.display = open(ptr::null());
            if api.display.is_null() {
                std::mem::forget(api);
                return Err("El editor VST3 requiere DISPLAY (X11/XWayland)".into());
            }
            api.window = create(
                api.display,
                root(api.display),
                80,
                80,
                width,
                height,
                0,
                0,
                0x202224,
            );
            select(api.display, api.window, (1 << 17) | (1 << 21) | 1 | 2);
            name(api.display, api.window, title.as_ptr());
            api.delete = atom(api.display, c"WM_DELETE_WINDOW".as_ptr(), 0);
            api.protocols = atom(api.display, c"WM_PROTOCOLS".as_ptr(), 0);
            protocols(api.display, api.window, &mut api.delete, 1);
            Ok(api)
        }
    }
    pub fn poll(&self) -> Option<Event> {
        unsafe {
            if (self.pending)(self.display) == 0 {
                return None;
            }
            let mut event = Event { pad: [0; 24] };
            (self.next)(self.display, &mut event);
            Some(event)
        }
    }
    pub fn key(&self, event: &mut Key) -> (u16, u64) {
        let mut bytes = [0; 16];
        let mut symbol = 0;
        let count =
            unsafe { (self.lookup)(event, bytes.as_mut_ptr(), 16, &mut symbol, ptr::null_mut()) };
        (if count == 1 { bytes[0] as u8 as u16 } else { 0 }, symbol)
    }
    pub fn raise(&self) {
        unsafe {
            (self.map)(self.display, self.window);
            (self.sync)(self.display, 0);
        }
    }
    pub fn resize(&self, width: u32, height: u32) {
        unsafe {
            (self.resize)(self.display, self.window, width, height);
            (self.flush)(self.display);
        }
    }
}
impl Drop for X11 {
    fn drop(&mut self) {
        unsafe {
            if !self.display.is_null() {
                (self.destroy)(self.display, self.window);
                (self.close)(self.display);
            }
            libc::dlclose(self.library);
        }
    }
}
unsafe fn symbol<T: Copy>(library: *mut c_void, name: &CStr) -> Result<T, String> {
    let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
    if pointer.is_null() {
        return Err(format!("X11: falta {}", name.to_string_lossy()));
    }
    Ok(unsafe { std::mem::transmute_copy(&pointer) })
}
