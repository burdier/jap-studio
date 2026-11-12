use elevate::{
    musical::{
        fx::{FxEdit, FxKind, FxNode, FxPlacement},
        track::{
            DEMO_TRACKS, TrackBatch, TrackBatchEdit, TrackRef, TrackSnapshot, apply_track_batch,
        },
    },
    project::ProjectDoc,
    scripting::tracks::build_track_batch,
};

fn origin() -> TrackSnapshot {
    TrackSnapshot::capture(&DEMO_TRACKS[..2], &[75, 75], &[50, 50], 4).unwrap()
}

#[test]
fn two_new_tracks_publish_together_with_stable_ids_and_loro_projection() {
    let before = origin();
    let batch = TrackBatch {
        origin: before.clone(),
        edits: vec![
            TrackBatchEdit::InsertAudio {
                handle: "left".into(),
                name: "Izquierda".into(),
                after: None,
                parent: None,
            },
            TrackBatchEdit::Level {
                track: TrackRef::Handle("left".into()),
                value: 60,
            },
            TrackBatchEdit::InsertAudio {
                handle: "right".into(),
                name: "Derecha".into(),
                after: Some(TrackRef::Handle("left".into())),
                parent: None,
            },
            TrackBatchEdit::Pan {
                track: TrackRef::Handle("right".into()),
                value: 80,
            },
        ],
    };
    let result = apply_track_batch(&before, &batch).unwrap();
    assert_eq!(before.entries.len(), 2);
    assert_eq!(result.snapshot.revision, 5);
    assert_eq!(result.sources, vec![Some(0), Some(1), None, None]);
    assert_eq!(
        result
            .snapshot
            .get(result.handles["left"])
            .unwrap()
            .mix
            .level,
        60
    );
    assert_eq!(
        result
            .snapshot
            .get(result.handles["right"])
            .unwrap()
            .mix
            .pan,
        80
    );
    let tracks: Vec<_> = result
        .snapshot
        .entries
        .iter()
        .map(|entry| entry.track.clone())
        .collect();
    let project = ProjectDoc::from_tracks("Lote", &tracks).unwrap();
    assert_eq!(project.tracks().unwrap().len(), 4);
    assert!(project.snapshot().is_ok());
}

#[test]
fn obsolete_origin_and_missing_destination_reject_the_entire_plan() {
    let before = origin();
    let batch = TrackBatch {
        origin: before.clone(),
        edits: vec![
            TrackBatchEdit::InsertAudio {
                handle: "new".into(),
                name: "Nueva".into(),
                after: None,
                parent: None,
            },
            TrackBatchEdit::Move {
                track: TrackRef::Handle("new".into()),
                after: TrackRef::Id(elevate::musical::track::TrackId(9999)),
            },
        ],
    };
    let error = apply_track_batch(&before, &batch).unwrap_err();
    assert_eq!(error.code, "referencia_invalida");
    assert_eq!(error.operation, Some(2));
    assert_eq!(before.entries.len(), 2);
    let mut changed = before.clone();
    changed.entries[0].mix.level = 12;
    assert_eq!(
        apply_track_batch(&changed, &batch).unwrap_err().code,
        "lote_obsoleto"
    );
}

#[test]
fn fx_cycle_rejects_the_batch_without_changing_the_source() {
    let mut before = origin();
    let parent = FxNode::new("Serie", FxKind::SerialChain { children: vec![] });
    let child = FxNode::new("Paralelo", FxKind::ParallelChain { branches: vec![] });
    let parent_id = parent.id;
    let child_id = child.id;
    before.entries[0]
        .track
        .chain
        .apply(FxEdit::Insert(parent, FxPlacement::Inside(None)))
        .unwrap();
    before.entries[0]
        .track
        .chain
        .apply(FxEdit::Insert(child, FxPlacement::Inside(Some(parent_id))))
        .unwrap();
    let batch = TrackBatch {
        origin: before.clone(),
        edits: vec![FxEdit::Move(parent_id, FxPlacement::Inside(Some(child_id)))]
            .into_iter()
            .map(|edit| TrackBatchEdit::Fx {
                track: TrackRef::Id(before.entries[0].track.id),
                edit,
            })
            .collect(),
    };
    let error = apply_track_batch(&before, &batch).unwrap_err();
    assert_eq!(error.code, "estructura_invalida");
    assert_eq!(error.operation, Some(1));
    assert!(before.entries[0].track.chain.find(parent_id).is_some());
}

#[test]
fn bundled_lua_template_builds_two_configured_tracks() {
    let before = origin();
    let source = include_str!("../assets/scripts/tracks/two_tracks.lua");
    let batch = build_track_batch(source, &before).unwrap();
    let result = apply_track_batch(&before, &batch).unwrap();
    let principal = result.snapshot.get(result.handles["principal"]).unwrap();
    let doble = result.snapshot.get(result.handles["doble"]).unwrap();
    assert_eq!((principal.mix.level, principal.mix.pan), (75, 45));
    assert_eq!((doble.mix.level, doble.mix.pan), (65, 70));
    assert_eq!(principal.track.chain.nodes().len(), 1);
    assert_eq!(doble.track.chain.nodes().len(), 1);
    assert_eq!(principal.track.name, "Voz principal");
    assert_eq!(doble.track.name, "Voz doble");
}
