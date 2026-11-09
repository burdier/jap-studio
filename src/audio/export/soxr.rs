//! libsoxr 0.1 (LGPL-2.1-or-later). Conversión offline VHQ de fuentes en memoria.
use super::native::Library;
use elevate::StereoFrame;
use libc::{c_char, c_ulong, c_void};
use std::ffi::CStr;

#[repr(C)]
struct Quality {
    precision: f64,
    phase: f64,
    passband: f64,
    stopband: f64,
    reserved: *mut c_void,
    flags: c_ulong,
}

pub(super) fn resample(
    input: &[StereoFrame],
    from: u32,
    to: u32,
) -> Result<Vec<StereoFrame>, String> {
    if from == 0 || to == 0 {
        return Err("Frecuencia de fuente inválida".into());
    }
    let library = Library::open(c"libsoxr.so.0")?;
    let length = (input.len() as u128 * to as u128).div_ceil(from as u128);
    let length = usize::try_from(length).map_err(|_| "Audio demasiado largo")?;
    let capacity = length.checked_add(32).ok_or("Audio demasiado largo")?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|e| e.to_string())?;
    output.resize(capacity, [0.0; 2]);
    // SAFETY: ABI de soxr.h. [f32; 2] es estéreo interleaved contiguo; tamaños en frames.
    unsafe {
        let quality: unsafe extern "C" fn(c_ulong, c_ulong) -> Quality =
            library.symbol(c"soxr_quality_spec")?;
        let convert: unsafe extern "C" fn(
            f64,
            f64,
            u32,
            *const c_void,
            usize,
            *mut usize,
            *mut c_void,
            usize,
            *mut usize,
            *const c_void,
            *const Quality,
            *const c_void,
        ) -> *const c_char = library.symbol(c"soxr_oneshot")?;
        let quality = quality(6, 0); // SOXR_VHQ; lineal, sin dither en float.
        let (mut consumed, mut produced) = (0, 0);
        let error = convert(
            from as f64,
            to as f64,
            2,
            input.as_ptr().cast(),
            input.len(),
            &mut consumed,
            output.as_mut_ptr().cast(),
            capacity,
            &mut produced,
            std::ptr::null(),
            &quality,
            std::ptr::null(),
        );
        if !error.is_null() {
            return Err(CStr::from_ptr(error).to_string_lossy().into_owned());
        }
        if consumed != input.len() {
            return Err("libsoxr no consumió toda la fuente".into());
        }
        output.truncate(produced);
    }
    Ok(output)
}
