use super::{
    SamplerSnapshot,
    native::{Api, Synth},
    sfz,
};
use elevate::devices::sampler::VOICES;
use std::{ffi::CString, sync::Arc};

pub(crate) struct SfizzEngine {
    api: Arc<Api>,
    synth: *mut Synth,
    held: [bool; 128],
    // Mantiene el WAV temporal hasta después de liberar el motor y sus lectores.
    _snapshot: SamplerSnapshot,
}
// Se transfiere a un único hilo; nunca comparte el synth ni permite llamadas simultáneas.
unsafe impl Send for SfizzEngine {}
impl SfizzEngine {
    pub fn prepare(snapshot: SamplerSnapshot, rate: u32) -> Result<Self, String> {
        let api = Api::open()?;
        let text = CString::new(sfz::instrument(&snapshot)).map_err(|e| e.to_string())?;
        let path = CString::new(
            snapshot
                .asset
                .wave
                .with_extension("sfz")
                .to_string_lossy()
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        let synth = unsafe { (api.create)() };
        if synth.is_null() {
            return Err("sfizz no pudo crear el sampler".into());
        }
        let mut result = Self {
            api,
            synth,
            held: [false; 128],
            _snapshot: snapshot,
        };
        unsafe {
            (result.api.block)(synth, 512);
            (result.api.rate)(synth, rate as f32);
            (result.api.voices)(synth, result._snapshot.params.get(VOICES) as i32);
            if !(result.api.load)(synth, path.as_ptr(), text.as_ptr()) {
                return Err("sfizz no pudo cargar el sample".into());
            }
        }
        let params = result._snapshot.params.clone();
        result.parameters(&params);
        Ok(result)
    }
    /// Configuración fuera de RT, antes de transferir el motor al stream.
    pub fn set_sample_rate(&mut self, rate: u32) {
        unsafe {
            (self.api.rate)(self.synth, rate as f32);
        }
    }
    pub fn control(&mut self, cc: u8, value: f32) {
        unsafe {
            (self.api.hdcc)(self.synth, 0, cc as i32, value.clamp(0., 1.));
        }
    }
    pub fn parameters(&mut self, params: &elevate::devices::sampler::SamplerParams) {
        for id in 0..elevate::devices::sampler::PARAMS.len() {
            if let Some((cc, v)) = elevate::devices::sampler::live_control(params, id) {
                self.control(cc, v);
            }
        }
    }
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        if note > 127 {
            return;
        }
        self.held[note as usize] = true;
        unsafe {
            (self.api.on)(self.synth, 0, note as i32, velocity as i32);
        }
    }
    pub fn note_off(&mut self, note: u8) {
        if note > 127 {
            return;
        }
        self.held[note as usize] = false;
        unsafe {
            (self.api.off)(self.synth, 0, note as i32, 0);
        }
    }
    pub fn all_off(&mut self) {
        self.midi(0xb0, 64, 0);
        for note in 0..128 {
            if self.held[note] {
                self.note_off(note as u8);
            }
        }
    }
    pub fn panic(&mut self) {
        self.midi(0xb0, 64, 0);
        self.midi(0xb0, 123, 0);
        self.held.fill(false);
    }
    pub fn midi(&mut self, command: u8, a: u8, b: u8) {
        unsafe {
            match command & 0xf0 {
                0xb0 => (self.api.cc)(self.synth, 0, a as i32, b as i32),
                0xe0 => (self.api.bend)(self.synth, 0, ((b as i32) << 7 | a as i32) - 8192),
                _ => {}
            }
        }
    }
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        debug_assert_eq!(left.len(), right.len());
        for (l, r) in left.chunks_mut(512).zip(right.chunks_mut(512)) {
            let mut channels = [l.as_mut_ptr(), r.as_mut_ptr()];
            unsafe {
                (self.api.render)(self.synth, channels.as_mut_ptr(), 2, l.len() as i32);
            }
        }
    }
}
impl Drop for SfizzEngine {
    fn drop(&mut self) {
        unsafe {
            (self.api.free)(self.synth);
        }
    }
}
