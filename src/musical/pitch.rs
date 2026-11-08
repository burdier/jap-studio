//! Nombres de altura MIDI para presentación. Sin audio ni widgets.
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub fn note_name(note: u8) -> String {
    format!(
        "{}{}",
        NAMES[usize::from(note % 12)],
        i16::from(note / 12) - 1
    )
}

#[cfg(test)]
mod tests {
    use super::note_name;

    #[test]
    fn midi_zero_is_c_minus_one_and_sixty_is_middle_c() {
        assert_eq!(note_name(0), "C-1");
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(69), "A4");
        assert_eq!(note_name(127), "G9");
    }
}
