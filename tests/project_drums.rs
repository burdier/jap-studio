use elevate::{
    devices::sampler::{PlayMode, SUSTAIN, TUNE},
    musical::{
        drums::{DrumPlaybackSource, DrumSession, DrumSongCell},
        grid::NoteGrid,
        note_expression::NoteProperty,
        time::{Meter, PPQ, Tick},
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
    let kick = drums.kit.add_sample("Audio/Kick.wav".into()).unwrap();
    let piece = &mut drums.kit.pieces[0];
    piece.sampler.set(TUNE, -2.0);
    piece.layers[0].velocity_min = 32;
    piece.layers[0].velocity_max = 100;
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
    let kick = drums.kit.add_sample("Audio/Kick.wav".into()).unwrap();
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

#[test]
fn custom_drum_pattern_and_sample_survive_reopen() {
    let mut drums = DrumSession::default();
    assert!(drums.kit.pieces.is_empty());
    let empty = ProjectDoc::new("Sin muestras").unwrap();
    empty.set_drum_session(&drums).unwrap();
    let reopened = ProjectDoc::import(&empty.snapshot().unwrap()).unwrap();
    assert_eq!(reopened.drum_session().unwrap().unwrap(), drums);

    let original = drums.active;
    let piece = drums.kit.add_sample("Audio/Rim seco.wav".into()).unwrap();
    let sampler = &drums.kit.pieces[0].sampler;
    assert_eq!(sampler.mode, PlayMode::OneShot);
    assert_eq!(sampler.get(SUSTAIN), 100.0);
    assert!(sampler.choke);

    let created = drums.new_pattern().unwrap();
    assert_ne!(created, original);
    let six_eight = Meter::new(6, 8).unwrap();
    drums.set_active_meter(six_eight).unwrap();
    drums.active_pattern_mut().grid = NoteGrid::new(8).unwrap();
    assert_eq!(drums.active_pattern().notes.length(), Tick(3 * PPQ));
    assert_eq!(drums.active_pattern().step_count(), 6);

    let notes = &mut drums.active_pattern_mut().notes;
    notes
        .add_note(piece, Tick(2 * PPQ), Tick(PPQ / 2), 36, 87)
        .unwrap();
    assert!(drums.set_active_meter(Meter::new(2, 4).unwrap()).is_err());
    assert_eq!(drums.active_pattern().meter, six_eight);
    drums.active_pattern_mut().grid = NoteGrid::new(16).unwrap();
    assert_eq!(drums.active_pattern().step_count(), 12);

    let doc = ProjectDoc::new("Rim en 6/8").unwrap();
    doc.set_drum_session(&drums).unwrap();
    let reopened = ProjectDoc::import(&doc.snapshot().unwrap()).unwrap();
    let restored = reopened.drum_session().unwrap().unwrap();
    assert_eq!(restored, drums);
    assert_eq!(restored.pattern(original).unwrap().step_count(), 16);
    assert_eq!(
        restored.active_pattern().notes.notes()[0].start,
        Tick(2 * PPQ)
    );
    assert_eq!(restored.kit.pieces.last().unwrap().name, "Rim seco");
}
