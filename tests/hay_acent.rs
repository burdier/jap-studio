use elevate::{
    musical::{
        clip::MidiClip,
        model::{InstrumentId, NoteId, Pattern},
        time::{PPQ, Tick},
    },
    scripting::{actions::MidiOptions, edit_midi},
};

const PULSE: &str = include_str!("../assets/scripts/midi/hay_acent/pulso.lua");
const ALTERNATE: &str = include_str!("../assets/scripts/midi/hay_acent/alternado.lua");
const GROW: &str = include_str!("../assets/scripts/midi/hay_acent/creciente.lua");
const CLICK: &str = include_str!("../assets/scripts/midi/hay_acent/clic_4_4.lua");

fn note(pattern: &mut Pattern, start: i64, pitch: u8) -> NoteId {
    pattern
        .add_note(InstrumentId(7), Tick(start), Tick(60), pitch, 90)
        .unwrap()
}

#[test]
fn accents_follow_time_and_group_chords_without_changing_other_data() {
    let mut pattern = Pattern::new(Tick(i64::from(PPQ) * 8)).unwrap();
    // ID order differs from musical order; one note is not selected.
    let late = note(&mut pattern, 2000, 64);
    let first = note(&mut pattern, 100, 60);
    let chord = note(&mut pattern, 100, 67);
    let second = note(&mut pattern, 1000, 62);
    let fourth = note(&mut pattern, 3000, 65);
    let fifth = note(&mut pattern, 4000, 69);
    let untouched = note(&mut pattern, 500, 72);
    let pattern = edit_midi(
        "controls[1] = {start=0, lane='cc1', value=40}",
        &pattern,
        &[],
        MidiOptions::default(),
    )
    .unwrap()
    .pattern;
    let selected = [first, chord, second, late, fourth, fifth];

    for (source, velocities) in [
        (PULSE, [112, 112, 72, 72, 72, 112]),
        (ALTERNATE, [112, 112, 72, 112, 72, 112]),
        (GROW, [48, 48, 64, 80, 96, 112]),
    ] {
        let mut expected = pattern.notes().to_vec();
        for (id, velocity) in selected.into_iter().zip(velocities) {
            expected.iter_mut().find(|n| n.id == id).unwrap().velocity = velocity;
        }
        let edit = edit_midi(source, &pattern, &selected, MidiOptions::default()).unwrap();
        assert_eq!(edit.pattern.notes(), expected);
        assert_eq!(edit.pattern.controls(), pattern.controls());
        assert_eq!(edit.pattern.next_id(), pattern.next_id());
        let mut clip = MidiClip::new("acento", 2, 8.0, pattern.clone());
        clip.muted_notes.insert(first);
        let mut selection = selected.to_vec();
        edit.apply(&mut clip, &mut selection);
        assert_eq!(selection, selected);
        assert!(clip.muted_notes.contains(&first));
        assert_eq!(
            clip.notes()
                .iter()
                .find(|n| n.id == untouched)
                .unwrap()
                .velocity,
            90
        );
    }
}

#[test]
fn accents_accept_empty_clips_and_implicit_selection() {
    let empty = Pattern::new(Tick(i64::from(PPQ) * 4)).unwrap();
    for source in [PULSE, ALTERNATE, GROW] {
        assert_eq!(
            edit_midi(source, &empty, &[], MidiOptions::default())
                .unwrap()
                .pattern,
            empty
        );
        let mut single = empty.clone();
        note(&mut single, 100, 60);
        let edit = edit_midi(source, &single, &[], MidiOptions::default()).unwrap();
        assert_eq!(edit.pattern.notes()[0].velocity, 112);
    }
}

#[test]
fn click_places_four_beats_in_the_containing_bar_and_is_repeatable() {
    let beat = i64::from(PPQ);
    let bar = beat * 4;
    let mut pattern = Pattern::new(Tick(bar * 3)).unwrap();
    let ids = [
        note(&mut pattern, bar + 10, 72),
        note(&mut pattern, bar + 30, 73),
        note(&mut pattern, bar + 50, 74),
        note(&mut pattern, bar + 70, 75),
    ];
    note(&mut pattern, 0, 60);
    let mut expected = pattern.notes().to_vec();
    for (index, id) in ids.iter().enumerate() {
        let row = expected.iter_mut().find(|row| row.id == *id).unwrap();
        row.start = Tick(bar + index as i64 * beat);
        row.duration = Tick(beat / 8);
        row.velocity = if index == 0 { 112 } else { 72 };
    }
    let edit = edit_midi(CLICK, &pattern, &ids, MidiOptions::default()).unwrap();
    assert_eq!(edit.pattern.notes(), expected);
    let repeated = edit_midi(CLICK, &edit.pattern, &ids, MidiOptions::default()).unwrap();
    assert_eq!(repeated.pattern, edit.pattern);
}

#[test]
fn click_rejects_wrong_counts_and_a_bar_that_does_not_fit() {
    let mut pattern = Pattern::new(Tick(i64::from(PPQ) * 3)).unwrap();
    let ids = [
        note(&mut pattern, 0, 72),
        note(&mut pattern, 100, 72),
        note(&mut pattern, 200, 72),
        note(&mut pattern, 300, 72),
    ];
    let before = pattern.clone();
    for selection in [&ids[..3], &ids[..]] {
        assert!(edit_midi(CLICK, &pattern, selection, MidiOptions::default()).is_err());
        assert_eq!(pattern, before);
    }
}
