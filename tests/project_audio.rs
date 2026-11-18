use elevate::{
    musical::{
        clip::{ClipId, ClipProps, MidiClip},
        model::Pattern,
        time::Tick,
        track::{Track, TrackKind},
    },
    project::{ProjectAudioClip, ProjectDoc},
};

#[test]
fn audio_clip_metadata_survives_track_reorder_without_loading_file() {
    let first = Track::new("A", TrackKind::Audio);
    let second = Track::new("B", TrackKind::Audio);
    let clip = ProjectAudioClip {
        id: ClipId::fresh(),
        track: second.id,
        source: "/missing/session/take.wav".into(),
        name: "Toma 1".into(),
        start_beat: 8.0,
        trim_start: 1.25,
        trim_len: Some(2.5),
        props: ClipProps {
            muted: true,
            gain_db: -3.0,
            color_index: Some(4),
            ..ClipProps::default()
        },
    };
    let project = ProjectDoc::from_tracks("Sesión", &[first, second]).unwrap();
    project.reconcile_audio_clips(&[clip.clone()]).unwrap();
    project.move_track(1, 0).unwrap();
    let restored = ProjectDoc::import(&project.snapshot().unwrap()).unwrap();
    assert_eq!(restored.audio_clips().unwrap(), vec![clip]);
    assert_eq!(
        restored.tracks().unwrap()[0].id,
        restored.audio_clips().unwrap()[0].track
    );
}

#[test]
fn audio_and_midi_cannot_share_clip_identity() {
    let track = Track::new("Pista", TrackKind::Synth);
    let project = ProjectDoc::from_tracks("Sesión", &[track.clone()]).unwrap();
    let midi = MidiClip::new("Notas", 0, 0.0, Pattern::new(Tick(960)).unwrap());
    project.reconcile_midi_clips(&[midi.clone()]).unwrap();
    let audio = ProjectAudioClip {
        id: midi.id,
        track: track.id,
        source: "/audio.wav".into(),
        name: "Audio".into(),
        start_beat: 0.0,
        trim_start: 0.0,
        trim_len: None,
        props: ClipProps::default(),
    };
    assert!(project.reconcile_audio_clips(&[audio]).is_err());
    assert!(project.audio_clips().unwrap().is_empty());
}

#[test]
fn independent_audio_position_and_gain_edits_merge() {
    let track = Track::new("Audio", TrackKind::Audio);
    let clip = ProjectAudioClip {
        id: ClipId::fresh(),
        track: track.id,
        source: "/take.wav".into(),
        name: "Toma".into(),
        start_beat: 0.0,
        trim_start: 0.0,
        trim_len: None,
        props: ClipProps::default(),
    };
    let project = ProjectDoc::from_tracks("Sesión", &[track]).unwrap();
    project.reconcile_audio_clips(&[clip.clone()]).unwrap();
    let snapshot = project.snapshot().unwrap();
    let mut first = ProjectDoc::import(&snapshot).unwrap();
    let mut second = ProjectDoc::import(&snapshot).unwrap();
    first.set_audio_clip_start(clip.id, 12.0).unwrap();
    second.set_audio_clip_gain(clip.id, -9.0).unwrap();
    let a = first.all_updates().unwrap();
    let b = second.all_updates().unwrap();
    first.merge(&b).unwrap();
    second.merge(&a).unwrap();
    for merged in [&first, &second] {
        let audio = merged.audio_clips().unwrap();
        assert_eq!(audio[0].start_beat, 12.0);
        assert_eq!(audio[0].props.gain_db, -9.0);
        assert_eq!(audio[0].source, clip.source);
    }
}
