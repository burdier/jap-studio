//! libsndfile 1.x (LGPL-2.1-or-later): escritura PCM/FLAC y cierre comprobado.
use super::native::Library;
use elevate::export::{BitDepth, ExportOptions, Format};
use libc::{c_char, c_int, c_void};
use std::{ffi::CStr, fs::File, os::fd::AsRawFd};

#[repr(C)]
struct Info {
    frames: i64,
    samplerate: c_int,
    channels: c_int,
    format: c_int,
    sections: c_int,
    seekable: c_int,
}

pub(super) struct Writer {
    handle: *mut c_void,
    write: unsafe extern "C" fn(*mut c_void, *const f32, i64) -> i64,
    close: unsafe extern "C" fn(*mut c_void) -> c_int,
    error: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    file: File,
    channels: usize,
    _library: Library,
}

impl Writer {
    pub fn new(file: File, format: Format, options: &ExportOptions) -> Result<Self, String> {
        let library = Library::open(c"libsndfile.so.1")?;
        let container = match format {
            Format::Wav => 0x010000,
            Format::Aiff => 0x020000,
            Format::Flac => 0x170000,
            _ => return Err("Formato PCM inválido".into()),
        };
        let subtype = match options.bits {
            BitDepth::Int16 => 2,
            BitDepth::Int24 => 3,
            BitDepth::Int32 => 4,
            BitDepth::Float32 => 6,
        };
        let channels = if options.mono { 1 } else { 2 };
        let mut info = Info {
            frames: 0,
            samplerate: options.sample_rate as c_int,
            channels,
            format: container | subtype,
            sections: 0,
            seekable: 0,
        };
        // SAFETY: firmas de sndfile.h; File conserva el descriptor (close_desc=0).
        unsafe {
            let open: unsafe extern "C" fn(c_int, c_int, *mut Info, c_int) -> *mut c_void =
                library.symbol(c"sf_open_fd")?;
            let check: unsafe extern "C" fn(*const Info) -> c_int =
                library.symbol(c"sf_format_check")?;
            let command: unsafe extern "C" fn(*mut c_void, c_int, *mut c_void, c_int) -> c_int =
                library.symbol(c"sf_command")?;
            let write = library.symbol(c"sf_writef_float")?;
            let close = library.symbol(c"sf_close")?;
            let error: unsafe extern "C" fn(*mut c_void) -> *const c_char =
                library.symbol(c"sf_strerror")?;
            if check(&info) == 0 {
                return Err(
                    "libsndfile no admite esa combinación de formato/frecuencia/profundidad".into(),
                );
            }
            let handle = open(file.as_raw_fd(), 0x20, &mut info, 0);
            if handle.is_null() {
                return Err(CStr::from_ptr(error(handle)).to_string_lossy().into_owned());
            }
            command(handle, 0x10C0, std::ptr::null_mut(), 1); // SFC_SET_CLIPPING: evitar wrap de PCM entero.
            Ok(Self {
                handle,
                write,
                close,
                error,
                file,
                channels: channels as usize,
                _library: library,
            })
        }
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if samples.len() % self.channels != 0 {
            return Err("Bloque PCM incompleto".into());
        }
        let frames = (samples.len() / self.channels) as i64;
        let written = unsafe { (self.write)(self.handle, samples.as_ptr(), frames) };
        if written != frames {
            return Err(unsafe { CStr::from_ptr((self.error)(self.handle)) }
                .to_string_lossy()
                .into_owned());
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), String> {
        let status = unsafe { (self.close)(self.handle) };
        self.handle = std::ptr::null_mut();
        if status != 0 {
            return Err(format!("libsndfile: error {status} al finalizar"));
        }
        self.file.sync_all().map_err(|e| e.to_string())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                (self.close)(self.handle);
            }
        }
    }
}
