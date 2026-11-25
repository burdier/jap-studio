use elevate::{
    musical::{
        clip::MidiClip,
        midi_map::{MidiBindTarget, MidiMapper, MidiSource},
        model::Pattern,
        time::Tick,
        track::{
            DEMO_TRACKS, TrackBatch, TrackBatchEdit, TrackRef, TrackSnapshot, apply_track_batch,
        },
    },
    project::ProjectDoc,
    scripting::tracks::build_track_batch,
};

fn origin() -> TrackSnapshot {
    TrackSnapshot::capture(&DEMO_TRACKS[..2], &[75, 65], &[50, 30], 12).unwrap()
}

#[test]
fn insertion_keeps_clip_identity_and_midi_mapping_on_the_original_track() {
    let before = origin();
    let old_track = before.entries[1].track.id;
    let batch = TrackBatch {
        origin: before.clone(),
        edits: vec![TrackBatchEdit::InsertAudio {
            handle: "inserted".into(),
            name: "Nueva".into(),
            after: Some(TrackRef::Id(before.entries[0].track.id)),
            parent: None,
        }],
    };
    let result = apply_track_batch(&before, &batch).unwrap();
    assert_eq!(result.sources, vec![Some(0), None, Some(1)]);
    assert_eq!(result.snapshot.entries[2].track.id, old_track);
    assert_eq!(
        (
            result.snapshot.entries[2].mix.level,
            result.snapshot.entries[2].mix.pan
        ),
        (65, 30)
    );

    let mut mapper = MidiMapper::new();
    mapper.bind(MidiSource::Cc(20), MidiBindTarget::TrackVolume(old_track));
    mapper.arm_target(MidiBindTarget::TrackPan(old_track));
    let mut original = vec![None; before.entries.len()];
    for (new, old) in result.sources.iter().enumerate() {
        if let Some(old) = old {
            original[*old] = Some(new);
        }
    }
    let valid_tracks: std::collections::HashSet<_> = result
        .snapshot
        .entries
        .iter()
        .map(|entry| entry.track.id)
        .collect();
    mapper.prune_orphans(
        &valid_tracks,
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );
    assert_eq!(
        mapper.source_for(MidiBindTarget::TrackVolume(old_track)),
        Some(MidiSource::Cc(20))
    );
    assert_eq!(
        mapper.armed_target(),
        Some(MidiBindTarget::TrackPan(old_track))
    );

    let mut clip = MidiClip::new("Melodía", 1, 4.0, Pattern::new(Tick(960)).unwrap());
    let clip_id = clip.id;
    clip.track = original[clip.track].unwrap();
    let tracks: Vec<_> = result
        .snapshot
        .entries
        .iter()
        .map(|entry| entry.track.clone())
        .collect();
    let project = ProjectDoc::from_session("Lote", &tracks, &[clip]).unwrap();
    let (_, clips) = ProjectDoc::import(&project.snapshot().unwrap())
        .unwrap()
        .materialize_midi_session()
        .unwrap();
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].id, clip_id);
    assert_eq!(clips[0].track, 2);
}

#[test]
fn invalid_destination_or_newer_revision_never_returns_a_partial_batch() {
    let before = origin();
    let script = r#"return {
      {op="insert_audio", handle="one", name="Uno"},
      {op="move", track="one", after=99999},
    }"#;
    let batch = build_track_batch(script, &before).unwrap();
    let error = apply_track_batch(&before, &batch).unwrap_err();
    assert_eq!(
        (error.code, error.operation),
        ("referencia_invalida", Some(2))
    );
    assert_eq!(before.entries.len(), 2);

    let mut newer = before.clone();
    newer.revision += 1;
    assert_eq!(
        apply_track_batch(&newer, &batch).unwrap_err().code,
        "lote_obsoleto"
    );
    assert_eq!(newer.entries.len(), 2);
}

#[test]
fn lua_rejects_unknown_fields_and_loro_rejects_a_bad_merge_without_changing_state() {
    let before = origin();
    let error = build_track_batch(
        r#"return {{op="insert_audio", handle="x", name="X", hidden=true}}"#,
        &before,
    )
    .unwrap_err();
    assert!(error.contains("Operación 1"));
    assert!(error.contains("hidden"));

    let tracks: Vec<_> = before
        .entries
        .iter()
        .map(|entry| entry.track.clone())
        .collect();
    let mut project = ProjectDoc::from_tracks("Base", &tracks).unwrap();
    let snapshot = project.snapshot().unwrap();
    assert!(project.merge(&[0, 1, 2, 3]).is_err());
    assert_eq!(project.snapshot().unwrap(), snapshot);
}

#[test]
fn batch_rejects_a_session_with_duplicate_instrument_assignments() {
    let mut before = origin();
    let instrument = elevate::musical::model::InstrumentId(9);
    before.entries[0].track.instrument = Some(instrument);
    before.entries[1].track.instrument = Some(instrument);
    let batch = TrackBatch {
        origin: before.clone(),
        edits: vec![TrackBatchEdit::Rename {
            track: TrackRef::Id(before.entries[0].track.id),
            name: "Nueva".into(),
        }],
    };
    let error = apply_track_batch(&before, &batch).unwrap_err();
    assert_eq!(
        (error.code, error.operation),
        ("estructura_invalida", Some(1))
    );
    assert_eq!(before.entries[0].track.name, "Kick");
}
