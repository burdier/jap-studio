use super::*;
use crate::audio::{
    AudioFile,
    effects::{ComponentEffect, Plan, Rack},
};
use elevate::musical::clip::ClipProps;
fn clip(value: [f32; 2], start_beat: f64, frames: usize) -> AudioClip {
    AudioClip {
        name: "fixture".into(),
        file: Arc::new(AudioFile {
            name: "fixture".into(),
            sample_rate: 48000,
            frames: vec![value; frames],
            peaks: vec![],
            max_peak: 1.,
        }),
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
        clips,
        gain: 1.,
        pan: 0.,
        fx_gain: 1.,
        rack: None,
    }
}
#[test]
fn shared_mix_sums_tracks_with_headroom_and_clamps_only_at_output() {
    let mut mix = Mix::prepare(
        vec![
            track(1, vec![clip([0.8, -0.8], 0., 12000)]),
            track(2, vec![clip([0.7, -0.7], 0., 12000)]),
        ],
        48000,
        120.,
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
    let mut whole = Mix::prepare(input(), 48000, 120.).unwrap();
    let mut divided = Mix::prepare(input(), 48000, 120.).unwrap();
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
    filtered.rack = Some(Rack::new(Plan::Component(filter), 0));
    let mut mix = Mix::prepare(
        vec![filtered, track(2, vec![clip([0., 0.4], 0., 12000)])],
        48000,
        120.,
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
    let mut mix = Mix::prepare(inputs, 48000, 120.).unwrap();
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
