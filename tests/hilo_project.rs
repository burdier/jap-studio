use elevate::{
    musical::{
        clip::MidiClip,
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
        track::{HiloMode, Track, TrackKind},
    },
    project::ProjectDoc,
};

#[test]
fn hilo_mode_and_group_clip_survive_project_roundtrip() {
    let mut group = Track::new("Hilo", TrackKind::Group);
    group.hilo = Some(HiloMode::RoundRobin);
    let mut first = Track::new("A", TrackKind::Synth);
    first.parent = Some(group.id);
    first.instrument = Some(InstrumentId(41));
    let mut second = Track::new("B", TrackKind::Synth);
    second.parent = Some(group.id);
    second.instrument = Some(InstrumentId(42));
    let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
    pattern
        .add_note(InstrumentId(41), Tick(0), Tick(PPQ / 2), 60, 100)
        .unwrap();
    let clip = MidiClip::new("Secuencia", 0, 0.0, pattern);
    let snapshot = ProjectDoc::from_session("Hilo", &[group, first, second], &[clip])
        .unwrap()
        .snapshot()
        .unwrap();
    let (tracks, clips) = ProjectDoc::import(&snapshot)
        .unwrap()
        .materialize_midi_session()
        .unwrap();
    assert_eq!(tracks[0].hilo, Some(HiloMode::RoundRobin));
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].track, 0);
}

#[test]
fn midi_route_to_one_of_multiple_hilos_survives_project_roundtrip() {
    let mut first_hilo = Track::new("Uno", TrackKind::Group);
    first_hilo.hilo = Some(HiloMode::RoundRobin);
    let mut second_hilo = Track::new("Dos", TrackKind::Group);
    second_hilo.hilo = Some(HiloMode::Layer);
    let mut first_a = Track::new("Uno A", TrackKind::Synth);
    first_a.parent = Some(first_hilo.id);
    first_a.instrument = Some(InstrumentId(41));
    let mut first_b = Track::new("Uno B", TrackKind::Synth);
    first_b.parent = Some(first_hilo.id);
    first_b.instrument = Some(InstrumentId(42));
    let mut second_a = Track::new("Dos A", TrackKind::Synth);
    second_a.parent = Some(second_hilo.id);
    second_a.instrument = Some(InstrumentId(43));
    let mut second_b = Track::new("Dos B", TrackKind::Synth);
    second_b.parent = Some(second_hilo.id);
    second_b.instrument = Some(InstrumentId(44));
    let lane = Track::new("Secuencia", TrackKind::Synth);
    let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
    pattern
        .add_note(InstrumentId(41), Tick(0), Tick(PPQ / 2), 60, 100)
        .unwrap();
    let mut clip = MidiClip::new("Se queda aquí", 6, 0.0, pattern);
    clip.hilo_target = Some(second_hilo.id);
    let snapshot = ProjectDoc::from_session(
        "Rutas",
        &[
            first_hilo,
            first_a,
            first_b,
            second_hilo,
            second_a,
            second_b,
            lane,
        ],
        &[clip],
    )
    .unwrap()
    .snapshot()
    .unwrap();
    let (tracks, clips) = ProjectDoc::import(&snapshot)
        .unwrap()
        .materialize_midi_session()
        .unwrap();
    assert_eq!(clips[0].track, 6);
    assert_eq!(clips[0].hilo_target, Some(tracks[3].id));
    assert_ne!(clips[0].hilo_target, Some(tracks[0].id));
}
