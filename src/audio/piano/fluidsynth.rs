//! Adaptador FFI aislado para `libfluidsynth` (`dlopen` dinámico en tiempo de ejecución).
//! Si `libfluidsynth.so.3` está disponible en el sistema, se usa como motor SoundFont C;
//! si no está instalado, `audio/piano` cae automáticamente a `rustysynth` sin romper la compilación.
use libc::{
    RTLD_LAZY, RTLD_LOCAL, c_char, c_double, c_float, c_int, c_uint, c_void, dlopen, dlsym,
};
use std::{ffi::CString, path::Path, sync::OnceLock};

mod voices;

// Este límite también acota las listas de voces usadas para expresión por nota.
const MAX_VOICES: usize = 256;
type FluidSettings = c_void;
type FluidSynth = c_void;

struct FluidApi {
    new_settings: unsafe extern "C" fn() -> *mut FluidSettings,
    delete_settings: unsafe extern "C" fn(*mut FluidSettings),
    settings_setnum: unsafe extern "C" fn(*mut FluidSettings, *const c_char, c_double) -> c_int,
    settings_setint: unsafe extern "C" fn(*mut FluidSettings, *const c_char, c_int) -> c_int,
    new_synth: unsafe extern "C" fn(*mut FluidSettings) -> *mut FluidSynth,
    delete_synth: unsafe extern "C" fn(*mut FluidSynth),
    sfload: unsafe extern "C" fn(*mut FluidSynth, *const c_char, c_int) -> c_int,
    get_voicelist: unsafe extern "C" fn(*mut FluidSynth, *mut *mut c_void, c_int, c_int),
    voice_get_id: unsafe extern "C" fn(*const c_void) -> c_uint,
    voice_gen_incr: unsafe extern "C" fn(*mut c_void, c_int, c_float),
    voice_update_param: unsafe extern "C" fn(*mut c_void, c_int),
    noteon: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int, c_int) -> c_int,
    noteoff: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int) -> c_int,
    all_notes_off: unsafe extern "C" fn(*mut FluidSynth, c_int) -> c_int,
    cc: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int, c_int) -> c_int,
    channel_pressure: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int) -> c_int,
    pitch_bend: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int) -> c_int,
    pitch_wheel_sens: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int) -> c_int,
    bank_select: unsafe extern "C" fn(*mut FluidSynth, c_int, c_uint) -> c_int,
    program_change: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int) -> c_int,
    set_gain: unsafe extern "C" fn(*mut FluidSynth, c_float),
    set_polyphony: unsafe extern "C" fn(*mut FluidSynth, c_int) -> c_int,
    reverb_on: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int),
    chorus_on: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int),
    set_gen: unsafe extern "C" fn(*mut FluidSynth, c_int, c_int, c_float) -> c_int,
    write_float: unsafe extern "C" fn(
        *mut FluidSynth,
        c_int,
        *mut c_void,
        c_int,
        c_int,
        *mut c_void,
        c_int,
        c_int,
    ) -> c_int,
}

unsafe impl Send for FluidApi {}
unsafe impl Sync for FluidApi {}

static API: OnceLock<Option<FluidApi>> = OnceLock::new();

unsafe fn sym<T>(handle: *mut c_void, name: &[u8]) -> Option<T> {
    let ptr = unsafe { dlsym(handle, name.as_ptr() as *const c_char) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

fn api() -> Option<&'static FluidApi> {
    API.get_or_init(|| unsafe {
        let candidates = [c"libfluidsynth.so.3".as_ptr(), c"libfluidsynth.so".as_ptr()];
        let handle = candidates
            .into_iter()
            .map(|name| dlopen(name, RTLD_LAZY | RTLD_LOCAL))
            .find(|h| !h.is_null())?;
        Some(FluidApi {
            new_settings: sym(handle, b"new_fluid_settings\0")?,
            delete_settings: sym(handle, b"delete_fluid_settings\0")?,
            settings_setnum: sym(handle, b"fluid_settings_setnum\0")?,
            settings_setint: sym(handle, b"fluid_settings_setint\0")?,
            new_synth: sym(handle, b"new_fluid_synth\0")?,
            delete_synth: sym(handle, b"delete_fluid_synth\0")?,
            sfload: sym(handle, b"fluid_synth_sfload\0")?,
            get_voicelist: sym(handle, b"fluid_synth_get_voicelist\0")?,
            voice_get_id: sym(handle, b"fluid_voice_get_id\0")?,
            voice_gen_incr: sym(handle, b"fluid_voice_gen_incr\0")?,
            voice_update_param: sym(handle, b"fluid_voice_update_param\0")?,
            noteon: sym(handle, b"fluid_synth_noteon\0")?,
            noteoff: sym(handle, b"fluid_synth_noteoff\0")?,
            all_notes_off: sym(handle, b"fluid_synth_all_notes_off\0")?,
            cc: sym(handle, b"fluid_synth_cc\0")?,
            channel_pressure: sym(handle, b"fluid_synth_channel_pressure\0")?,
            pitch_bend: sym(handle, b"fluid_synth_pitch_bend\0")?,
            pitch_wheel_sens: sym(handle, b"fluid_synth_pitch_wheel_sens\0")?,
            bank_select: sym(handle, b"fluid_synth_bank_select\0")?,
            program_change: sym(handle, b"fluid_synth_program_change\0")?,
            set_gain: sym(handle, b"fluid_synth_set_gain\0")?,
            set_polyphony: sym(handle, b"fluid_synth_set_polyphony\0")?,
            reverb_on: sym(handle, b"fluid_synth_reverb_on\0")?,
            chorus_on: sym(handle, b"fluid_synth_chorus_on\0")?,
            set_gen: sym(handle, b"fluid_synth_set_gen\0")?,
            write_float: sym(handle, b"fluid_synth_write_float\0")?,
        })
    })
    .as_ref()
}

pub(crate) fn is_available() -> bool {
    api().is_some()
}

pub(crate) struct FluidEngine {
    api: &'static FluidApi,
    settings: *mut FluidSettings,
    synth: *mut FluidSynth,
    rpn_msb: u8,
    rpn_lsb: u8,
    data_msb: u8,
}

unsafe impl Send for FluidEngine {}

impl FluidEngine {
    pub fn new(
        sf2_path: &Path,
        sample_rate: u32,
        polyphony: usize,
        effects: bool,
    ) -> Result<Self, String> {
        let api = api().ok_or("libfluidsynth.so.3 no está disponible")?;
        let c_path = CString::new(sf2_path.to_string_lossy().as_bytes())
            .map_err(|_| "Ruta SoundFont inválida para CString")?;
        unsafe {
            let settings = (api.new_settings)();
            if settings.is_null() {
                return Err("new_fluid_settings devolvió NULL".into());
            }
            if (api.settings_setnum)(
                settings,
                c"synth.sample-rate".as_ptr(),
                f64::from(sample_rate),
            ) < 0
            {
                (api.delete_settings)(settings);
                return Err(format!("FluidSynth no admite {sample_rate} Hz"));
            }
            (api.settings_setint)(
                settings,
                c"synth.polyphony".as_ptr(),
                polyphony.clamp(8, MAX_VOICES) as c_int,
            );
            (api.settings_setint)(
                settings,
                c"synth.reverb.active".as_ptr(),
                i32::from(effects),
            );
            (api.settings_setint)(
                settings,
                c"synth.chorus.active".as_ptr(),
                i32::from(effects),
            );

            let synth = (api.new_synth)(settings);
            if synth.is_null() {
                (api.delete_settings)(settings);
                return Err("new_fluid_synth devolvió NULL".into());
            }
            let sf_id = (api.sfload)(synth, c_path.as_ptr(), 0);
            if sf_id < 0 {
                (api.delete_synth)(synth);
                (api.delete_settings)(settings);
                return Err(format!("FluidSynth no pudo cargar {}", sf2_path.display()));
            }
            (api.bank_select)(synth, 0, 0);
            (api.program_change)(synth, 0, 0);
            (api.set_gain)(synth, 0.7);
            Ok(Self {
                api,
                settings,
                synth,
                rpn_msb: 127,
                rpn_lsb: 127,
                data_msb: 0,
            })
        }
    }

    pub fn note_on(&mut self, channel: i32, note: i32, velocity: i32) {
        unsafe {
            (self.api.noteon)(self.synth, channel, note, velocity);
        }
    }

    pub fn note_off(&mut self, channel: i32, note: i32) {
        unsafe {
            (self.api.noteoff)(self.synth, channel, note);
        }
    }

    pub fn note_off_all(&mut self, channel: i32) {
        unsafe {
            (self.api.all_notes_off)(self.synth, channel);
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        unsafe {
            (self.api.set_gain)(self.synth, volume.clamp(0.0, 5.0));
        }
    }

    pub fn set_polyphony(&mut self, polyphony: usize) {
        unsafe {
            (self.api.set_polyphony)(self.synth, polyphony.clamp(8, MAX_VOICES) as c_int);
        }
    }

    pub fn set_effects(&mut self, enabled: bool) {
        let on = c_int::from(enabled);
        unsafe {
            (self.api.reverb_on)(self.synth, -1, on);
            (self.api.chorus_on)(self.synth, -1, on);
        }
    }

    pub fn process_midi(&mut self, channel: i32, command: u8, data1: u8, data2: u8) {
        unsafe {
            match command & 0xF0 {
                0xB0 => match data1 {
                    0 => {
                        (self.api.bank_select)(self.synth, channel, c_uint::from(data2));
                    }
                    101 => self.rpn_msb = data2,
                    100 => self.rpn_lsb = data2,
                    6 => {
                        self.data_msb = data2;
                        self.apply_rpn(channel, data2, 0);
                    }
                    38 => {
                        self.apply_rpn(channel, self.data_msb, data2);
                    }
                    _ => {
                        (self.api.cc)(self.synth, channel, c_int::from(data1), c_int::from(data2));
                    }
                },
                0xD0 => {
                    (self.api.channel_pressure)(self.synth, channel, c_int::from(data1));
                }
                0xC0 => {
                    (self.api.program_change)(self.synth, channel, c_int::from(data1));
                }
                0xE0 => {
                    let raw = (c_int::from(data2) << 7) | c_int::from(data1);
                    (self.api.pitch_bend)(self.synth, channel, raw.clamp(0, 16383));
                }
                _ => {}
            }
        }
    }

    unsafe fn apply_rpn(&mut self, channel: i32, coarse: u8, fine: u8) {
        if self.rpn_msb != 0 {
            return;
        }
        unsafe {
            match self.rpn_lsb {
                0 => {
                    (self.api.pitch_wheel_sens)(self.synth, channel, c_int::from(coarse));
                }
                1 => {
                    let raw = ((i32::from(coarse) << 7) | i32::from(fine)) - 8192;
                    let cents = (raw as f32 / 81.92).clamp(-100.0, 100.0);
                    // GEN_FINETUNE = 51 en SoundFont 2.01
                    (self.api.set_gen)(self.synth, channel, 51, cents);
                }
                2 => {
                    let semitones = (f32::from(coarse) - 64.0).clamp(-12.0, 12.0);
                    // GEN_COARSETUNE = 52 en SoundFont 2.01
                    (self.api.set_gen)(self.synth, channel, 52, semitones);
                }
                _ => {}
            }
        }
    }

    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len()) as c_int;
        if len <= 0 {
            return;
        }
        unsafe {
            (self.api.write_float)(
                self.synth,
                len,
                left.as_mut_ptr() as *mut c_void,
                0,
                1,
                right.as_mut_ptr() as *mut c_void,
                0,
                1,
            );
        }
    }
}

impl Drop for FluidEngine {
    fn drop(&mut self) {
        unsafe {
            if !self.synth.is_null() {
                (self.api.delete_synth)(self.synth);
            }
            if !self.settings.is_null() {
                (self.api.delete_settings)(self.settings);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluidsynth_loads_dynamically_if_present_and_renders_sf2() {
        if !is_available() {
            return;
        }
        let banks = super::super::soundfont::discover();
        let Some(first) = banks.first() else {
            return;
        };
        let mut engine =
            FluidEngine::new(&first.path, 48_000, 64, true).expect("FluidEngine debe iniciar");
        engine.note_on(0, 60, 100);
        let mut left = [0.0_f32; 256];
        let mut right = [0.0_f32; 256];
        engine.render(&mut left, &mut right);
        let energy: f32 = left.iter().chain(right.iter()).map(|s| s.abs()).sum();
        assert!(energy > 0.0, "FluidSynth debe sintetizar audio audible");
    }
}
