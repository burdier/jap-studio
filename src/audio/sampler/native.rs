//! ABI C de sfizz 1.2.x. La biblioteca y sus símbolos viven tanto como el motor.
use crate::audio::dynlib::DynLib;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    sync::Arc,
};

pub(super) type Synth = c_void;
pub(super) struct Api {
    _lib: DynLib,
    pub create: unsafe extern "C" fn() -> *mut Synth,
    pub cut: unsafe extern "C" fn(*mut Synth),
    pub free: unsafe extern "C" fn(*mut Synth),
    pub load: unsafe extern "C" fn(*mut Synth, *const c_char, *const c_char) -> bool,
    pub rate: unsafe extern "C" fn(*mut Synth, f32),
    pub block: unsafe extern "C" fn(*mut Synth, i32),
    pub voices: unsafe extern "C" fn(*mut Synth, i32),
    pub volume: unsafe extern "C" fn(*mut Synth, f32),
    pub active_voices: unsafe extern "C" fn(*mut Synth) -> i32,
    pub on: unsafe extern "C" fn(*mut Synth, i32, i32, i32),
    pub off: unsafe extern "C" fn(*mut Synth, i32, i32, i32),
    pub cc: unsafe extern "C" fn(*mut Synth, i32, i32, i32),
    pub hdcc: unsafe extern "C" fn(*mut Synth, i32, i32, f32),
    pub pressure: unsafe extern "C" fn(*mut Synth, i32, i32),
    pub bend: unsafe extern "C" fn(*mut Synth, i32, i32),
    pub render: unsafe extern "C" fn(*mut Synth, *mut *mut f32, i32, i32),
}
// Solo contiene funciones inmutables y un DynLib; el synth tiene dueño exclusivo.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}
impl Api {
    pub fn open() -> Result<Arc<Self>, String> {
        let mut candidates = Vec::new();
        if let Ok(path) = std::env::var("HAY_SFIZZ") {
            candidates.push(path);
        }
        #[cfg(target_os = "windows")]
        candidates.extend(["sfizz.dll", "libsfizz.dll"].map(str::to_owned));
        #[cfg(target_os = "macos")]
        candidates.extend(["libsfizz.dylib", "libsfizz.1.dylib"].map(str::to_owned));
        #[cfg(target_os = "linux")]
        candidates.extend(["libsfizz.so.1", "libsfizz.so"].map(str::to_owned));
        if let Ok(cwd) = std::env::current_dir() {
            #[cfg(target_os = "windows")]
            candidates.push(cwd.join("sfizz.dll").to_string_lossy().into_owned());
            #[cfg(target_os = "linux")]
            {
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
        }
        for path in candidates {
            let Ok(path) = CString::new(path) else {
                continue;
            };
            let Ok(lib) = DynLib::open(&path) else {
                continue;
            };
            return unsafe { Self::from_lib(lib) }.map(Arc::new);
        }
        Err("sfizz no está instalado. Configura HAY_SFIZZ con la biblioteca compartida".into())
    }
    unsafe fn from_lib(lib: DynLib) -> Result<Self, String> {
        unsafe fn sym<T: Copy>(lib: &DynLib, name: &CStr) -> Result<T, String> {
            unsafe { lib.symbol(name) }.map_err(|_| format!("sfizz: falta {}", name.to_string_lossy()))
        }
        unsafe {
            Ok(Self {
                create: sym(&lib, c"sfizz_create_synth")?,
                cut: sym(&lib, c"sfizz_all_sound_off")?,
                free: sym(&lib, c"sfizz_free")?,
                load: sym(&lib, c"sfizz_load_string")?,
                rate: sym(&lib, c"sfizz_set_sample_rate")?,
                block: sym(&lib, c"sfizz_set_samples_per_block")?,
                voices: sym(&lib, c"sfizz_set_num_voices")?,
                volume: sym(&lib, c"sfizz_set_volume")?,
                active_voices: sym(&lib, c"sfizz_get_num_active_voices")?,
                on: sym(&lib, c"sfizz_send_note_on")?,
                off: sym(&lib, c"sfizz_send_note_off")?,
                cc: sym(&lib, c"sfizz_send_cc")?,
                hdcc: sym(&lib, c"sfizz_send_hdcc")?,
                pressure: sym(&lib, c"sfizz_send_channel_aftertouch")?,
                bend: sym(&lib, c"sfizz_send_pitch_wheel")?,
                render: sym(&lib, c"sfizz_render_block")?,
                _lib: lib,
            })
        }
    }
}
