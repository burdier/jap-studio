use elevate::{
    musical::{
        time::{Meter, Tempo},
        track::{HiloMode, Track, TrackKind, TrackMix},
    },
    project::ProjectDoc,
};

#[test]
fn hilo_mix_stays_with_track_ids_after_reorder_and_snapshot() {
    let mut group = Track::new("Hilo", TrackKind::Group);
    group.hilo = Some(HiloMode::RoundRobin);
    let mut a = Track::new("A", TrackKind::Synth);
    a.parent = Some(group.id);
    a.instrument = Some(elevate::musical::model::InstrumentId(101));
    let mut b = Track::new("B", TrackKind::Synth);
    b.parent = Some(group.id);
    b.instrument = Some(elevate::musical::model::InstrumentId(102));
    let id_a = a.id;
    let id_b = b.id;
    let tempo = Tempo::new(127.5).unwrap();
    let meter = Meter::new(7, 8).unwrap();
    let project = ProjectDoc::from_session_state(
        "Colaboración",
        &[group, a, b],
        &[],
        tempo,
        meter,
        &[75, 62, 91],
        &[50, 20, 80],
    )
    .unwrap();
    project.move_track(1, 2).unwrap();
    let imported = ProjectDoc::import(&project.snapshot().unwrap()).unwrap();
    assert_eq!(imported.timing().unwrap(), (tempo, meter));
    assert_eq!(
        imported.track_mix(id_a).unwrap(),
        TrackMix { level: 62, pan: 20 }
    );
    assert_eq!(
        imported.track_mix(id_b).unwrap(),
        TrackMix { level: 91, pan: 80 }
    );
    assert_eq!(imported.tracks().unwrap()[1].id, id_b);
}

#[test]
fn older_project_defaults_time_and_mix_and_rejects_bad_edit() {
    let track = Track::new("Audio", TrackKind::Audio);
    let project = ProjectDoc::from_session("Anterior", &[track.clone()], &[]).unwrap();
    let imported = ProjectDoc::import(&project.snapshot().unwrap()).unwrap();
    assert_eq!(
        imported.timing().unwrap(),
        (Tempo::default(), Meter::default())
    );
    assert_eq!(
        imported.track_mix(track.id).unwrap(),
        TrackMix { level: 75, pan: 50 }
    );
    assert!(
        imported
            .set_track_mix(
                track.id,
                TrackMix {
                    level: 101,
                    pan: 50
                }
            )
            .is_err()
    );
    assert_eq!(
        imported.track_mix(track.id).unwrap(),
        TrackMix { level: 75, pan: 50 }
    );
}

#[test]
fn independent_time_and_mix_edits_merge_without_overwriting_each_other() {
    let track = Track::new("Audio", TrackKind::Audio);
    let base = ProjectDoc::from_session("Colaboración", &[track.clone()], &[])
        .unwrap()
        .snapshot()
        .unwrap();
    let mut first = ProjectDoc::import(&base).unwrap();
    let mut second = ProjectDoc::import(&base).unwrap();
    first.set_tempo(Tempo::new(135.25).unwrap()).unwrap();
    first.set_track_level(track.id, 61).unwrap();
    second.set_meter(Meter::new(5, 4).unwrap()).unwrap();
    second.set_track_pan(track.id, 82).unwrap();
    let first_updates = first.all_updates().unwrap();
    let second_updates = second.all_updates().unwrap();
    first.merge(&second_updates).unwrap();
    second.merge(&first_updates).unwrap();
    for project in [&first, &second] {
        assert_eq!(
            project.timing().unwrap(),
            (Tempo::new(135.25).unwrap(), Meter::new(5, 4).unwrap())
        );
        assert_eq!(
            project.track_mix(track.id).unwrap(),
            TrackMix { level: 61, pan: 82 }
        );
    }
}

#[test]
fn competing_meter_edits_resolve_to_one_complete_value() {
    let track = Track::new("Audio", TrackKind::Audio);
    let base = ProjectDoc::from_session("Compás", &[track], &[])
        .unwrap()
        .snapshot()
        .unwrap();
    let mut first = ProjectDoc::import(&base).unwrap();
    let mut second = ProjectDoc::import(&base).unwrap();
    let a = Meter::new(3, 4).unwrap();
    let b = Meter::new(7, 8).unwrap();
    first.set_meter(a).unwrap();
    second.set_meter(b).unwrap();
    let first_updates = first.all_updates().unwrap();
    let second_updates = second.all_updates().unwrap();
    first.merge(&second_updates).unwrap();
    second.merge(&first_updates).unwrap();
    let result = first.timing().unwrap().1;
    assert!(result == a || result == b);
    assert_eq!(second.timing().unwrap().1, result);
}
