use super::{
    SampleAsset, SamplerSnapshot,
    native::{Api, Synth},
    sfz,
};
use elevate::devices::sampler::{PARAMS, SamplerParams, VOICES, live_control};
use std::{ffi::CString, sync::Arc};

pub(crate) struct SfizzEngine {
    api: Arc<Api>,
    synth: *mut Synth,
    held: [bool; 128],
    // Mantiene el WAV temporal hasta después de liberar el motor y sus lectores.
    _assets: Vec<Arc<SampleAsset>>,
    params: SamplerParams,
}
// Se transfiere a un único hilo; nunca comparte el synth ni permite llamadas simultáneas.
unsafe impl Send for SfizzEngine {}
impl SfizzEngine {
    pub fn prepare(snapshot: SamplerSnapshot, rate: u32) -> Result<Self, String> {
        Self::prepare_layers(vec![(snapshot.asset, 1, 127)], snapshot.params, rate)
    }

    pub fn prepare_layers(
        layers: Vec<(Arc<SampleAsset>, u8, u8)>,
        params: SamplerParams,
        rate: u32,
    ) -> Result<Self, String> {
        let api = Api::open()?;
        let first = layers
            .first()
            .ok_or("Un instrumento necesita una muestra")?;
        let path = CString::new(
            first
                .0
                .wave
                .with_extension("sfz")
                .to_string_lossy()
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        let text = layers
            .iter()
            .map(|(asset, lo, hi)| sfz::region(asset, &params, (*lo, *hi)))
            .collect::<Vec<_>>()
            .join("\n");
        let text = CString::new(text).map_err(|e| e.to_string())?;
        let synth = unsafe { (api.create)() };
        if synth.is_null() {
            return Err("sfizz no pudo crear el sampler".into());
        }
        let mut result = Self {
            api,
            synth,
            held: [false; 128],
            _assets: layers.into_iter().map(|(asset, _, _)| asset).collect(),
            params,
        };
        unsafe {
            (result.api.block)(synth, 512);
            (result.api.rate)(synth, rate as f32);
            (result.api.voices)(synth, result.params.get(VOICES) as i32);
            if !(result.api.load)(synth, path.as_ptr(), text.as_ptr()) {
                return Err("sfizz no pudo cargar el sample".into());
            }
        }
        let params = result.params.clone();
        result.parameters(&params);
        Ok(result)
    }
    /// Configuración fuera de RT, antes de transferir el motor al stream.
    pub fn set_sample_rate(&mut self, rate: u32) {
        unsafe {
            (self.api.rate)(self.synth, rate as f32);
        }
        let params = self.params.clone();
        self.parameters(&params);
    }
    pub fn control(&mut self, cc: u8, value: f32) {
        unsafe {
            (self.api.hdcc)(self.synth, 0, cc as i32, value.clamp(0., 1.));
        }
    }
    pub fn parameters(&mut self, params: &SamplerParams) {
        self.params = params.clone();
        for id in 0..PARAMS.len() {
            if let Some((cc, v)) = live_control(params, id) {
                self.control(cc, v);
            }
        }
    }
    pub fn update_live(&mut self, params: &SamplerParams) {
        for id in 0..PARAMS.len() {
            if self.params.get(id) != params.get(id)
                && let Some((cc, value)) = live_control(params, id)
            {
                self.control(cc, value);
            }
        }
        self.params = params.clone();
    }

    pub fn cut(&mut self) {
        unsafe { (self.api.cut)(self.synth) };
        self.held.fill(false);
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
        self.note_off_velocity(note, 64);
    }
    pub fn note_off_velocity(&mut self, note: u8, release_velocity: u8) {
        if note > 127 {
            return;
        }
        self.held[note as usize] = false;
        unsafe {
            (self.api.off)(
                self.synth,
                0,
                note as i32,
                i32::from(release_velocity.min(127)),
            );
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
                0xd0 => (self.api.pressure)(self.synth, 0, a as i32),
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
