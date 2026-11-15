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
