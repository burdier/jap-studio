use elevate::{
    musical::track::{
        DEMO_TRACKS, TrackBatch, TrackBatchEdit, TrackId, TrackRef, TrackSnapshot,
        apply_track_batch,
    },
    project::ProjectDoc,
    scripting::tracks::build_track_batch,
};

fn origin() -> TrackSnapshot {
    TrackSnapshot::capture(&DEMO_TRACKS[..2], &[75, 75], &[50, 50], 1).unwrap()
}

#[test]
fn handles_can_join_new_tracks_in_one_group_and_survive_loro_roundtrip() {
    let before = origin();
    let source = r#"
        return {
          { op="insert_audio", handle="a", name="A" },
          { op="insert_audio", handle="b", name="B", after="a" },
          { op="group", handle="vocals", name="Voces", members={"a", "b"} },
          { op="level", track="vocals", value=60 },
        }
    "#;
    let result = apply_track_batch(&before, &build_track_batch(source, &before).unwrap()).unwrap();
    let group = result.handles["vocals"];
    assert_eq!(result.snapshot.get(group).unwrap().mix.level, 60);
    for handle in ["a", "b"] {
        assert_eq!(
            result
                .snapshot
                .get(result.handles[handle])
                .unwrap()
                .track
                .parent,
            Some(group)
        );
    }
    let tracks: Vec<_> = result
        .snapshot
        .entries
        .iter()
        .map(|entry| entry.track.clone())
        .collect();
    let bytes = ProjectDoc::from_tracks("Voces", &tracks)
        .unwrap()
        .snapshot()
        .unwrap();
    let restored = ProjectDoc::import(&bytes).unwrap().tracks().unwrap();
    assert_eq!(
        restored
            .iter()
            .find(|track| track.id == result.handles["b"])
            .unwrap()
            .parent,
        Some(group)
    );
}

#[test]
fn forward_and_duplicate_handles_report_the_failing_operation() {
    let before = origin();
    let forward = TrackBatch {
        origin: before.clone(),
        edits: vec![TrackBatchEdit::Rename {
            track: TrackRef::Handle("later".into()),
            name: "X".into(),
        }],
    };
    let error = apply_track_batch(&before, &forward).unwrap_err();
    assert_eq!(
        (error.code, error.operation),
        ("referencia_invalida", Some(1))
    );

    let duplicate = TrackBatch {
        origin: before.clone(),
        edits: vec![
            TrackBatchEdit::InsertAudio {
                handle: "same".into(),
                name: "A".into(),
                after: None,
                parent: None,
            },
            TrackBatchEdit::InsertAudio {
                handle: "same".into(),
                name: "B".into(),
                after: None,
                parent: None,
            },
        ],
    };
    let error = apply_track_batch(&before, &duplicate).unwrap_err();
    assert_eq!(
        (error.code, error.operation),
        ("referencia_invalida", Some(2))
    );
    assert_eq!(before.entries.len(), 2);
    assert!(before.get(TrackId(1)).is_some());
}
