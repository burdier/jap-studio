use elevate::{
    musical::{
        clip::{ClipId, ClipProps, MidiClip},
        model::Pattern,
        time::Tick,
        track::{Track, TrackKind},
    },
    project::{ProjectAudioClip, ProjectDoc},
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(1);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "hay-collect-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            if fs::create_dir(&path).is_ok() {
                return Self(path);
            }
        }
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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

#[test]
fn collect_copies_shared_file_once_and_uses_relative_paths() {
    let root = TestRoot::new();
    let source = root.0.join("toma.wav");
    fs::write(&source, b"audio de prueba").unwrap();
    let track = Track::new("Audio", TrackKind::Audio);
    let clips = [0.0, 4.0].map(|beat| ProjectAudioClip {
        id: ClipId::fresh(),
        track: track.id,
        source: source.to_string_lossy().into_owned(),
        name: "Toma".into(),
        start_beat: beat,
        trim_start: 0.0,
        trim_len: None,
        props: ClipProps::default(),
    });
    let project = ProjectDoc::from_tracks("Sesión", &[track]).unwrap();
    project.reconcile_audio_clips(&clips).unwrap();
    let destination = root.0.join("Proyecto");
    assert_eq!(
        project
            .collect_audio_into(&destination, &root.0)
            .unwrap()
            .files,
        1
    );
    let packaged = ProjectDoc::load(&destination.join("session.loro")).unwrap();
    let collected = packaged.audio_clips().unwrap();
    assert_eq!(collected.len(), 2);
    assert_eq!(collected[0].source, collected[1].source);
    assert!(collected[0].source.starts_with("Audio/"));
    assert_eq!(
        fs::read(destination.join(&collected[0].source)).unwrap(),
        b"audio de prueba"
    );
    assert_eq!(
        project.audio_clips().unwrap()[0].source,
        source.to_string_lossy()
    );
}

#[test]
fn missing_source_does_not_publish_an_incomplete_folder() {
    let root = TestRoot::new();
    let source = root.0.join("toma.wav");
    fs::write(&source, b"audio de prueba").unwrap();
    let track = Track::new("Audio", TrackKind::Audio);
    let clips = [source, root.0.join("ausente.wav")].map(|path| ProjectAudioClip {
        id: ClipId::fresh(),
        track: track.id,
        source: path.to_string_lossy().into_owned(),
        name: "Toma".into(),
        start_beat: 0.0,
        trim_start: 0.0,
        trim_len: None,
        props: ClipProps::default(),
    });
    let project = ProjectDoc::from_tracks("Sesión", &[track]).unwrap();
    project.reconcile_audio_clips(&clips).unwrap();
    let destination = root.0.join("Proyecto");
    assert!(project.collect_audio_into(&destination, &root.0).is_err());
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
    assert_eq!(project.audio_clips().unwrap()[0].source, clips[0].source);
}

#[test]
fn collect_never_replaces_an_existing_directory() {
    let root = TestRoot::new();
    let destination = root.0.join("Proyecto");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("nota.txt"), "conservar").unwrap();
    let project = ProjectDoc::new("Sesión").unwrap();
    assert!(project.collect_audio_into(&destination, &root.0).is_err());
    assert_eq!(
        fs::read_to_string(destination.join("nota.txt")).unwrap(),
        "conservar"
    );
}
