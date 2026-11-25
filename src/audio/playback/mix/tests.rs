use super::*;
use crate::audio::{
    AudioFile,
    effects::{ComponentEffect, Plan, Rack},
};
use elevate::musical::clip::ClipProps;
fn clip(value: [f32; 2], start_beat: f64, frames: usize) -> AudioClip {
    AudioClip {
        processing: Default::default(),
        id: elevate::musical::clip::ClipId::fresh(),
        name: "fixture".into(),
        file: Arc::new(AudioFile {
            name: "fixture".into(),
            source_path: None,
            sample_rate: 48000,
            frames: vec![value; frames],
            peaks: vec![],
            max_peak: 1.,
        }),
        source_path_override: None,
        track: 0,
        start_beat,
        props: ClipProps::default(),
        trim_start: 0.,
        trim_len: None,
    }
}
fn track(id: u32, clips: Vec<AudioClip>) -> TrackInput {
    TrackInput {
        id: TrackId(id),
        parent: None,
        is_return: false,
        sends: Vec::new(),
        source_audible: true,
        audible: true,
        clips,
        gain: 1.,
        pan: 0.,
        fx_gain: 1.,
        rack: None,
        midi: None,
        automation: Default::default(),
    }
}
#[test]
fn group_runs_after_its_children_and_applies_fx_once() {
    let mut group = track(1, vec![]);
    group.rack = Some(Rack::new(Plan::Gain(0.5), 0));
    group.gain = 0.5;
    let mut first = track(2, vec![clip([0.25, 0.], 0., 256)]);
    first.parent = Some(group.id);
    let mut second = track(3, vec![clip([0.5, 0.], 0., 256)]);
    second.parent = Some(group.id);
    let outside = track(4, vec![clip([0.125, 0.], 0., 256)]);
    let mut mix = Mix::prepare(
        vec![group, first, second, outside],
        None,
        48_000,
        120.,
        Meter::default(),
    )
    .unwrap();
    let mut frames = [[0.; 2]; 256];
    let mut stems = StemBuffers::from([
        (Stem::Track(TrackId(1)), [[0.; 2]; 256]),
        (Stem::Track(TrackId(2)), [[0.; 2]; 256]),
    ]);
    mix.render_block(0, &mut frames, None, None, false, Some(&mut stems));
    assert_eq!(frames[255], [0.3125, 0.]);
    assert_eq!(stems[&Stem::Track(TrackId(1))][255], [0.1875, 0.]);
    assert_eq!(stems[&Stem::Track(TrackId(2))][255], [0.25, 0.]);
}

#[test]
fn pdc_aligns_children_before_bus_and_roots_before_master() {
    let (longest, input, output) = compensation(&[7, 3, 11, 4], &[None, Some(0), Some(0), None]);
    assert_eq!(longest, 18);
    assert_eq!(input, [11, 0, 0, 0]);
    assert_eq!(output, [0, 8, 0, 14]);
}
#[test]
fn track_meter_measures_post_fader_signal_before_the_master_sum() {
    let mut attenuated = track(1, vec![clip([0.8, -0.6], 0.0, 256)]);
    attenuated.gain = 0.5;
    let mut muted = track(2, vec![clip([1.0, 1.0], 0.0, 256)]);
    muted.gain = 0.0;
    let mut mix = Mix::prepare(
        vec![attenuated, muted],
        None,
        48_000,
        120.0,
        Meter::default(),
    )
    .unwrap();
    let mut frames = [[0.0; 2]; 256];
    mix.render(0, &mut frames);
    let peak = |mix: &Mix, track: usize| {
        f32::from_bits(mix.tracks[track].control.peak.load(Ordering::Relaxed))
    };
    assert!((peak(&mix, 0) - 0.4).abs() < 1e-6);
    assert_eq!(peak(&mix, 1), 0.0);
    mix.render(256, &mut frames);
    assert!((0.0..0.4).contains(&peak(&mix, 0)));
}

#[test]
fn shared_mix_sums_tracks_with_headroom_and_clamps_only_at_output() {
    let mut mix = Mix::prepare(
        vec![
            track(1, vec![clip([0.8, -0.8], 0., 12000)]),
            track(2, vec![clip([0.7, -0.7], 0., 12000)]),
        ],
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut frames = vec![[0.; 2]; 8192];
    mix.render(0, &mut frames);
    let last = *frames.last().unwrap();
    assert!((last[0] - 1.5).abs() < 0.001);
    assert!((last[1] + 1.5).abs() < 0.001);
    let mut physical = [9_f32; 4];
    crate::audio::output::write_stereo(&mut physical, &[last], 4, 1);
    assert_eq!(physical, [0., 1., -1., 0.]);
}
#[test]
fn offsets_overlap_and_end_are_independent_of_callback_size() {
    let input = || {
        vec![
            track(
                1,
                vec![clip([0.2, 0.], 0., 10000), clip([0.3, 0.], 0.125, 5000)],
            ),
            track(2, vec![clip([0., 0.4], 0.25, 6000)]),
        ]
    };
    let mut whole = Mix::prepare(
        input(),
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut divided = Mix::prepare(
        input(),
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    assert_eq!(whole.end, 12000);
    let mut a = vec![[0.; 2]; 13000];
    let mut b = a.clone();
    whole.render(0, &mut a);
    for (i, block) in b.chunks_mut(137).enumerate() {
        divided.render((i * 137) as u64, block);
    }
    assert_eq!(a, b);
    assert!(a[5000][0] > 0.49 && a[5000][1] == 0.);
    assert!(a[9000][0] > 0.19 && a[9000][1] > 0.39);
    assert_eq!(&a[12000..], &vec![[0.; 2]; 1000]);
}
#[test]
fn filter_changes_only_its_track_and_shared_track_clips_enter_one_bus() {
    let descriptor = Arc::new(
        elevate::scripting::audio_components::parse(include_str!(
            "../../../../assets/scripts/components/audio/filter.lua"
        ))
        .unwrap(),
    );
    let filter = ComponentEffect::prepare(descriptor, 48000).unwrap();
    filter.set(2, 1.).unwrap();
    let mut filtered = track(
        1,
        vec![clip([0.8, 0.], 0., 12000), clip([0.2, 0.], 0., 12000)],
    );
    filtered.rack = Some(Rack::new(
        Plan::Component(elevate::musical::fx::FxId(99), filter, None),
        0,
    ));
    let mut mix = Mix::prepare(
        vec![filtered, track(2, vec![clip([0., 0.4], 0., 12000)])],
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut output = vec![[0.; 2]; 8192];
    mix.render(0, &mut output);
    let last = output.last().unwrap();
    assert!(last[0].abs() < 0.0001);
    assert!((last[1] - 0.4).abs() < 0.001);
}

#[test]
fn live_mute_solo_level_and_pan_use_shared_resolution_without_muting_other_tracks() {
    use elevate::musical::track::{Track, TrackKind, effective_mix};
    let mut tracks = vec![
        Track::new("A", TrackKind::Audio),
        Track::new("B", TrackKind::Audio),
    ];
    let mut inputs = vec![
        track(0, vec![clip([0.4, 0.], 0., 100000)]),
        track(0, vec![clip([0., 0.6], 0., 100000)]),
    ];
    for (input, t) in inputs.iter_mut().zip(&tracks) {
        input.id = t.id;
    }
    let mut mix = Mix::prepare(
        inputs,
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut levels = vec![75, 75];
    let mut pans = vec![50, 50];
    let mut position = 0;
    let mut hear = |tracks: &[Track], levels: &[i32], pans: &[i32]| {
        // The same resolver used by DawApp publishes to the same controls read by render.
        for (i, t) in tracks.iter().enumerate() {
            let effective = effective_mix(tracks, levels, pans, i);
            let control = &mix
                .tracks
                .iter()
                .find(|p| p.control.id == t.id)
                .unwrap()
                .control;
            control
                .gain
                .store(effective.gain.to_bits(), Ordering::Relaxed);
            control.pan.set(effective.pan);
        }
        let mut output = vec![[0.; 2]; 8192];
        mix.render(position, &mut output);
        position += 8192;
        *output.last().unwrap()
    };
    let close = |actual: [f32; 2], expected: [f32; 2]| {
        assert!(
            (actual[0] - expected[0]).abs() < 0.001 && (actual[1] - expected[1]).abs() < 0.001,
            "{actual:?} != {expected:?}"
        );
    };
    close(hear(&tracks, &levels, &pans), [0.4, 0.6]);
    tracks[0].muted = true;
    close(hear(&tracks, &levels, &pans), [0., 0.6]);
    tracks[0].muted = false;
    tracks[0].solo = true;
    close(hear(&tracks, &levels, &pans), [0.4, 0.]);
    tracks[1].solo = true;
    close(hear(&tracks, &levels, &pans), [0.4, 0.6]);
    tracks[0].muted = true;
    close(hear(&tracks, &levels, &pans), [0., 0.6]);
    tracks[0].muted = false;
    tracks[0].solo = false;
    tracks[1].solo = false;
    levels[0] = 30;
    close(hear(&tracks, &levels, &pans), [0.16, 0.6]);
    pans[0] = 100;
    close(hear(&tracks, &levels, &pans), [0., 0.6]);
    pans[0] = 50;
    pans[1] = 0;
    close(hear(&tracks, &levels, &pans), [0.16, 0.]);
    // Reordering UI tracks keeps updates addressed to their identities.
    tracks.swap(0, 1);
    levels.swap(0, 1);
    pans.swap(0, 1);
    close(hear(&tracks, &levels, &pans), [0.16, 0.]);
}

#[test]
fn shared_transport_loops_and_seeks_all_sources_at_the_same_frame() {
    use crate::audio::playback::transport::{Cursor, Request};
    let input = || {
        vec![
            track(1, vec![clip([0.25, 0.], 0., 20000)]),
            track(2, vec![clip([0., 0.5], 0.125, 20000)]),
        ]
    };
    let mut a = Mix::prepare(
        input(),
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut b = Mix::prepare(
        input(),
        None,
        48000,
        120.,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut ca = Cursor::new(0, a.end);
    let mut cb = Cursor::new(0, b.end);
    let mut request = Request {
        loop_range: Some((2000, 4000)),
        ..Default::default()
    };
    let mut whole = vec![[0.; 2]; 12000];
    let mut pieces = whole.clone();
    ca.render(request, &mut whole, |at, out| a.render(at, out));
    for part in pieces.chunks_mut(137) {
        cb.render(request, part, |at, out| b.render(at, out));
    }
    assert_eq!(whole, pieces);
    assert_eq!(ca.frame(), cb.frame());
    assert!(whole[3500][1] > 0.49);
    assert_eq!(whole[4500][1], 0.);
    request.generation = 1;
    request.seek = 1000;
    request.loop_range = None;
    let mut seek = [[0.; 2]; 1];
    ca.render(request, &mut seek, |at, out| a.render(at, out));
    assert!(seek[0][0] > 0.24);
    assert_eq!(seek[0][1], 0.);
}

#[test]
fn mismatched_vst_rate_rejects_preparation_before_opening_a_stream() {
    let mut input = track(1, vec![clip([0.1, 0.], 0., 100)]);
    input.rack = Some(Rack::new(Plan::Gain(1.), 44100));
    assert!(
        Mix::prepare(
            vec![input],
            None,
            48000,
            120.,
            elevate::musical::time::Meter::default()
        )
        .err()
        .unwrap()
        .contains("VST3")
    );
}

#[test]
#[ignore = "requiere HAY_VST3_TEST_BUNDLE y un VST3 instalado"]
fn two_real_vst_instruments_share_one_mix_with_wav() {
    use crate::audio::{
        piano::VoiceSource,
        vst3::{SharedInstrument, VstEngine},
    };
    use elevate::musical::{
        clip::MidiClip,
        model::{InstrumentId, Pattern},
        time::{Meter, PPQ, Tick},
        track::{Track, TrackKind},
    };
    let bundle = std::env::var_os("HAY_VST3_TEST_BUNDLE")
        .expect("HAY_VST3_TEST_BUNDLE debe apuntar al instrumento .vst3");
    crate::audio::vst3::environment::configure().unwrap();
    let bundle = std::path::Path::new(&bundle);
    let mut inputs = Vec::new();
    let mut shared = Vec::new();
    for (index, pitch, pan) in [(0, 60, -1.), (1, 64, 1.)] {
        let id = InstrumentId(700 + index as u32);
        let engine = VstEngine::load_instrument_class(bundle, 48_000, None).unwrap();
        let vst = SharedInstrument::new(engine, 48_000);
        let mut track_state = Track::new(format!("VST {index}"), TrackKind::Synth);
        track_state.instrument = Some(id);
        let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
        pattern
            .add_note(id, Tick(0), Tick(PPQ / 2), pitch, 100)
            .unwrap();
        let midi = MidiClip::new("nota", index, 0., pattern);
        let mut input = track(track_state.id.0, vec![]);
        input.pan = pan;
        input.midi = Some(MidiTrackInput {
            index,
            track: track_state,
            instrument: id,
            clips: vec![midi],
            source: VoiceSource::Vst(vst.clone()),
            hilo: None,
        });
        inputs.push(input);
        shared.push(vst);
    }
    let mut wav = track(900, vec![clip([0.02, 0.02], 0., 48_000)]);
    wav.clips[0].track = 2;
    inputs.push(wav);
    let mut mix = Mix::prepare(inputs, None, 48_000, 120., Meter::default()).unwrap();
    for vst in &shared {
        vst.enter_arrangement().unwrap();
    }
    let mut peaks = [0_f32; 2];
    for index in 0..96 {
        let mut frames = [[0.; 2]; 256];
        mix.render_timed(index * 256, &mut frames, None);
        for frame in frames {
            assert!(frame.iter().all(|sample| sample.is_finite()));
            peaks[0] = peaks[0].max((frame[0] - 0.02).abs());
            peaks[1] = peaks[1].max((frame[1] - 0.02).abs());
        }
    }
    assert!(peaks[0] > 0.01 && peaks[1] > 0.01, "picos VST3: {peaks:?}");
    drop(mix);
    let editors: Vec<_> = shared.iter().map(|vst| vst.editor.clone()).collect();
    for vst in &shared {
        vst.leave_arrangement();
    }
    drop(shared);
    for editor in editors {
        editor.wait_for_shutdown();
    }
}

#[test]
fn wav_and_midi_on_one_track_reach_the_same_bus() {
    use crate::audio::piano::{PianoSettings, PianoSnapshot, VoiceSource};
    use elevate::musical::{
        clip::MidiClip,
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
        track::{Track, TrackKind},
    };
    let mut musical_track = Track::new("Keys", TrackKind::Synth);
    musical_track.instrument = Some(InstrumentId(41));
    let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
    pattern
        .add_note(InstrumentId(41), Tick(0), Tick(10), 60, 100)
        .unwrap();
    let midi = MidiClip::new("nota", 0, 0.0, pattern);
    let source = VoiceSource::Piano(PianoSnapshot {
        path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/soundfonts/UprightPianoKW-20220221.sf2"),
        settings: PianoSettings::default(),
        midi: vec![],
        master_volume: 0.7,
        filter: Default::default(),
    });
    let mut input = track(musical_track.id.0, vec![clip([0.1, 0.1], 0.0, 1024)]);
    input.midi = Some(MidiTrackInput {
        index: 0,
        track: musical_track,
        instrument: InstrumentId(41),
        clips: vec![midi],
        source,
        hilo: None,
    });
    let mut mix = Mix::prepare(
        vec![input],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut frames = [[0.0; 2]; 512];
    mix.render(0, &mut frames);
    assert!(frames.iter().any(|frame| (frame[0] - 0.1).abs() > 0.0001));
    assert!(
        frames
            .iter()
            .all(|frame| frame[0].is_finite() && frame[1].is_finite())
    );
}

#[test]
fn live_piano_note_joins_wav_without_a_midi_clip() {
    use crate::audio::piano::{PianoSettings, PianoSnapshot, VoiceSource};
    use elevate::musical::{
        midi::MidiMessage,
        model::InstrumentId,
        track::{Track, TrackKind},
    };

    let id = InstrumentId(41);
    let mut piano = Track::new("Keys", TrackKind::Synth);
    piano.instrument = Some(id);
    let mut instrument = track(piano.id.0, vec![]);
    instrument.midi = Some(MidiTrackInput {
        index: 0,
        track: piano,
        instrument: id,
        clips: vec![],
        source: VoiceSource::Piano(PianoSnapshot {
            path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/soundfonts/UprightPianoKW-20220221.sf2"),
            settings: PianoSettings::default(),
            midi: vec![],
            master_volume: 0.7,
            filter: Default::default(),
        }),
        hilo: None,
    });
    let wav = track(900, vec![clip([0.1, 0.1], 0., 48_000)]);
    let mut mix = Mix::prepare(
        vec![wav, instrument],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    assert_eq!(mix.instrument_ids(), vec![id]);
    let mut before = [[0.0; 2]; 256];
    mix.render(0, &mut before);
    assert!(before.iter().all(|frame| (frame[0] - 0.1).abs() < 0.0001));

    mix.live_midi(
        id,
        MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        },
    );
    let mut during = [[0.0; 2]; 1024];
    mix.render(256, &mut during);
    assert!(during.iter().any(|frame| (frame[0] - 0.1).abs() > 0.0001));
    assert!(
        during
            .iter()
            .all(|frame| frame[0].is_finite() && frame[1].is_finite())
    );

    mix.live_midi(id, MidiMessage::NoteOff { note: 60 });
    let mut after = [[0.0; 2]; 256];
    mix.render(1280, &mut after);
    assert!(
        after
            .iter()
            .all(|frame| frame[0].is_finite() && frame[1].is_finite())
    );
}

#[test]
fn two_sampler_instances_with_different_samples_mix_with_wav() {
    use crate::audio::{
        piano::VoiceSource,
        sampler::{SampleAsset, SamplerSnapshot},
    };
    use elevate::{
        devices::sampler::SamplerParams,
        musical::{
            clip::MidiClip,
            model::{InstrumentId, Pattern},
            time::{PPQ, Tick},
            track::{Track, TrackKind},
        },
    };
    fn sample(hz: f32) -> Arc<SampleAsset> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "hay-mix-test-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let frames = 12_000u32;
        let bytes = frames * 4;
        let mut wav = Vec::with_capacity(44 + bytes as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&48_000u32.to_le_bytes());
        wav.extend_from_slice(&192_000u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        for frame in 0..frames {
            let value =
                ((frame as f32 * hz * std::f32::consts::TAU / 48_000.0).sin() * 16_000.0) as i16;
            wav.extend_from_slice(&value.to_le_bytes());
            wav.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(&path, wav).unwrap();
        let asset = SampleAsset::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        asset
    }
    let build = |index: usize, id: u32, pan: f32, hz| {
        let instrument = InstrumentId(id);
        let mut musical_track = Track::new("Sampler", TrackKind::Synth);
        musical_track.instrument = Some(instrument);
        let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
        pattern
            .add_note(instrument, Tick(0), Tick(PPQ / 2), 60, 100)
            .unwrap();
        let mut input = track(musical_track.id.0, vec![]);
        input.pan = pan;
        input.midi = Some(MidiTrackInput {
            index,
            track: musical_track,
            instrument,
            clips: vec![MidiClip::new("nota", index, 0.0, pattern)],
            source: VoiceSource::Sampler(SamplerSnapshot {
                asset: sample(hz),
                params: SamplerParams::default(),
            }),
            hilo: None,
        });
        input
    };
    let mut mix = Mix::prepare(
        vec![
            build(0, 101, -1.0, 220.0),
            build(1, 102, 1.0, 440.0),
            track(103, vec![clip([0.1, 0.1], 0.0, 4_096)]),
        ],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut frames = vec![[0.0; 2]; 2_048];
    mix.render(0, &mut frames);
    assert!(frames[512..].iter().any(|f| (f[0] - 0.1).abs() > 0.0001));
    assert!(frames[512..].iter().any(|f| (f[1] - 0.1).abs() > 0.0001));
    assert!(frames[512..].iter().any(|f| (f[0] - f[1]).abs() > 0.001));
}

#[test]
fn layer_hilo_sums_two_soundfonts_and_wav_with_independent_branch_levels() {
    use crate::audio::piano::{PianoSettings, PianoSnapshot, VoiceSource};
    use elevate::musical::{
        clip::MidiClip,
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
        track::{HiloMode, Track, TrackKind},
    };
    let group = Track::new("Hilo", TrackKind::Group);
    let ids = [InstrumentId(101), InstrumentId(102)];
    let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
    pattern
        .add_note(ids[0], Tick(0), Tick(PPQ / 2), 60, 100)
        .unwrap();
    let group_clip = MidiClip::new("capa", 0, 0.0, pattern);
    let make_branch = |index: usize, instrument: InstrumentId, pan: f32| {
        let mut musical_track = Track::new("Piano", TrackKind::Synth);
        musical_track.parent = Some(group.id);
        musical_track.instrument = Some(instrument);
        let mut input = track(musical_track.id.0, vec![]);
        input.pan = pan;
        input.midi = Some(MidiTrackInput {
            index,
            track: musical_track,
            instrument,
            clips: vec![],
            source: VoiceSource::Piano(PianoSnapshot {
                path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/soundfonts/UprightPianoKW-20220221.sf2"),
                settings: PianoSettings::default(),
                midi: vec![],
                master_volume: 0.7,
                filter: Default::default(),
            }),
            hilo: Some(HiloBranchInput {
                clips: vec![group_clip.clone()],
                group_index: 0,
                branch_index: index - 1,
                instruments: ids.to_vec(),
                mode: HiloMode::Layer,
                weave: None,
            }),
        });
        input
    };
    let mut mix = Mix::prepare(
        vec![
            make_branch(1, ids[0], -1.0),
            make_branch(2, ids[1], 1.0),
            track(103, vec![clip([0.1, 0.1], 0.0, 20_000)]),
        ],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut before = vec![[0.0; 2]; 2_048];
    mix.render(0, &mut before);
    assert!(
        before[512..]
            .iter()
            .any(|frame| (frame[0] - 0.1).abs() > 0.0001)
    );
    assert!(
        before[512..]
            .iter()
            .any(|frame| (frame[1] - 0.1).abs() > 0.0001)
    );

    mix.tracks[0]
        .control
        .gain
        .store(0.0f32.to_bits(), Ordering::Relaxed);
    let mut after = vec![[0.0; 2]; 2_048];
    mix.render(2_048, &mut after);
    assert!(
        after[1_024..]
            .iter()
            .all(|frame| (frame[0] - 0.1).abs() < 0.001)
    );
    assert!(
        after[1_024..]
            .iter()
            .any(|frame| (frame[1] - 0.1).abs() > 0.0001)
    );
}

#[test]
fn round_robin_group_clip_alternates_audible_branches_on_play() {
    use crate::audio::piano::{PianoSettings, PianoSnapshot, VoiceSource};
    use elevate::musical::{
        clip::MidiClip,
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
        track::{HiloMode, Track, TrackKind},
    };
    let group = Track::new("Hilo", TrackKind::Group);
    let ids = [InstrumentId(151), InstrumentId(152)];
    let mut pattern = Pattern::new(Tick(2 * PPQ)).unwrap();
    for (index, pitch) in [60, 64].into_iter().enumerate() {
        pattern
            .add_note(ids[0], Tick(index as i64 * PPQ), Tick(PPQ / 4), pitch, 100)
            .unwrap();
    }
    let group_clip = MidiClip::new("Tejer", 0, 0.0, pattern);
    let inputs: Vec<_> = ids
        .into_iter()
        .enumerate()
        .map(|(branch_index, instrument)| {
            let mut musical_track = Track::new("Piano", TrackKind::Synth);
            musical_track.parent = Some(group.id);
            musical_track.instrument = Some(instrument);
            let mut input = track(musical_track.id.0, vec![]);
            input.pan = if branch_index == 0 { -1.0 } else { 1.0 };
            input.midi = Some(MidiTrackInput {
                index: branch_index + 1,
                track: musical_track,
                instrument,
                clips: vec![],
                source: VoiceSource::Piano(PianoSnapshot {
                    path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/soundfonts/UprightPianoKW-20220221.sf2"),
                    settings: PianoSettings {
                        maximum_polyphony: 64,
                        reverb_and_chorus: false,
                    },
                    midi: vec![],
                    master_volume: 0.7,
                    filter: Default::default(),
                }),
                hilo: Some(HiloBranchInput {
                    clips: vec![group_clip.clone()],
                    group_index: 0,
                    branch_index,
                    instruments: ids.to_vec(),
                    mode: HiloMode::RoundRobin,
                    weave: None,
                }),
            });
            input
        })
        .collect();
    let mut mix = Mix::prepare(
        inputs,
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut first = [[0.0; 2]; 2048];
    mix.render(0, &mut first);
    assert!(first.iter().any(|frame| frame[0].abs() > 0.001));
    assert!(first.iter().all(|frame| frame[1].abs() < 1e-6));
    let mut second = [[0.0; 2]; 2048];
    mix.render(24_000, &mut second);
    assert!(second.iter().any(|frame| frame[1].abs() > 0.001));
    // Retirar la voz derecha durante su nota conserva su release, sin cortar el bus común.
    let mut removed = [false; MAX_TRACKS];
    removed[1] = true;
    assert_eq!(mix.begin_retirements(&removed), 480_000);
    let mut tail = [[0.; 2]; 1024];
    mix.render(26_048, &mut tail);
    assert!(tail.iter().any(|frame| frame[1].abs() > 0.0001));
    assert_eq!(mix.begin_retirements(&removed), 480_000 - 1024);

    // Una vuelta del loop sigue tocando la rama izquierda. La derecha no se reactiva
    // al llegar a su próximo NoteOn del clip ni al recibir MIDI directo.
    mix.render(0, &mut first);
    assert!(first.iter().any(|frame| frame[0].abs() > 0.001));
    mix.live_midi(
        ids[1],
        MidiMessage::NoteOn {
            note: 72,
            velocity: 127,
        },
    );
    let mut rest = vec![[0.; 2]; 480_000 - 1024 - first.len() + 64];
    mix.render(24_000, &mut rest);
    assert!(
        rest[rest.len() - 64..]
            .iter()
            .all(|frame| frame[1].abs() < 1e-6)
    );
}

#[test]
fn drums_and_wav_use_the_same_frame_clock() {
    use elevate::musical::{
        drums::{DrumKit, DrumPiece},
        model::{InstrumentId, Pattern},
        time::{PPQ, Tick},
    };
    let piece = InstrumentId(1);
    let mut kit = DrumKit::default();
    kit.pieces.push(DrumPiece {
        id: piece,
        name: "Kick".into(),
        layers: Vec::new(),
        sampler: Default::default(),
        choke_group: None,
        mix: Default::default(),
        timing_offset: 0,
        timing_mode: elevate::musical::drums::OffsetMode::Fixed,
    });
    let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
    pattern
        .add_note(piece, Tick(0), Tick(PPQ / 2), 60, 100)
        .unwrap();
    let mut mix = Mix::prepare(
        vec![track(1, vec![clip([0.1, 0.1], 0.0, 48_000)])],
        Some(DrumInput {
            score: Some(super::super::super::drums::DrumScore {
                pattern,
                repeat: true,
            }),
            kit,
        }),
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    assert!(mix.drum_io.as_ref().unwrap().0.state.read(DrumBlock::loops));
    let mut frames = vec![[0.0; 2]; 1024];
    mix.render(0, &mut frames);
    assert!(frames.iter().any(|f| (f[0] - 0.1).abs() > 0.001));
}

#[test]
fn track_automation_ramps_volume_and_pan_across_blocks() {
    use elevate::musical::track_automation::TrackAutomation;
    let mut auto = TrackAutomation::default();
    // 120 BPM @ 48 kHz => 24 000 frames por negra.
    // Volumen de 0.0 (en beat 0) a 0.75 (unidad 1.0x en beat 2).
    auto.volume.upsert(0.0, 0.0);
    auto.volume.upsert(2.0, 0.75);
    // Pan de izquierda total (0.0 => -1.0) en beat 0 a derecha total (1.0 => +1.0) en beat 2.
    auto.pan.upsert(0.0, 0.0);
    auto.pan.upsert(2.0, 1.0);

    let mut input = track(1, vec![clip([0.5, 0.5], 0.0, 96_000)]);
    input.automation = auto;
    let mut mix = Mix::prepare(
        vec![input],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();

    // En beat 0 el volumen automatizado es 0.0 (tras suavizado del primer bloque converge a ~0).
    let mut at_start = vec![[0.0; 2]; 1024];
    mix.render(0, &mut at_start);
    let tail_start = at_start[1023];
    assert!(tail_start[0].abs() < 0.05 && tail_start[1].abs() < 0.05);

    // En beat 2 (frame 48_000) el volumen es 1.0x y el pan es derecha total (+1.0 => L=0, R=0.5).
    let mut at_end = vec![[0.0; 2]; 1024];
    mix.render(48_000, &mut at_end);
    let tail_end = at_end[1023];
    assert!(
        tail_end[0].abs() < 1e-4,
        "canal L debe estar silenciado con pan R: {:?}",
        tail_end
    );
    assert!(
        (tail_end[1] - 0.5).abs() < 0.02,
        "canal R debe estar cerca de 0.5 con volumen 0 dB y pan R: {:?}",
        tail_end
    );
}

#[test]
fn track_automation_modulates_return_sends_and_lua_fx_parameters() {
    use elevate::musical::{
        fx::FxId,
        track_automation::{AutomationParam, TrackAutomation},
    };
    // 1. Envío Return automatizado desde 0% estático: beat 0 -> 0.0, beat 2 -> 1.0.
    let mut source = track(1, vec![clip([0.4, 0.4], 0.0, 96_000)]);
    let mut auto = TrackAutomation::default();
    auto.lane_mut(AutomationParam::Send(TrackId(2)))
        .upsert(0.0, 0.0);
    auto.lane_mut(AutomationParam::Send(TrackId(2)))
        .upsert(2.0, 1.0);
    source.automation = auto;

    let mut ret = track(2, vec![]);
    ret.is_return = true;
    ret.rack = Some(Rack::new(Plan::Gain(0.5), 0));

    let mut mix = Mix::prepare(
        vec![source, ret],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut start = vec![[0.0; 2]; 512];
    mix.render(0, &mut start);
    assert!(
        (start[511][0] - 0.4).abs() < 0.02,
        "en beat 0 el envío automatizado es 0%: {:?}",
        start[511]
    );
    let mut end = vec![[0.0; 2]; 512];
    mix.render(48_000, &mut end);
    assert!(
        (end[511][0] - 0.6).abs() < 0.02,
        "en beat 2 el envío es 100% hacia Return 0.5x => 0.4 + 0.2 = 0.6: {:?}",
        end[511]
    );

    // 2. Parámetro de componente Lua/Hay automatizado en el rack FX.
    let descriptor = Arc::new(
        elevate::scripting::audio_components::parse(include_str!(
            "../../../../assets/scripts/components/audio/double_volume.lua"
        ))
        .unwrap(),
    );
    let fx_id = FxId(77);
    let component = ComponentEffect::prepare(descriptor, 48_000).unwrap();
    let mut fx_track = track(3, vec![clip([0.5, 0.5], 0.0, 96_000)]);
    fx_track.rack = Some(Rack::new(Plan::Component(fx_id, component, None), 0));
    let mut fx_auto = TrackAutomation::default();
    fx_auto
        .lane_mut(AutomationParam::FxParam {
            fx: fx_id,
            param: 0,
        })
        .upsert(0.0, 0.0);
    fx_auto
        .lane_mut(AutomationParam::FxParam {
            fx: fx_id,
            param: 0,
        })
        .upsert(2.0, 0.5);
    fx_track.automation = fx_auto;

    let mut fx_mix = Mix::prepare(
        vec![fx_track],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut fx_start = vec![[0.0; 2]; 2048];
    fx_mix.render(0, &mut fx_start);
    assert!(
        fx_start[2047][0].abs() < 0.05,
        "en beat 0 el parámetro FX automatizado atenúa a ~0: {:?}",
        fx_start[2047]
    );
    let mut fx_end = vec![[0.0; 2]; 2048];
    fx_mix.render(48_000, &mut fx_end);
    assert!(
        fx_end[2047][0] > 0.35,
        "en beat 2 el parámetro FX automatizado sube la ganancia: {:?}",
        fx_end[2047]
    );
}

#[test]
fn parallel_pool_produces_exact_same_samples_as_sequential_render() {
    use elevate::musical::track_automation::TrackAutomation;
    let make_inputs = || {
        let mut group = track(100, vec![]);
        group.rack = Some(Rack::new(Plan::Gain(0.8), 0));
        let mut inputs = vec![group];
        for i in 1..=8u32 {
            let mut t = track(
                i,
                vec![clip([0.05 * i as f32, -0.03 * i as f32], 0.0, 4096)],
            );
            if i <= 4 {
                t.parent = Some(TrackId(100));
            }
            let mut auto = TrackAutomation::default();
            auto.volume.upsert(0.0, 0.25 + 0.05 * i as f32);
            auto.volume.upsert(1.0, 0.85);
            auto.pan.upsert(0.0, (i as f32) / 9.0);
            t.automation = auto;
            inputs.push(t);
        }
        inputs
    };

    let mut parallel_mix = Mix::prepare(
        make_inputs(),
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    let mut seq_mix = Mix::prepare(
        make_inputs(),
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();
    seq_mix.disable_pool();

    let mut out_par = vec![[0.0; 2]; 4096];
    let mut out_seq = vec![[0.0; 2]; 4096];
    parallel_mix.render(0, &mut out_par);
    seq_mix.render(0, &mut out_seq);
    assert_eq!(out_par, out_seq);
    assert!(out_par.iter().any(|f| f[0].abs() > 0.05));
}

#[test]
fn send_return_routes_track_post_fader_through_return_fx_into_master() {
    let mut source = track(1, vec![clip([0.5, 0.5], 0.0, 2048)]);
    source.gain = 0.5;
    source.sends = vec![(TrackId(2), 0.5)];

    let mut ret = track(2, vec![]);
    ret.is_return = true;
    ret.rack = Some(Rack::new(Plan::Gain(2.0), 0));
    ret.pan = 1.0; // 100% derecha

    let mut mix = Mix::prepare(
        vec![source, ret],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();

    // Pista 1 post-fader = [0.25, 0.25].
    // Envío 0.5 → Return 2 recibe [0.125, 0.125], FX 2.0x → [0.25, 0.25], pan R → [0.0, 0.25].
    // Suma en master = [0.25, 0.50].
    let mut out = vec![[0.0; 2]; 512];
    mix.render(0, &mut out);
    let tail = out[511];
    assert!(
        (tail[0] - 0.25).abs() < 1e-4 && (tail[1] - 0.50).abs() < 1e-4,
        "esperado [0.25, 0.50], obtenido {:?}",
        tail
    );

    // Actualizar envíos en vivo sin bloqueos (quitar envío a 0%) → solo queda la señal seca.
    mix.tracks[0].control.sends.publish(Vec::new());
    let mut dry_only = vec![[0.0; 2]; 512];
    mix.render(512, &mut dry_only);
    let tail_dry = dry_only[511];
    assert!(
        (tail_dry[0] - 0.25).abs() < 1e-4 && (tail_dry[1] - 0.25).abs() < 1e-4,
        "esperado solo señal seca [0.25, 0.25], obtenido {:?}",
        tail_dry
    );
}

#[test]
fn sidechain_compressor_ducks_target_track_from_source_track() {
    use elevate::musical::fx::{FxId, SidechainSource};

    let descriptor = Arc::new(
        elevate::scripting::audio_components::parse(include_str!(
            "../../../../assets/scripts/components/audio/compressor.lua"
        ))
        .unwrap(),
    );
    let comp = ComponentEffect::prepare(descriptor, 48_000).unwrap();
    comp.set(0, -24.0).unwrap(); // Threshold -24 dB
    comp.set(1, 10.0).unwrap(); // Ratio 10:1
    comp.set(2, 0.5).unwrap(); // Attack rápido 0.5 ms
    comp.set(3, 50.0).unwrap(); // Release 50 ms
    comp.set(4, 0.0).unwrap(); // Hard knee
    comp.set(6, 100.0).unwrap(); // 100% wet

    // Pista 1 (Kick / Key): señal fuerte en los primeros 4096 frames, luego silencio.
    let key = track(1, vec![clip([0.9, 0.9], 0.0, 4096)]);
    // Pista 2 (Bajo): señal suave constante (-26 dB ~ 0.05) por debajo del umbral interno,
    // así que solo puede comprimirse si recibe realmente el sidechain de la Pista 1.
    let mut bass = track(2, vec![clip([0.05, 0.05], 0.0, 16_384)]);
    bass.rack = Some(Rack::new(
        Plan::Component(
            FxId(42),
            comp.clone(),
            Some(SidechainSource::Track(TrackId(1))),
        ),
        0,
    ));

    let mut mix = Mix::prepare(
        vec![key, bass],
        None,
        48_000,
        120.0,
        elevate::musical::time::Meter::default(),
    )
    .unwrap();

    // Durante el golpe del Kick (Pista 1), el Bajo (Pista 2) debe atenuarse fuertemente.
    let mut stems = StemBuffers::from([
        (Stem::Track(TrackId(1)), [[0.0; 2]; 256]),
        (Stem::Track(TrackId(2)), [[0.0; 2]; 256]),
    ]);
    let mut master = [[0.0; 2]; 256];
    for block_idx in 0..8 {
        mix.render_block(
            block_idx * 256,
            &mut master,
            None,
            None,
            false,
            Some(&mut stems),
        );
    }
    let ducked = stems[&Stem::Track(TrackId(2))][255][0].abs();
    let gr_while_key = comp.gain_reduction_db();
    assert!(
        ducked < 0.02,
        "bajo debía comprimirse por sidechain (< 0.02), obtenido {ducked}"
    );
    assert!(
        gr_while_key > 6.0,
        "medidor GR debía reportar reducción > 6 dB, obtenido {gr_while_key}"
    );

    // Cuando la Pista 1 deja de sonar (frame >= 4096), el Bajo se recupera cerca de 0.05.
    for block_idx in 16..48 {
        mix.render_block(
            block_idx * 256,
            &mut master,
            None,
            None,
            false,
            Some(&mut stems),
        );
    }
    let recovered = stems[&Stem::Track(TrackId(2))][255][0].abs();
    assert!(
        recovered > 0.04,
        "bajo debía recuperarse sin key (> 0.04), obtenido {recovered}"
    );
}
