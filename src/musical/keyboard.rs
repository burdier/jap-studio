//! Geometría de un teclado MIDI: qué teclas existen en un rango y dónde empieza cada octava.
//! Reutilizable por cualquier instrumento; no conoce audio, widgets ni al piano.
use super::pitch::note_name;

/// Una tecla blanca y, si existe, la negra pegada a su derecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhiteKey {
    pub note: u8,
    pub black: Option<u8>,
}

impl WhiteKey {
    /// Etiqueta de octava (`C4`) solo en las teclas C; el resto no la lleva.
    pub fn octave_label(&self) -> Option<String> {
        (self.note % 12 == 0).then(|| note_name(self.note))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardLayout {
    keys: Vec<WhiteKey>,
}

impl KeyboardLayout {
    /// Teclas del rango `first..=last` (notas MIDI 0–127).
    pub fn new(first: u8, last: u8) -> Self {
        let last = last.min(127);
        let keys = (first..=last)
            .filter(|note| !is_black(*note))
            .map(|note| WhiteKey {
                note,
                black: (note < last && is_black(note + 1)).then_some(note + 1),
            })
            .collect();
        Self { keys }
    }

    pub fn keys(&self) -> &[WhiteKey] {
        &self.keys
    }
}

pub fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}

#[cfg(test)]
mod tests {
    use super::{KeyboardLayout, is_black};

    #[test]
    fn three_octaves_have_21_white_keys_and_15_black_ones_with_c_labels() {
        let layout = KeyboardLayout::new(48, 83);
        assert_eq!(layout.keys().len(), 21);
        assert_eq!(
            layout.keys().iter().filter(|k| k.black.is_some()).count(),
            15
        );
        let labels: Vec<_> = layout
            .keys()
            .iter()
            .filter_map(|k| k.octave_label())
            .collect();
        assert_eq!(labels, ["C3", "C4", "C5"]);
        assert!(layout.keys().iter().all(|k| !is_black(k.note)));
    }

    #[test]
    fn a_range_ending_on_a_white_key_has_no_dangling_black_key() {
        let layout = KeyboardLayout::new(60, 64);
        assert_eq!(layout.keys().last().unwrap().black, None);
    }
}
