//! Mapeo de teclado de computadora. No forma parte del instrumento de audio.
pub(super) fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(note % 12)],
        i16::from(note / 12) - 1
    )
}
pub(super) fn keyboard_note(key: &str) -> Option<u8> {
    Some(match key {
        "a" => 60,
        "w" => 61,
        "s" => 62,
        "e" => 63,
        "d" => 64,
        "f" => 65,
        "t" => 66,
        "g" => 67,
        "y" => 68,
        "h" => 69,
        "u" => 70,
        "j" => 71,
        "k" => 72,
        _ => return None,
    })
}
