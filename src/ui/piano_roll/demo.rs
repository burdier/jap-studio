//! Frase de demostración de la referencia visual; se prepara una vez al insertar.
use elevate::musical::{
    clip::MidiClip,
    model::{InstrumentId, Pattern},
    time::{PPQ, Tick},
};

pub(super) fn reference_clip(
    name: String,
    track: usize,
    beat: f64,
    instrument: InstrumentId,
) -> MidiClip {
    let mut pattern = Pattern::new(Tick(32 * PPQ)).expect("ocho compases válidos");
    let mut add = |start: f64, length: f64, pitch: u8, velocity: u8| {
        pattern
            .add_note(
                instrument,
                Tick((start * PPQ as f64) as i64),
                Tick((length * PPQ as f64) as i64),
                pitch,
                velocity,
            )
            .expect("nota de demostración dentro del clip");
    };
    for (start, pitch, length) in [
        (1.0, 71, 0.85),
        (2.0, 73, 0.9),
        (3.0, 74, 0.9),
        (4.0, 75, 0.85),
        (5.0, 77, 0.9),
        (6.0, 78, 1.0),
        (7.0, 75, 0.85),
        (8.0, 73, 1.9),
        (10.0, 71, 0.9),
        (11.0, 72, 0.9),
        (12.0, 70, 1.9),
        (15.0, 69, 0.9),
        (17.0, 73, 0.85),
        (18.0, 74, 0.9),
        (19.0, 75, 0.9),
        (20.0, 73, 0.85),
        (21.0, 71, 0.85),
        (22.0, 70, 0.9),
        (24.0, 69, 0.9),
        (25.0, 71, 0.85),
        (26.0, 73, 0.85),
        (27.0, 74, 0.85),
        (28.0, 75, 0.9),
        (29.0, 77, 0.85),
        (30.0, 78, 1.5),
    ] {
        let velocity = [52, 68, 66, 100, 86, 80, 60, 84][start as usize % 8];
        add(start, length, pitch, velocity);
    }
    for start in [0.0, 8.0, 16.0, 24.0] {
        add(start, 3.9, 55, 88);
    }
    for start in [4.0, 12.0, 20.0] {
        for pitch in [58, 61, 64] {
            add(start, 3.9, pitch, 100);
        }
    }
    MidiClip::new(name, track, beat, pattern)
}
