//! Cargador dinámico multiplataforma (.so en Linux, .dylib en macOS, .dll en Windows).
//! Sin dependencias externas: usa `dlopen/dlsym/dlclose` en Unix y `kernel32` en Windows.
use std::ffi::{CStr, c_void};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryA(lp_lib_file_name: *const std::ffi::c_char) -> *mut c_void;
    fn GetProcAddress(
        h_module: *mut c_void,
        lp_proc_name: *const std::ffi::c_char,
    ) -> *mut c_void;
    fn FreeLibrary(h_lib_module: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}

pub(crate) struct DynLib(*mut c_void);

// Los handles de biblioteca compartida se transfieren a workers de preparación o se cierran al soltar.
unsafe impl Send for DynLib {}
unsafe impl Sync for DynLib {}

impl DynLib {
    pub fn open(name: &CStr) -> Result<Self, String> {
        #[cfg(unix)]
        {
            let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_LOCAL | libc::RTLD_NOW) };
            if handle.is_null() {
                let error = unsafe { libc::dlerror() };
                let detail = if error.is_null() {
                    "dlopen falló".into()
                } else {
                    unsafe { CStr::from_ptr(error) }
                        .to_string_lossy()
                        .into_owned()
                };
                Err(format!(
                    "No se pudo cargar {}: {detail}",
                    name.to_string_lossy()
                ))
            } else {
                Ok(Self(handle))
            }
        }
        #[cfg(windows)]
        {
            let handle = unsafe { LoadLibraryA(name.as_ptr()) };
            if handle.is_null() {
                let code = unsafe { GetLastError() };
                Err(format!(
                    "No se pudo cargar {} (Win32 error {code})",
                    name.to_string_lossy()
                ))
            } else {
                Ok(Self(handle))
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(format!(
                "Carga dinámica no soportada en esta plataforma ({})",
                name.to_string_lossy()
            ))
        }
    }

    pub fn raw(&self) -> *mut c_void {
        self.0
    }

    /// Devuelve el handle crudo transfiriendo la propiedad al llamador (no ejecutará `Drop`).
    pub fn into_raw(self) -> *mut c_void {
        let ptr = self.0;
        std::mem::forget(self);
        ptr
    }

    /// El llamador debe usar la firma C exacta y no usar el símbolo después de cerrar la biblioteca.
    pub unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, String> {
        unsafe { symbol_raw(self.0, name) }
    }
}

pub(crate) unsafe fn symbol_raw<T: Copy>(handle: *mut c_void, name: &CStr) -> Result<T, String> {
    #[cfg(unix)]
    let pointer = unsafe { libc::dlsym(handle, name.as_ptr()) };
    #[cfg(windows)]
    let pointer = unsafe { GetProcAddress(handle, name.as_ptr()) };
    #[cfg(not(any(unix, windows)))]
    let pointer: *mut c_void = std::ptr::null_mut();

    if pointer.is_null() {
        return Err(format!("Símbolo C ausente: {}", name.to_string_lossy()));
    }
    assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&pointer));
    Ok(unsafe { std::mem::transmute_copy(&pointer) })
}

pub(crate) unsafe fn close_raw(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::dlclose(handle);
    }
    #[cfg(windows)]
    unsafe {
        let _ = FreeLibrary(handle);
    }
}

impl Drop for DynLib {
    fn drop(&mut self) {
        unsafe { close_raw(self.0) };
    }
}
