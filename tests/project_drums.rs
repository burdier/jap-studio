use elevate::{
    devices::sampler::TUNE,
    musical::{
        drums::{DrumLayer, DrumPlaybackSource, DrumSession, DrumSongCell},
        note_expression::NoteProperty,
        time::{PPQ, Tick},
    },
    project::ProjectDoc,
};

#[test]
fn drum_session_round_trip_keeps_two_bars_variation_and_linked_occurrences() {
    let original = ProjectDoc::new("Dos compases").unwrap();
    let doc = ProjectDoc::import(&original.snapshot().unwrap()).unwrap();
    assert!(doc.drum_session().unwrap().is_none());

    let mut drums = DrumSession::default();
    drums.kit.bus.fader.level = 70;
    let kick = &mut drums.kit.pieces[0];
    kick.sampler.set(TUNE, -2.0);
    kick.layers.push(DrumLayer {
        source: "Audio/Kick.wav".into(),
        velocity_min: 32,
        velocity_max: 100,
    });
    let kick = kick.id;
    let first = drums.active;
    drums.resize_active(Tick(8 * PPQ)).unwrap();
    let pattern = &mut drums.active_pattern_mut().notes;
    let note = pattern
        .add_note(kick, Tick(4 * PPQ), Tick(PPQ / 4), 36, 110)
        .unwrap();
    pattern
        .set_note_property(note, NoteProperty::Probability, 80)
        .unwrap();

    let variation = drums.duplicate_active().unwrap();
    drums
        .active_pattern_mut()
        .notes
        .set_velocity(note, 42)
        .unwrap();
    for (at, pattern) in [(Tick(0), first), (Tick(8 * PPQ), variation)] {
        drums
            .set_song_cell(DrumSongCell { at, pattern }, true)
            .unwrap();
    }
    drums.source = DrumPlaybackSource::Song;
    doc.set_drum_session(&drums).unwrap();

    let reopened = ProjectDoc::import(&doc.snapshot().unwrap()).unwrap();
    let restored = reopened.drum_session().unwrap().unwrap();
    assert_eq!(restored, drums);
    assert_ne!(first, variation);
    assert_eq!(
        restored.pattern(first).unwrap().notes.notes()[0].velocity,
        110
    );
    let plan = restored.playback_pattern().unwrap().unwrap();
    assert_eq!(plan.length(), Tick(16 * PPQ));
    assert_eq!(plan.notes()[1].start, Tick(12 * PPQ));
    assert_eq!(plan.notes()[1].velocity, 42);
}
