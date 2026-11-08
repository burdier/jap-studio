//! Controlador «teclado del ordenador como MIDI» (estilo Ableton).
//! Convierte teclas en `MidiMessage`; no conoce GPUI, audio ni a qué instrumento llega.
//! Otro controlador (MIDI físico, pad XY) producirá los mismos mensajes.
use super::{midi::MidiMessage, pitch::note_name};
use std::collections::HashMap;

pub const TOGGLE_KEY: &str = "m";
const OCTAVE_DOWN_KEY: &str = "z";
const OCTAVE_UP_KEY: &str = "x";
const VELOCITY_DOWN_KEY: &str = "c";
const VELOCITY_UP_KEY: &str = "v";
const SUSTAIN_TOGGLE_KEY: &str = "/";
const BEND_DOWN_KEY: &str = "-";
const BEND_UP_KEY: &str = "=";
const MOD_DOWN_KEY: &str = ",";
const MOD_UP_KEY: &str = ".";
const MACRO_CC_KEY: &str = "'";
/// Teclas de una octava y media a partir de la nota base: a w s e d f t g y h u j k o l p ;
const NOTE_KEYS: [&str; 17] = [
    "a", "w", "s", "e", "d", "f", "t", "g", "y", "h", "u", "j", "k", "o", "l", "p", ";",
];
const BASE_NOTE: i16 = 60;
const MAX_OCTAVE_SHIFT: i8 = 4;
const VELOCITY_STEP: u8 = 20;
const MOD_STEP: u8 = 16;
pub const DEFAULT_VELOCITY: u8 = 100;

pub const HINT: &str =
    "M: MIDI · a..; notas · Z/X oct · C/V vel · / sustain · -/= bend · ,/. mod · ' macro CC21";

#[derive(Debug)]
pub struct ComputerKeyboard {
    enabled: bool,
    octave_shift: i8,
    velocity: u8,
    sustain: bool,
    modulation: u8,
    macro_cc: u8,
    /// Tecla física → nota que disparó, para soltar la correcta aunque cambie la octava.
    held: HashMap<String, u8>,
}

impl Default for ComputerKeyboard {
    fn default() -> Self {
        Self {
            enabled: false,
            octave_shift: 0,
            velocity: DEFAULT_VELOCITY,
            sustain: false,
            modulation: 0,
            macro_cc: 64,
            held: HashMap::new(),
        }
    }
}

impl ComputerKeyboard {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn velocity(&self) -> u8 {
        self.velocity
    }

    pub fn sustain(&self) -> bool {
        self.sustain
    }

    pub fn modulation(&self) -> u8 {
        self.modulation
    }

    /// Número de octava de la nota más grave (`4` para C4).
    pub fn octave(&self) -> i16 {
        i16::from(self.note_for(0).unwrap_or(0)) / 12 - 1
    }

    /// Nota más grave del teclado, p. ej. `C4`.
    pub fn base_label(&self) -> String {
        note_name(self.note_for(0).unwrap_or(0))
    }

    /// Activa o desactiva el controlador. Al apagar devuelve los NoteOff pendientes.
    pub fn set_enabled(&mut self, enabled: bool) -> Vec<MidiMessage> {
        self.enabled = enabled;
        if enabled {
            Vec::new()
        } else {
            self.release_all()
        }
    }

    pub fn release_all(&mut self) -> Vec<MidiMessage> {
        self.held
            .drain()
            .map(|(_, note)| MidiMessage::NoteOff { note })
            .collect()
    }

    /// `None`: la tecla no es del controlador y debe seguir su camino normal.
    /// `Some(mensajes)`: la tecla se consumió (puede no producir mensajes).
    pub fn key_down(&mut self, key: &str, repeat: bool) -> Option<Vec<MidiMessage>> {
        if key == TOGGLE_KEY {
            return Some(if repeat {
                Vec::new()
            } else {
                self.set_enabled(!self.enabled)
            });
        }
        if !self.enabled {
            return None;
        }
        if let Some(index) = NOTE_KEYS.iter().position(|mapped| *mapped == key) {
            let note = self.note_for(index as i16);
            return Some(match note {
                Some(note) if !repeat && !self.held.contains_key(key) => {
                    self.held.insert(key.to_string(), note);
                    vec![MidiMessage::NoteOn {
                        note,
                        velocity: self.velocity,
                    }]
                }
                _ => Vec::new(),
            });
        }
        match key {
            OCTAVE_DOWN_KEY => self.shift_octave(-1),
            OCTAVE_UP_KEY => self.shift_octave(1),
            VELOCITY_DOWN_KEY => self.velocity = self.velocity.saturating_sub(VELOCITY_STEP).max(1),
            VELOCITY_UP_KEY => self.velocity = (self.velocity + VELOCITY_STEP).min(127),
            SUSTAIN_TOGGLE_KEY if !repeat => {
                self.sustain = !self.sustain;
                return Some(vec![MidiMessage::ControlChange {
                    cc: 64,
                    value: if self.sustain { 127 } else { 0 },
                }]);
            }
            BEND_DOWN_KEY if !repeat => {
                return Some(vec![MidiMessage::PitchBend { bend: -4096 }]);
            }
            BEND_UP_KEY if !repeat => {
                return Some(vec![MidiMessage::PitchBend { bend: 4096 }]);
            }
            MOD_DOWN_KEY => {
                self.modulation = self.modulation.saturating_sub(MOD_STEP);
                return Some(vec![MidiMessage::ControlChange {
                    cc: 1,
                    value: self.modulation,
                }]);
            }
            MOD_UP_KEY => {
                self.modulation = (self.modulation + MOD_STEP).min(127);
                return Some(vec![MidiMessage::ControlChange {
                    cc: 1,
                    value: self.modulation,
                }]);
            }
            MACRO_CC_KEY => {
                self.macro_cc = (self.macro_cc + 16) % 128;
                return Some(vec![MidiMessage::ControlChange {
                    cc: 21,
                    value: self.macro_cc,
                }]);
            }
            SUSTAIN_TOGGLE_KEY | BEND_DOWN_KEY | BEND_UP_KEY => {}
            _ => return None,
        }
        Some(Vec::new())
    }

    /// Suelta la nota de la tecla si estaba sonando (o centra el pitch bend).
    pub fn key_up(&mut self, key: &str) -> Option<MidiMessage> {
        if self.enabled && (key == BEND_DOWN_KEY || key == BEND_UP_KEY) {
            return Some(MidiMessage::PitchBend { bend: 0 });
        }
        self.held
            .remove(key)
            .map(|note| MidiMessage::NoteOff { note })
    }

    fn shift_octave(&mut self, delta: i8) {
        self.octave_shift = (self.octave_shift + delta).clamp(-MAX_OCTAVE_SHIFT, MAX_OCTAVE_SHIFT);
    }

    fn note_for(&self, offset: i16) -> Option<u8> {
        let note = BASE_NOTE + i16::from(self.octave_shift) * 12 + offset;
        u8::try_from(note).ok().filter(|note| *note <= 127)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(note: u8) -> Vec<MidiMessage> {
        vec![MidiMessage::NoteOn {
            note,
            velocity: DEFAULT_VELOCITY,
        }]
    }

    #[test]
    fn disabled_controller_ignores_everything_except_the_toggle_key() {
        let mut kb = ComputerKeyboard::default();
        assert_eq!(kb.key_down("a", false), None);
        assert_eq!(kb.key_down("z", false), None);
        assert_eq!(kb.key_down("m", false), Some(vec![]));
        assert!(kb.enabled());
        assert_eq!(kb.key_down("a", false), Some(on(60)));
    }

    #[test]
    fn octave_keys_shift_pitch_and_a_held_key_releases_the_note_it_started() {
        let mut kb = ComputerKeyboard::default();
        kb.set_enabled(true);
        assert_eq!(kb.key_down("a", false), Some(on(60)));
        kb.key_down("x", false);
        assert_eq!(kb.key_down("s", false), Some(on(74)));
        assert_eq!(kb.key_up("a"), Some(MidiMessage::NoteOff { note: 60 }));
        kb.key_down("z", false);
        kb.key_down("z", false);
        assert_eq!(kb.key_down("d", false), Some(on(52)));
        assert_eq!(kb.base_label(), "C3");
        assert_eq!(kb.octave(), 3);
    }

    #[test]
    fn repeats_do_not_retrigger_and_disabling_releases_held_notes() {
        let mut kb = ComputerKeyboard::default();
        kb.set_enabled(true);
        kb.key_down("a", false);
        assert_eq!(kb.key_down("a", true), Some(vec![]));
        assert_eq!(
            kb.set_enabled(false),
            vec![MidiMessage::NoteOff { note: 60 }]
        );
        assert_eq!(kb.key_up("a"), None);
    }

    #[test]
    fn octave_and_velocity_stay_inside_their_limits() {
        let mut kb = ComputerKeyboard::default();
        kb.set_enabled(true);
        for _ in 0..20 {
            kb.key_down("x", false);
            kb.key_down("v", false);
        }
        assert_eq!(kb.velocity(), 127);
        assert_eq!(kb.key_down("a", false).unwrap().len(), 1);
        assert_eq!(kb.base_label(), "C8");
        for _ in 0..20 {
            kb.key_down("c", false);
        }
        assert_eq!(kb.velocity(), 1);
    }
}
