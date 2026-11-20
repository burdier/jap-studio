use super::snapshot::Snapshot;
use crate::instruments::Piano;
use elevate::{
    musical::{
        clip::{ClipId, ClipProps, MidiClip},
        model::{InstrumentId, Pattern},
        time::{Meter, Tempo, Tick},
        track::{Track, TrackKind},
    },
    project::ProjectAudioClip,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "hay-project-open-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn snapshot() -> Snapshot {
    let tracks = vec![
        Track::new("Audio", TrackKind::Audio),
        Track::new("Notas", TrackKind::Synth),
    ];
    let mut pattern = Pattern::new(Tick(3840)).unwrap();
    pattern
        .add_note(InstrumentId(1), Tick(240), Tick(480), 64, 93)
        .unwrap();
    Snapshot {
        tracks,
        midi: vec![MidiClip::new("Melodía", 1, 3.0, pattern)],
        audio: vec![],
        pianos: vec![],
        drums: Default::default(),
        tempo: Tempo::new(97.5).unwrap(),
        meter: Meter::new(7, 8).unwrap(),
        levels: vec![63, 82],
        pans: vec![12, 74],
    }
}
fn wav(path: &std::path::Path) {
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(96000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend([0, 0, 0, 32]);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn saved_arrangement_reopens_after_folder_move_with_shared_audio_and_stable_ids() {
    let fixture = Fixture::new();
    let original = fixture.0.join("original");
    std::fs::create_dir(&original).unwrap();
    wav(&original.join("take.wav"));
    let mut before = snapshot();
    for beat in [2., 8.] {
        before.audio.push(ProjectAudioClip {
            id: ClipId::fresh(),
            track: before.tracks[0].id,
            source: "take.wav".into(),
            name: "Toma".into(),
            start_beat: beat,
            trim_start: 0.00001,
            trim_len: Some(0.00002),
            props: ClipProps {
                gain_db: -6.,
                ..Default::default()
            },
        });
    }
    before
        .document("Prueba")
        .unwrap()
        .save(&original.join("session.loro"))
        .unwrap();
    let moved = fixture.0.join("moved");
    std::fs::rename(original, &moved).unwrap();
    let (after, audio, _) = Snapshot::load(&moved.join("session.loro")).unwrap();
    assert_eq!(
        after.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
        before.tracks.iter().map(|t| t.id).collect::<Vec<_>>()
    );
    assert_eq!(after.midi[0].id, before.midi[0].id);
    assert_eq!(after.midi[0].pattern, before.midi[0].pattern);
    assert_eq!(
        (after.tempo, after.meter, after.levels, after.pans),
        (before.tempo, before.meter, before.levels, before.pans)
    );
    assert!(Arc::ptr_eq(&audio[0].file, &audio[1].file));
    assert_eq!(audio[1].id, before.audio[1].id);
    assert_eq!(
        (
            audio[1].start_beat,
            audio[1].trim_start,
            audio[1].trim_len,
            audio[1].props
        ),
        (8., 0.00001, Some(0.00002), before.audio[1].props)
    );
    assert_eq!(
        audio[0].file.source_path.as_ref().unwrap(),
        &moved.join("take.wav")
    );
    std::fs::remove_file(moved.join("take.wav")).unwrap();
    assert!(
        Snapshot::load(&moved.join("session.loro"))
            .err()
            .unwrap()
            .contains("Falta audio")
    );
}

#[test]
fn piano_parameters_survive_disk_reload_and_missing_soundfont_is_rejected() {
    let fixture = Fixture::new();
    let mut before = snapshot();
    let id = InstrumentId::fresh();
    let mut piano = Piano::new_with_id(id);
    piano.set_param(crate::instruments::piano_params::ParamId::Cutoff, 2345.);
    before.tracks[1].instrument = Some(id);
    before.pianos.push(piano.project_state());
    before.resolve_piano_sources().unwrap();
    let source = fixture.0.join("piano.sf2");
    std::fs::copy(&before.pianos[0].source, &source).unwrap();
    before.pianos[0].source = source.to_str().unwrap().into();
    let path = fixture.0.join("session.loro");
    before.document("Piano").unwrap().save(&path).unwrap();
    let (after, _, _) = Snapshot::load(&path).unwrap();
    assert_eq!(after.pianos, before.pianos);
    let restored = Piano::restore_project(&after.pianos[0]).unwrap();
    assert_eq!(
        restored.param(crate::instruments::piano_params::ParamId::Cutoff),
        2345.
    );
    assert!(InstrumentId::fresh().0 > id.0);
    std::fs::remove_file(source).unwrap();
    assert!(Snapshot::load(&path).err().unwrap().contains("SoundFont"));
}

#[test]
fn unsupported_instrument_and_corrupt_project_fail_before_publication() {
    let fixture = Fixture::new();
    let mut state = snapshot();
    state.tracks[0].instrument = Some(InstrumentId(2));
    assert!(
        state
            .validate_support()
            .unwrap_err()
            .contains("Sampler/VST3")
    );
    let path = fixture.0.join("bad.loro");
    std::fs::write(&path, b"not a project").unwrap();
    assert!(Snapshot::load(&path).is_err());
}

#[test]
fn subsequent_save_reconciles_existing_document_after_track_removal() {
    let fixture = Fixture::new();
    let before = snapshot();
    let first = before.document("Primero").unwrap().snapshot().unwrap();
    let mut next = snapshot();
    next.midi.clear();
    next.tracks.truncate(1);
    next.levels.truncate(1);
    next.pans.truncate(1);
    let doc = next.document_from("Segundo", Some(&first)).unwrap();
    let path = fixture.0.join("session.loro");
    doc.save(&path).unwrap();
    let (loaded, _, _) = Snapshot::load(&path).unwrap();
    assert_eq!(loaded.tracks.len(), 1);
    assert_eq!(loaded.tracks[0].id, next.tracks[0].id);
    assert!(loaded.midi.is_empty());
    assert_eq!(
        elevate::project::ProjectDoc::load(&path)
            .unwrap()
            .name()
            .unwrap(),
        "Segundo"
    );
}
