//! Integración real de render, codecs C y decodificación independiente (sin ventana/CPAL).
use super::*;
use crate::audio::{
    AudioFile,
    piano::{PianoSettings, PianoSnapshot, VoiceSource},
};
use elevate::{
    export::ExportOptions,
    musical::{
        clip::{ClipProps, MidiClip},
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
        track::{HiloMode, Track, TrackKind},
    },
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static ID: AtomicU64 = AtomicU64::new(0);
        let root = if std::path::Path::new("/tmp/opencode").is_dir() {
            PathBuf::from("/tmp/opencode")
        } else {
            std::env::temp_dir()
        };
        let path = root.join(format!(
            "hay-export-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn options(&self, command: &str) -> ExportOptions {
        let mut options = ExportOptions::parse(command).unwrap();
        options.directory = self.0.clone();
        options
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn clip(frames: Vec<[f32; 2]>, rate: u32) -> AudioClip {
    AudioClip {
        id: elevate::musical::clip::ClipId::fresh(),
        name: "Fuente".into(),
        file: Arc::new(AudioFile {
            name: "Fuente".into(),
            source_path: None,
            sample_rate: rate,
            frames,
            peaks: Vec::new(),
            max_peak: 1.0,
        }),
        track: 0,
        start_beat: 0.0,
        props: ClipProps::default(),
        trim_start: 0.0,
        trim_len: None,
    }
}
fn snapshot(audio: Vec<AudioClip>) -> Snapshot {
    Snapshot {
        audio,
        midi: Vec::new(),
        piano: None,
        sampler: None,
        hilo: Vec::new(),
        tracks: vec![
            TrackMix {
                name: "Uno".into(),
                gain: 1.0,
                fx_gain: 1.0,
                pan: 0.0,
            },
            TrackMix {
                name: "Dos".into(),
                gain: 1.0,
                fx_gain: 1.0,
                pan: 0.0,
            },
        ],
        utility: UtilityState::default(),
        bpm: 120.0,
    }
}
fn run(snapshot: Snapshot, options: ExportOptions) -> Report {
    job::run(
        snapshot,
        options,
        &AtomicBool::new(false),
        &AtomicU32::new(0),
    )
    .unwrap()
}
fn render(snapshot: &Snapshot, command: &str) -> Vec<[f32; 2]> {
    let options = ExportOptions::parse(command).unwrap();
    let range = render::range(snapshot, &options).unwrap();
    let mut output = Vec::new();
    render::render(
        snapshot,
        &options,
        &[0, 1],
        range,
        &AtomicBool::new(false),
        |block, _| {
            output.extend_from_slice(block);
            Ok(())
        },
    )
    .unwrap();
    output
}

#[test]
fn overlap_trim_gain_pan_and_utility_match_stems_and_master() {
    let temp = Temp::new();
    let make = || {
        let mut data = vec![[0.9, 0.9]; 960];
        data.extend(vec![[0.4, 0.2]; 3840]);
        let mut a = clip(data, 48000);
        a.trim_start = 0.02;
        a.trim_len = Some(0.04);
        a.start_beat = 0.04;
        let mut b = a.clone();
        b.track = 1;
        a.props.gain_db = 20.0 * 0.5_f32.log10();
        let mut muted = a.clone();
        muted.props.muted = true;
        let mut snap = snapshot(vec![a, b, muted]);
        snap.tracks[0].gain = 0.5;
        snap.tracks[0].pan = -1.0;
        snap.tracks[1].gain = 0.25;
        snap.tracks[1].pan = 1.0;
        snap.utility = UtilityState {
            inserted: true,
            gain: 2.0,
            bypassed: false,
        };
        snap
    };
    let master = run(make(), temp.options("export -n Master.wav -bits 32f"));
    let master = AudioFile::decode(&master.paths[0]).unwrap();
    assert_eq!(master.frames.len(), 2880);
    assert!(master.frames[..960].iter().all(|f| *f == [0.0; 2]));
    assert!((master.frames[1000][0] - 0.2).abs() < 1e-5);
    assert!((master.frames[1000][1] - 0.1).abs() < 1e-5);
    let stems = run(
        make(),
        temp.options("export -n Stems.wav -individuals -bits 32f"),
    );
    assert_eq!(stems.paths.len(), 2);
    let a = AudioFile::decode(&stems.paths[0]).unwrap();
    let b = AudioFile::decode(&stems.paths[1]).unwrap();
    assert_eq!(a.frames.len(), b.frames.len());
    for (master, (a, b)) in master.frames.iter().zip(a.frames.iter().zip(&b.frames)) {
        for channel in 0..2 {
            assert!((master[channel] - (a[channel] + b[channel]) * 2.0).abs() < 1e-5);
        }
    }
    let cropped = render(&make(), "export -start 0.06 -length 0.02");
    assert_eq!(cropped.len(), 480);
    assert_eq!(cropped, master.frames[1440..1920]);
    let mut muted_track = make();
    muted_track.tracks[0].gain = 0.0;
    assert!(render(&muted_track, "export").iter().all(|f| f[0] == 0.0));
    let tail = render(&make(), "export -start 0.06 -length 0.02 -tail 0.02");
    assert_eq!(tail.len(), 1440);
    assert_eq!(&tail[..480], &cropped);
    assert!(
        tail[480..].iter().all(|f| *f == [0.0; 2]),
        "la cola no debe incluir más audio del Arrangement"
    );
}

#[test]
fn real_codecs_round_trip_mono_resampled_audio_with_exact_pcm_duration() {
    let temp = Temp::new();
    let data: Vec<_> = (0..4410)
        .map(|i| {
            let s = (i as f32 * std::f32::consts::TAU * 1000.0 / 44100.0).sin() * 0.5;
            [s, s]
        })
        .collect();
    let report = run(
        snapshot(vec![clip(data, 44100)]),
        temp.options("export -n Test.[wav,flac,mp3,aiff] -sr 48000 -bits 24 -mono -tail 0.01"),
    );
    assert_eq!(report.paths.len(), 4);
    assert_eq!(report.overs, 0);
    for path in &report.paths {
        if path.extension().unwrap() == "aiff" {
            let bytes = fs::read(path).unwrap();
            assert_eq!(&bytes[..4], b"FORM");
            assert!(bytes.windows(4).any(|x| x == b"AIFF"));
            continue;
        }
        let decoded = AudioFile::decode(path).unwrap();
        assert_eq!(decoded.sample_rate, 48000);
        if path.extension().unwrap() != "mp3" {
            assert_eq!(decoded.frames.len(), 5280);
            assert!(decoded.frames[4800..].iter().all(|f| *f == [0.0; 2]));
            let error: f32 = decoded.frames[200..4500]
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let expected =
                        (((i + 200) as f32) * std::f32::consts::TAU * 1000.0 / 48000.0).sin() * 0.5;
                    (f[0] - expected).abs()
                })
                .sum::<f32>()
                / 4300.0;
            assert!(error < 0.001, "remuestreo/latencia {error}");
        }
        assert!(decoded.frames.iter().any(|f| f[0].abs() > 0.2));
        assert!(decoded.frames.iter().all(|f| f[0] == f[1]));
    }
    let bytes = fs::read(&report.paths[0]).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1); // canales fmt WAV
    assert!(
        !fs::read_dir(&temp.0).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "part"))
    );
}

#[test]
fn pcm_depths_preserve_float_headroom_and_vbr_mp3_decodes() {
    let temp = Temp::new();
    for bits in ["16", "24", "32", "32f"] {
        let report = run(
            snapshot(vec![clip(vec![[1.25, -1.25]; 1024], 48000)]),
            temp.options(&format!("export -n Depth{bits}.wav -bits {bits}")),
        );
        assert_eq!(report.overs, 2048);
        let decoded = AudioFile::decode(&report.paths[0]).unwrap();
        let expected = if bits == "32f" { 1.25 } else { 1.0 };
        assert!((decoded.frames[500][0] - expected).abs() < 0.0001);
        assert!((decoded.frames[500][1] + expected).abs() < 0.0001);
    }
    let data: Vec<_> = (0..4800)
        .map(|i| {
            let s = (i as f32 * 0.1).sin() * 0.25;
            [s, -s]
        })
        .collect();
    let report = run(
        snapshot(vec![clip(data, 48000)]),
        temp.options("export -n Variable.mp3 -vbr 2"),
    );
    assert!(
        AudioFile::decode(&report.paths[0])
            .unwrap()
            .frames
            .iter()
            .any(|f| f[0].abs() > 0.1)
    );
}

#[test]
fn collisions_cancellation_and_codec_errors_leave_no_partial_exports() {
    let temp = Temp::new();
    let make = || snapshot(vec![clip(vec![[0.1; 2]; 1000], 48000)]);
    fs::write(temp.0.join("Keep.flac"), b"user data").unwrap();
    let result = job::run(
        make(),
        temp.options("export -n Keep.[wav,flac]"),
        &AtomicBool::new(false),
        &AtomicU32::new(0),
    );
    assert!(result.is_err());
    assert_eq!(fs::read(temp.0.join("Keep.flac")).unwrap(), b"user data");
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    let result = job::run(
        make(),
        temp.options("export -n Cancel.wav"),
        &AtomicBool::new(true),
        &AtomicU32::new(0),
    );
    assert!(result.unwrap_err().contains("cancelada"));
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    assert!(native::Library::open(c"libhay_does_not_exist.so").is_err());
}

#[test]
fn midi_offline_uses_soundfont_settings_and_preserves_pre_roll_for_cropped_exports() {
    let mut pattern = Pattern::new(Tick(960)).unwrap();
    pattern
        .add_note(InstrumentId(1), Tick(0), Tick(480), 60, 110)
        .unwrap();
    let mut snap = snapshot(Vec::new());
    snap.midi.push(MidiClip::new("Piano", 0, 0.25, pattern));
    snap.piano = Some(PianoSnapshot {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/soundfonts/UprightPianoKW-20220221.sf2"),
        settings: PianoSettings {
            maximum_polyphony: 64,
            reverb_and_chorus: false,
        },
        midi: vec![(0xB0, 7, 100), (0xC0, 0, 0)],
        master_volume: 0.7,
        filter: Default::default(),
    });
    let all = render(&snap, "export -tail 0.1");
    assert!(all[..6000].iter().all(|f| *f == [0.0; 2]));
    assert!(all[6000..].iter().flatten().any(|x| x.abs() > 0.001));
    let crop = render(&snap, "export -start 0.5 -length 0.5");
    assert_eq!(crop.len(), 12000);
    for (a, b) in crop
        .iter()
        .flatten()
        .zip(all[12000..24000].iter().flatten())
    {
        assert!((a - b).abs() < 1e-5);
    }
    snap.piano.as_mut().unwrap().filter = elevate::devices::filter::FilterSettings {
        enabled: true,
        cutoff: 40.,
        ..Default::default()
    };
    let filtered = render(&snap, "export -tail 0.1");
    let energy = |frames: &[[f32; 2]]| frames.iter().flatten().map(|x| x * x).sum::<f32>();
    assert!(
        energy(&filtered) < energy(&all) * 0.1,
        "el filtro del snapshot debe procesar el SoundFont exportado"
    );
    snap.piano.as_mut().unwrap().master_volume = 0.0;
    let silent = render(&snap, "export");
    let peak = silent
        .iter()
        .flatten()
        .fold(0.0_f32, |peak, x| peak.max(x.abs()));
    assert!(peak < 1e-7, "master cero: pico {peak}");
}

#[test]
fn hilo_export_routes_two_pianos_to_master_and_branch_stems() {
    let temp = Temp::new();
    let make = || {
        let mut group = Track::new("Hilo", TrackKind::Group);
        group.hilo = Some(HiloMode::RoundRobin);
        let instruments = [InstrumentId(71), InstrumentId(72)];
        let branches: Vec<_> = instruments
            .iter()
            .map(|&instrument| {
                let mut track = Track::new("Piano", TrackKind::Synth);
                track.parent = Some(group.id);
                track.instrument = Some(instrument);
                track
            })
            .collect();
        let mut pattern = Pattern::new(Tick(PPQ * 4)).unwrap();
        for index in 0..4 {
            pattern
                .add_note(
                    instruments[0],
                    Tick(index * PPQ),
                    Tick(PPQ / 2),
                    60 + index as u8,
                    100,
                )
                .unwrap();
        }
        let piano = PianoSnapshot {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/soundfonts/UprightPianoKW-20220221.sf2"),
            settings: PianoSettings {
                maximum_polyphony: 64,
                reverb_and_chorus: false,
            },
            midi: vec![(0xB0, 7, 100), (0xC0, 0, 0)],
            master_volume: 0.7,
            filter: Default::default(),
        };
        let mut snap = snapshot(Vec::new());
        snap.tracks = ["Hilo", "A", "B"]
            .map(|name| TrackMix {
                name: name.into(),
                gain: 1.0,
                fx_gain: 1.0,
                pan: 0.0,
            })
            .to_vec();
        snap.midi.push(MidiClip::new("Notas", 0, 0.0, pattern));
        snap.hilo = branches
            .into_iter()
            .enumerate()
            .map(|(branch_index, track)| HiloExportBranch {
                track_index: branch_index + 1,
                group_index: 0,
                branch_index,
                instruments: instruments.to_vec(),
                mode: HiloMode::RoundRobin,
                weave: None,
                track,
                source: VoiceSource::Piano(PianoSnapshot {
                    master_volume: if branch_index == 0 { 0.7 } else { 0.3 },
                    ..piano.clone()
                }),
            })
            .collect();
        snap
    };
    let master = run(make(), temp.options("export -n HiloMaster.wav -bits 32f"));
    let master = AudioFile::decode(&master.paths[0]).unwrap();
    let mut routed = make();
    let target = routed.hilo[0].track.parent.unwrap();
    routed.tracks.push(TrackMix {
        name: "Secuencia".into(),
        gain: 1.0,
        fx_gain: 1.0,
        pan: 0.0,
    });
    routed.midi[0].track = 3;
    routed.midi[0].hilo_target = Some(target);
    let routed = run(routed, temp.options("export -n HiloEnviado.wav -bits 32f"));
    let routed = AudioFile::decode(&routed.paths[0]).unwrap();
    assert_eq!(routed.frames, master.frames);
    let stems = run(
        make(),
        temp.options("export -n HiloStems.wav -individuals -bits 32f"),
    );
    assert_eq!(stems.paths.len(), 2);
    let a = AudioFile::decode(&stems.paths[0]).unwrap();
    let b = AudioFile::decode(&stems.paths[1]).unwrap();
    assert_eq!(master.frames.len(), a.frames.len());
    assert_eq!(a.frames.len(), b.frames.len());
    assert!(a.frames[..6_000].iter().flatten().any(|x| x.abs() > 0.001));
    assert!(b.frames[..6_000].iter().all(|frame| *frame == [0.0; 2]));
    assert!(
        b.frames[24_000..30_000]
            .iter()
            .flatten()
            .any(|x| x.abs() > 0.001)
    );
    let only = run(
        make(),
        temp.options("export -n Solo.wav -tracks 2 -bits 32f"),
    );
    let only = AudioFile::decode(&only.paths[0]).unwrap();
    assert_eq!(only.frames, a.frames);
    for (master, (a, b)) in master.frames.iter().zip(a.frames.iter().zip(&b.frames)) {
        for channel in 0..2 {
            assert!((master[channel] - a[channel] - b[channel]).abs() < 2e-5);
        }
    }
}
