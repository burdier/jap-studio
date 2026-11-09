//! LAME 3.100 (LGPL): float normalizado, CBR/VBR, flush y etiqueta de delay/padding.
use super::native::Library;
use elevate::export::{ExportOptions, Mp3Mode};
use libc::{c_int, c_void};
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
};

pub(super) struct Writer {
    handle: *mut c_void,
    encode:
        unsafe extern "C" fn(*mut c_void, *const f32, *const f32, c_int, *mut u8, c_int) -> c_int,
    flush: unsafe extern "C" fn(*mut c_void, *mut u8, c_int) -> c_int,
    tag: unsafe extern "C" fn(*const c_void, *mut u8, usize) -> usize,
    close: unsafe extern "C" fn(*mut c_void) -> c_int,
    file: File,
    channels: usize,
    left: Vec<f32>,
    right: Vec<f32>,
    output: Vec<u8>,
    _library: Library,
}

impl Writer {
    pub fn new(file: File, options: &ExportOptions) -> Result<Self, String> {
        let library = Library::open(c"libmp3lame.so.0")?;
        // SAFETY: firmas de lame.h. Todos los símbolos se resuelven antes de crear el handle.
        unsafe {
            let init: unsafe extern "C" fn() -> *mut c_void = library.symbol(c"lame_init")?;
            let init_params: unsafe extern "C" fn(*mut c_void) -> c_int =
                library.symbol(c"lame_init_params")?;
            let encode = library.symbol(c"lame_encode_buffer_ieee_float")?;
            let flush = library.symbol(c"lame_encode_flush")?;
            let tag = library.symbol(c"lame_get_lametag_frame")?;
            let close = library.symbol(c"lame_close")?;
            let handle = init();
            if handle.is_null() {
                return Err("LAME: no se pudo crear el codificador".into());
            }
            let channels = if options.mono { 1 } else { 2 };
            let writer = Self {
                handle,
                encode,
                flush,
                tag,
                close,
                file,
                channels,
                left: Vec::new(),
                right: Vec::new(),
                output: vec![0; 16_384],
                _library: library,
            };
            for (name, value) in [
                (c"lame_set_in_samplerate", options.sample_rate as c_int),
                (c"lame_set_out_samplerate", options.sample_rate as c_int),
                (c"lame_set_num_channels", channels as c_int),
                (c"lame_set_quality", 2),
                (c"lame_set_bWriteVbrTag", 1),
            ] {
                writer.set(name, value)?;
            }
            match options.mp3 {
                Mp3Mode::Cbr(kbps) => {
                    writer.set(c"lame_set_VBR", 0)?;
                    writer.set(c"lame_set_brate", kbps as c_int)?;
                }
                Mp3Mode::Vbr(q) => {
                    writer.set(c"lame_set_VBR", 4)?;
                    writer.set(c"lame_set_VBR_q", q as c_int)?;
                }
            }
            if init_params(handle) < 0 {
                return Err("LAME: parámetros incompatibles".into());
            }
            Ok(writer)
        }
    }

    fn set(&self, name: &std::ffi::CStr, value: c_int) -> Result<(), String> {
        let set: unsafe extern "C" fn(*mut c_void, c_int) -> c_int =
            unsafe { self._library.symbol(name)? };
        if unsafe { set(self.handle, value) } < 0 {
            return Err(format!("LAME rechazó {}={value}", name.to_string_lossy()));
        }
        Ok(())
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if samples.len() % self.channels != 0 {
            return Err("Bloque MP3 incompleto".into());
        }
        let frames = samples.len() / self.channels;
        if frames > 4096 {
            return Err("Bloque MP3 demasiado grande".into());
        }
        self.left.clear();
        self.right.clear();
        for frame in samples.chunks_exact(self.channels) {
            self.left.push(frame[0].clamp(-1.0, 1.0));
            self.right.push(frame[self.channels - 1].clamp(-1.0, 1.0));
        }
        let count = unsafe {
            (self.encode)(
                self.handle,
                self.left.as_ptr(),
                self.right.as_ptr(),
                frames as c_int,
                self.output.as_mut_ptr(),
                self.output.len() as c_int,
            )
        };
        self.write_encoded(count)
    }

    fn write_encoded(&mut self, count: c_int) -> Result<(), String> {
        if count < 0 {
            return Err(format!("LAME: error de codificación {count}"));
        }
        self.file
            .write_all(&self.output[..count as usize])
            .map_err(|e| e.to_string())
    }

    pub fn finish(mut self) -> Result<(), String> {
        let count = unsafe {
            (self.flush)(
                self.handle,
                self.output.as_mut_ptr(),
                self.output.len() as c_int,
            )
        };
        self.write_encoded(count)?;
        let size = unsafe { (self.tag)(self.handle, self.output.as_mut_ptr(), self.output.len()) };
        if size > self.output.len() {
            return Err("Etiqueta LAME mayor que el buffer".into());
        }
        if size > 0 {
            // No escribimos ID3 antes del audio: el frame reservado está en offset cero.
            self.file
                .seek(SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            self.file
                .write_all(&self.output[..size])
                .map_err(|e| e.to_string())?;
        }
        self.file.sync_all().map_err(|e| e.to_string())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.handle);
        }
    }
}
