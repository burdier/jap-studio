//! Estado de ejecución del piano: voces, notas sostenidas y ratón.
//! No importa GPUI. La vista consulta este objeto; el audio vive en `audio/piano`.
use crate::audio::piano::PianoPlayer;
use elevate::{
    catalog::{self, CatalogEntry},
    musical::model::InstrumentId,
};
use std::collections::HashSet;

const ENTRY_ID: &str = "instrument.piano";
const WHITE_NOTES: [u8; 14] = [60, 62, 64, 65, 67, 69, 71, 72, 74, 76, 77, 79, 81, 83];
const BLACK_NOTES: [u8; 10] = [61, 63, 66, 68, 70, 73, 75, 78, 80, 82];
const KEYBOARD: [(&str, u8); 13] = [
    ("a", 60),
    ("w", 61),
    ("s", 62),
    ("e", 63),
    ("d", 64),
    ("f", 65),
    ("t", 66),
    ("g", 67),
    ("y", 68),
    ("h", 69),
    ("u", 70),
    ("j", 71),
    ("k", 72),
];
const KEYBOARD_HINT: &str =
    "Teclado: a w s e d f t g y h u j k = C4…C5 · click: nota · Space: liberar";

/// Piano único del prototipo. El descriptor viene del catálogo compartido.
/// Si se añaden más instrumentos, este tipo podrá pasar a un enum o trait sin
/// que la vista conozca el motor de audio.
pub(crate) struct Piano {
    player: Option<PianoPlayer>,
    held: HashSet<u8>,
    mouse: Option<u8>,
}

impl Piano {
    pub fn new() -> Self {
        Self {
            player: None,
            held: HashSet::new(),
            mouse: None,
        }
    }

    /// Identidad estable de la única instancia del prototipo.
    pub fn id(&self) -> InstrumentId {
        InstrumentId(1)
    }

    pub fn descriptor(&self) -> &'static CatalogEntry {
        catalog::find(ENTRY_ID).expect("el catálogo debe declarar instrument.piano")
    }

    pub fn keyboard_hint(&self) -> &'static str {
        KEYBOARD_HINT
    }

    pub fn white_notes(&self) -> &'static [u8] {
        &WHITE_NOTES
    }

    pub fn black_notes(&self) -> &'static [u8] {
        &BLACK_NOTES
    }

    pub fn keyboard_note(&self, key: &str) -> Option<u8> {
        KEYBOARD
            .iter()
            .find_map(|(mapped, note)| (*mapped == key).then_some(*note))
    }

    pub fn held(&self, note: u8) -> bool {
        self.held.contains(&note)
    }

    pub fn set_mouse_note(&mut self, note: Option<u8>) {
        self.mouse = note;
    }

    pub fn take_mouse_note(&mut self) -> Option<u8> {
        self.mouse.take()
    }

    pub fn is_running(&self) -> bool {
        self.player.is_some()
    }

    pub fn sample_rate(&self) -> Option<u32> {
        self.player.as_ref().map(|player| player.sample_rate)
    }

    /// Abre el dispositivo en la primera nota. Devuelve si la orden se aceptó.
    pub fn note_on(
        &mut self,
        note: u8,
        velocity: u8,
        gain: f32,
        bypass: bool,
    ) -> Result<bool, String> {
        if note > 127 || self.held.contains(&note) {
            return Ok(false);
        }
        if self.player.is_none() {
            self.player = Some(PianoPlayer::start(gain, bypass)?);
        }
        if let Some(player) = &self.player {
            player.note_on(note, velocity);
        }
        self.held.insert(note);
        Ok(true)
    }

    pub fn note_off(&mut self, note: u8) -> bool {
        if !self.held.remove(&note) {
            return false;
        }
        if let Some(player) = &self.player {
            player.note_off(note);
        }
        true
    }

    pub fn all_notes_off(&mut self) -> bool {
        self.mouse = None;
        if self.held.is_empty() {
            return false;
        }
        if let Some(player) = &self.player {
            player.all_notes_off();
        }
        self.held.clear();
        true
    }

    pub fn stop(&mut self) {
        self.all_notes_off();
        self.player = None;
        self.held.clear();
        self.mouse = None;
    }

    pub fn set_gain(&self, gain: f32) {
        if let Some(player) = &self.player {
            player.set_gain(gain);
        }
    }

    pub fn set_bypass(&self, bypass: bool) {
        if let Some(player) = &self.player {
            player.set_bypass(bypass);
        }
    }

    pub fn failed(&self) -> bool {
        self.player.as_ref().is_some_and(PianoPlayer::failed)
    }
}

/// Carga el SoundFont fuera del callback antes del primer note-on.
pub(crate) fn preload() -> Result<(), String> {
    crate::audio::piano::preload()
}
