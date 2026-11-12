use elevate::{
    ParameterId, ProcessSpec,
    devices::{Device, audio_component::AudioComponentDevice},
    primitives::{Primitive, filter::Filter},
    scripting::audio_components::parse,
};
use std::sync::Arc;
const SOURCE: &str = include_str!("../assets/scripts/components/audio/filter.lua");
fn instance() -> AudioComponentDevice {
    AudioComponentDevice::new(
        Arc::new(parse(SOURCE).unwrap()),
        ProcessSpec::new(48000.).unwrap(),
    )
    .unwrap()
}
fn sine(hz: f32, rate: f32) -> Vec<[f32; 2]> {
    (0..rate as usize)
        .map(|i| {
            [
                (std::f32::consts::TAU * hz * i as f32 / rate).sin() * 0.1,
                0.,
            ]
        })
        .collect()
}
fn energy(frames: &[[f32; 2]]) -> f32 {
    frames[frames.len() / 2..]
        .iter()
        .map(|f| f[0] * f[0])
        .sum::<f32>()
        / (frames.len() / 2) as f32
}
#[test]
fn lua_filter_matches_native_audio_and_instances_remain_independent() {
    let mut component = instance();
    let mut untouched = instance();
    component.set_parameter(ParameterId(0), 300.).unwrap();
    let mut native = Filter::new(ProcessSpec::new(48000.).unwrap());
    native.set_parameter(ParameterId(0), 300.).unwrap();
    let input = sine(4000., 48000.);
    let mut actual = input.clone();
    let mut expected = input.clone();
    let mut other = input.clone();
    component.process(&mut actual);
    native.process(&mut expected);
    untouched.process(&mut other);
    assert_eq!(actual, expected);
    assert!(energy(&other) > energy(&actual) * 20.);
    component.set_bypassed(true).unwrap();
    let mut dry = input.clone();
    component.process(&mut dry);
    assert_eq!(dry, input);
    assert!(component.set_parameter(ParameterId(2), 0.5).is_err());
}
#[test]
fn modes_and_response_curve_match_measured_gain_at_two_sample_rates() {
    for rate in [44100., 96000.] {
        for mode in 0..3 {
            let mut native = Filter::new(ProcessSpec::new(rate).unwrap());
            native.set_parameter(ParameterId(2), mode as f32).unwrap();
            native.reset();
            let points = Filter::response(rate, [1000., 0.707, mode as f32]);
            for index in [30, 90, 135] {
                let (fraction, db) = points[index];
                let hz = 20. * 1000_f32.powf(fraction);
                native.reset();
                let mut frames = sine(hz, rate as f32);
                let input = energy(&frames);
                native.process(&mut frames);
                let measured = 10. * (energy(&frames) / input).log10();
                assert!(
                    (measured.clamp(-60., 24.) - db).abs() < 0.12,
                    "mode={mode} rate={rate} hz={hz}: {measured} vs {db}"
                );
                assert!(frames.iter().all(|f| f[1] == 0. && f[0].is_finite()));
            }
        }
    }
}
#[test]
fn automation_is_partition_invariant_and_invalid_parameters_preserve_state() {
    let mut a = instance();
    let mut b = instance();
    for (id, value) in [(0, 20000.), (1, 10.), (2, 2.)] {
        a.set_parameter(ParameterId(id), value).unwrap();
        b.set_parameter(ParameterId(id), value).unwrap();
    }
    assert!(a.set_parameter(ParameterId(0), f32::NAN).is_err());
    let mut whole = sine(9000., 48000.);
    let mut chunks = whole.clone();
    a.process(&mut whole);
    for chunk in chunks.chunks_mut(37) {
        b.process(chunk);
    }
    assert_eq!(whole, chunks);
    assert!(whole.iter().flatten().all(|v| v.is_finite()));
}
#[test]
fn descriptor_versions_ranges_choices_and_response_targets_are_validated() {
    for invalid in [
        SOURCE.replace("api_version = 2", "api_version = 1"),
        SOURCE.replace("max = 20000", "max = 30000"),
        SOURCE.replace("mode = 0", "mode = 0.5"),
        SOURCE.replace("response_node = \"filter\"", "response_node = \"missing\""),
        SOURCE.replace("kind = \"choice\"", "kind = \"number\""),
    ] {
        assert!(parse(&invalid).is_err());
    }
    let descriptor = parse(SOURCE).unwrap();
    let cutoff = &descriptor.controls[0];
    assert!((cutoff.from_fraction(0.5) - (20_f32 * 20000.).sqrt()).abs() < 0.01);
    assert!((cutoff.fraction(1000.) - 0.56632334).abs() < 0.0001);
    assert!(!descriptor.controls[2].accepts(0.5));
    parse(include_str!(
        "../assets/scripts/components/audio/double_volume.lua"
    ))
    .unwrap();
}
