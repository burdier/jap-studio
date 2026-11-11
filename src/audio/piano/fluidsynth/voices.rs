//! Pan relativo por nota: modifica solo las voces nacidas de ese NoteOn.
//! API: https://www.fluidsynth.org/api/group__voice__control.html
//! Los punteros no sobreviven a una iteración de síntesis; buffers acotados, sin heap.
use super::{FluidEngine, MAX_VOICES};
use std::ptr::null_mut;

impl FluidEngine {
    pub fn note_on_pan(&mut self, channel: i32, note: i32, velocity: i32, pan: i8) {
        if pan == 0 {
            self.note_on(channel, note, velocity);
            return;
        }
        let mut voices = [null_mut(); MAX_VOICES];
        let mut previous_ids = [0_u32; MAX_VOICES];
        // El host posee el synth en este hilo y no renderiza entre las dos lecturas.
        unsafe {
            (self.api.get_voicelist)(self.synth, voices.as_mut_ptr(), MAX_VOICES as i32, -1);
            let count = voices.iter().take_while(|voice| !voice.is_null()).count();
            for (id, &voice) in previous_ids.iter_mut().zip(&voices[..count]) {
                *id = (self.api.voice_get_id)(voice);
            }
            previous_ids[..count].sort_unstable();
            self.note_on(channel, note, velocity);
            voices.fill(null_mut());
            (self.api.get_voicelist)(self.synth, voices.as_mut_ptr(), MAX_VOICES as i32, -1);
            let offset = f32::from(pan) * 500.0 / if pan < 0 { 64.0 } else { 63.0 };
            // GEN_PAN = 17 en SoundFont 2.01. Conservar la imagen del preset:
            // sumar el desplazamiento a cada capa, sin reemplazar su pan original.
            for &voice in voices.iter().take_while(|voice| !voice.is_null()) {
                let id = (self.api.voice_get_id)(voice);
                if previous_ids[..count].binary_search(&id).is_err() {
                    (self.api.voice_gen_incr)(voice, 17, offset);
                    (self.api.voice_update_param)(voice, 17);
                }
            }
        }
    }
}
