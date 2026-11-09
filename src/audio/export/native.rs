//! Carga dinámica local al worker, como FluidSynth. Ningún puntero sale de su adaptador.
use libc::{RTLD_LOCAL, RTLD_NOW, c_void, dlclose, dlerror, dlopen, dlsym};
use std::ffi::CStr;

pub(super) struct Library(*mut c_void);

impl Library {
    pub fn open(name: &CStr) -> Result<Self, String> {
        // SAFETY: cadena terminada en NUL; el handle se cierra al caer Library.
        let handle = unsafe { dlopen(name.as_ptr(), RTLD_LOCAL | RTLD_NOW) };
        if handle.is_null() {
            let error = unsafe { dlerror() };
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

    /// El llamador debe usar la firma C exacta y no usar el símbolo después de Library.
    pub unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, String> {
        let pointer = unsafe { dlsym(self.0, name.as_ptr()) };
        if pointer.is_null() {
            return Err(format!("Símbolo C ausente: {}", name.to_string_lossy()));
        }
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&pointer));
        Ok(unsafe { std::mem::transmute_copy(&pointer) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            dlclose(self.0);
        }
    }
}
