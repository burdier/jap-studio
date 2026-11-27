//! libsndfile 1.x (LGPL-2.1-or-later) en Unix; escritor WAV PCM/Float32 nativo en Windows.
#[cfg(unix)]
mod platform {
    use super::super::native::Library;
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

    pub(in super::super) struct Writer {
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
            #[cfg(target_os = "macos")]
            let candidates: &[&CStr] = &[c"libsndfile.1.dylib", c"libsndfile.dylib"];
            #[cfg(not(target_os = "macos"))]
            let candidates: &[&CStr] = &[c"libsndfile.so.1", c"libsndfile.so"];
            let library = candidates
                .iter()
                .find_map(|name| Library::open(name).ok())
                .ok_or("No se encontró libsndfile en el sistema")?;
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
                        "libsndfile no admite esa combinación de formato/frecuencia/profundidad"
                            .into(),
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
}

#[cfg(not(unix))]
mod platform {
    use elevate::export::{BitDepth, ExportOptions, Format};
    use std::{
        fs::File,
        io::{BufWriter, Seek, SeekFrom, Write},
    };

    pub(in super::super) struct Writer {
        file: BufWriter<File>,
        channels: u16,
        bits: BitDepth,
        data_bytes: u64,
    }

    impl Writer {
        pub fn new(file: File, format: Format, options: &ExportOptions) -> Result<Self, String> {
            if format != Format::Wav {
                return Err("En Windows actualmente la exportación nativa admite WAV (PCM/Float32)".into());
            }
            let channels: u16 = if options.mono { 1 } else { 2 };
            let (audio_format, bits_per_sample): (u16, u16) = match options.bits {
                BitDepth::Int16 => (1, 16),
                BitDepth::Int24 => (1, 24),
                BitDepth::Int32 => (1, 32),
                BitDepth::Float32 => (3, 32),
            };
            let block_align = channels * (bits_per_sample / 8);
            let byte_rate = options.sample_rate * u32::from(block_align);
            let mut w = BufWriter::new(file);
            // Cabecera RIFF/WAVE estándar de 44 bytes (tamaños actualizados en finish).
            w.write_all(b"RIFF").map_err(|e| e.to_string())?;
            w.write_all(&0u32.to_le_bytes()).map_err(|e| e.to_string())?;
            w.write_all(b"WAVE").map_err(|e| e.to_string())?;
            w.write_all(b"fmt ").map_err(|e| e.to_string())?;
            w.write_all(&16u32.to_le_bytes()).map_err(|e| e.to_string())?;
            w.write_all(&audio_format.to_le_bytes())
                .map_err(|e| e.to_string())?;
            w.write_all(&channels.to_le_bytes())
                .map_err(|e| e.to_string())?;
            w.write_all(&options.sample_rate.to_le_bytes())
                .map_err(|e| e.to_string())?;
            w.write_all(&byte_rate.to_le_bytes())
                .map_err(|e| e.to_string())?;
            w.write_all(&block_align.to_le_bytes())
                .map_err(|e| e.to_string())?;
            w.write_all(&bits_per_sample.to_le_bytes())
                .map_err(|e| e.to_string())?;
            w.write_all(b"data").map_err(|e| e.to_string())?;
            w.write_all(&0u32.to_le_bytes()).map_err(|e| e.to_string())?;
            Ok(Self {
                file: w,
                channels,
                bits: options.bits,
                data_bytes: 0,
            })
        }

        pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
            if samples.len() % usize::from(self.channels) != 0 {
                return Err("Bloque PCM incompleto".into());
            }
            let mut buf = Vec::with_capacity(samples.len() * 4);
            match self.bits {
                BitDepth::Int16 => {
                    for &s in samples {
                        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
                }
                BitDepth::Int24 => {
                    for &s in samples {
                        let v = (s.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
                        let b = v.to_le_bytes();
                        buf.extend_from_slice(&b[0..3]);
                    }
                }
                BitDepth::Int32 => {
                    for &s in samples {
                        let v = (f64::from(s.clamp(-1.0, 1.0)) * 2_147_483_647.0).round() as i32;
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
                }
                BitDepth::Float32 => {
                    for &s in samples {
                        buf.extend_from_slice(&s.to_le_bytes());
                    }
                }
            }
            self.file.write_all(&buf).map_err(|e| e.to_string())?;
            self.data_bytes = self.data_bytes.saturating_add(buf.len() as u64);
            Ok(())
        }

        pub fn finish(mut self) -> Result<(), String> {
            self.file.flush().map_err(|e| e.to_string())?;
            let data_size = u32::try_from(self.data_bytes).unwrap_or(u32::MAX);
            let riff_size = data_size.saturating_add(36);
            let mut raw = self.file.into_inner().map_err(|e| e.to_string())?;
            raw.seek(SeekFrom::Start(4)).map_err(|e| e.to_string())?;
            raw.write_all(&riff_size.to_le_bytes())
                .map_err(|e| e.to_string())?;
            raw.seek(SeekFrom::Start(40)).map_err(|e| e.to_string())?;
            raw.write_all(&data_size.to_le_bytes())
                .map_err(|e| e.to_string())?;
            raw.sync_all().map_err(|e| e.to_string())
        }
    }
}

pub(super) use platform::Writer;
