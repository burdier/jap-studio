//! ABI C de sfizz 1.2.x. La biblioteca y sus símbolos viven tanto como el motor.
use libc::{c_char, c_void};
use std::{ffi::CString, sync::Arc};

pub(super) type Synth = c_void;
pub(super) struct Api {
    handle: *mut c_void,
    pub create: unsafe extern "C" fn() -> *mut Synth,
    pub free: unsafe extern "C" fn(*mut Synth),
    pub load: unsafe extern "C" fn(*mut Synth, *const c_char, *const c_char) -> bool,
    pub rate: unsafe extern "C" fn(*mut Synth, f32),
    pub block: unsafe extern "C" fn(*mut Synth, i32),
    pub voices: unsafe extern "C" fn(*mut Synth, i32),
    pub on: unsafe extern "C" fn(*mut Synth, i32, i32, i32),
    pub off: unsafe extern "C" fn(*mut Synth, i32, i32, i32),
    pub cc: unsafe extern "C" fn(*mut Synth, i32, i32, i32),
    pub hdcc: unsafe extern "C" fn(*mut Synth, i32, i32, f32),
    pub bend: unsafe extern "C" fn(*mut Synth, i32, i32),
    pub render: unsafe extern "C" fn(*mut Synth, *mut *mut f32, i32, i32),
}
// Solo contiene funciones inmutables y un handle de dlopen; el synth tiene dueño exclusivo.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}
impl Drop for Api {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.handle);
        }
    }
}
impl Api {
    pub fn open() -> Result<Arc<Self>, String> {
        let mut candidates = Vec::new();
        if let Ok(path) = std::env::var("HAY_SFIZZ") {
            candidates.push(path);
        }
        candidates.extend(["libsfizz.so.1", "libsfizz.so", "libsfizz.dylib"].map(str::to_owned));
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(
                cwd.join(".local/sfizz/lib/libsfizz.so")
                    .to_string_lossy()
                    .into_owned(),
            );
            candidates.push(
                cwd.join(".local/sfizz/lib64/libsfizz.so")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        for path in candidates {
            let Ok(path) = CString::new(path) else {
                continue;
            };
            let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
            if handle.is_null() {
                continue;
            }
            let result = unsafe { Self::from_handle(handle) };
            if result.is_err() {
                unsafe {
                    libc::dlclose(handle);
                }
            }
            return result.map(Arc::new);
        }
        Err("sfizz no está instalado. Ejecuta scripts/build-sfizz.sh o configura HAY_SFIZZ con la biblioteca compartida".into())
    }
    unsafe fn from_handle(handle: *mut c_void) -> Result<Self, String> {
        unsafe fn symbol<T: Copy>(handle: *mut c_void, name: &std::ffi::CStr) -> Result<T, String> {
            let ptr = unsafe { libc::dlsym(handle, name.as_ptr()) };
            if ptr.is_null() {
                return Err(format!("sfizz: falta {}", name.to_string_lossy()));
            }
            Ok(unsafe { std::mem::transmute_copy(&ptr) })
        }
        unsafe {
            Ok(Self {
                handle,
                create: symbol(handle, c"sfizz_create_synth")?,
                free: symbol(handle, c"sfizz_free")?,
                load: symbol(handle, c"sfizz_load_string")?,
                rate: symbol(handle, c"sfizz_set_sample_rate")?,
                block: symbol(handle, c"sfizz_set_samples_per_block")?,
                voices: symbol(handle, c"sfizz_set_num_voices")?,
                on: symbol(handle, c"sfizz_send_note_on")?,
                off: symbol(handle, c"sfizz_send_note_off")?,
                cc: symbol(handle, c"sfizz_send_cc")?,
                hdcc: symbol(handle, c"sfizz_send_hdcc")?,
                bend: symbol(handle, c"sfizz_send_pitch_wheel")?,
                render: symbol(handle, c"sfizz_render_block")?,
            })
        }
    }
}
