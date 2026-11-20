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

#[test]
fn linked_drum_clip_refreshes_but_its_local_copy_keeps_note_edits() {
    use elevate::musical::{
        clip::MidiClip,
        drums::{DrumClipTarget, arrangement_pattern},
        track::DEMO_TRACKS,
    };
    let mut drums = DrumSession::default();
    let kick = drums.kit.pieces[0].id;
    let notes = &mut drums.active_pattern_mut().notes;
    notes
        .add_note(kick, Tick(PPQ / 2), Tick(PPQ / 2), 36, 90)
        .unwrap();
    let late = notes
        .add_note(kick, Tick(2 * PPQ), Tick(PPQ / 2), 39, 100)
        .unwrap();
    let mut linked = MidiClip::new("Kit · Kick", 0, 3.0, notes.clone());
    linked.drum_target = Some(DrumClipTarget {
        kit: drums.kit.id,
        piece: kick,
        pattern: drums.active,
        linked: true,
    });
    let mut local = linked.duplicate();
    local.start_beat = 8.0;
    local
        .edit_pattern(|pattern| pattern.remove_note(late))
        .unwrap();
    linked.trim_start = 1.0;
    linked.trim_len = Some(2.0);
    drums
        .active_pattern_mut()
        .notes
        .set_velocity(late, 88)
        .unwrap();

    let ids = [linked.id, local.id];
    let doc = ProjectDoc::from_session("Batería", &DEMO_TRACKS, &[linked, local]).unwrap();
    doc.set_drum_session(&drums).unwrap();
    let reopened = ProjectDoc::import(&doc.snapshot().unwrap()).unwrap();
    let clips = reopened.midi_clips().unwrap();
    assert_eq!([clips[0].id, clips[1].id], ids);
    assert!(clips[0].drum_target.unwrap().linked);
    assert!(!clips[1].drum_target.unwrap().linked);
    assert_eq!(clips[0].notes()[1].velocity, 88);
    assert_eq!(clips[1].notes().len(), 1);

    let arranged = arrangement_pattern(&clips, |_| true).unwrap().unwrap();
    assert_eq!(arranged.length(), Tick(12 * PPQ));
    assert_eq!(arranged.notes()[0].start, Tick(4 * PPQ));
    assert_eq!(arranged.notes()[0].pitch, 39);
    assert_eq!(arranged.notes()[0].velocity, 88);
    assert_eq!(arranged.notes()[1].start, Tick(8 * PPQ + PPQ / 2));
}
