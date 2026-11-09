//! Dispositivo propio construido desde su descriptor; no necesita UI ni salida de audio.
use elevate::{
    Primitive, ProcessSpec,
    devices::{
        self,
        utility::Utility,
        utility_slot::{UtilityAction, UtilitySlot, UtilityState},
    },
    primitives::amplitude::Volume,
};

fn spec() -> ProcessSpec {
    ProcessSpec::new(48_000.0).unwrap()
}

fn settle(slot: &mut elevate::devices::utility_slot::PreparedUtility, input: [f32; 2]) -> [f32; 2] {
    let mut block = [input; 8192];
    slot.process(&mut block);
    block[8191]
}

fn close(actual: [f32; 2], expected: [f32; 2]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!((a - e).abs() < 1e-5, "{a} != {e}");
    }
}

#[test]
fn registered_factory_builds_independent_components_with_real_volume_processing() {
    let descriptor = devices::find("hay.utility").unwrap();
    assert_eq!(
        descriptor.id,
        elevate::catalog::find("hay.utility").unwrap().id
    );
    let mut a = (descriptor.create)(spec()).unwrap();
    let mut b = (descriptor.create)(spec()).unwrap();
    a.set_parameter(Utility::GAIN, 0.5).unwrap();
    b.set_parameter(Utility::GAIN, 1.5).unwrap();
    let mut left = [[0.4, -0.2]; 8192];
    let mut right = left;
    a.process(&mut left);
    b.process(&mut right);
    close(left[8191], [0.2, -0.1]);
    close(right[8191], [0.6, -0.3]);
    a.set_bypassed(true).unwrap();
    left.fill([0.4, -0.2]);
    a.process(&mut left);
    close(left[8191], [0.4, -0.2]);
    a.set_bypassed(false).unwrap();
    left.fill([0.4, -0.2]);
    a.process(&mut left);
    close(left[8191], [0.2, -0.1]);
}

#[test]
fn output_slot_inserts_changes_bypasses_and_removes_without_rebuilding_stream() {
    let slot = UtilitySlot::new();
    assert_eq!(slot.state(), UtilityState::default());
    let mut dsp = slot.prepare(spec()).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.4, -0.4]);
    slot.apply(UtilityAction::SetGain(0.25)).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.1, -0.1]);
    slot.apply(UtilityAction::ToggleBypass).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.4, -0.4]);
    slot.apply(UtilityAction::SetGain(0.5)).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.4, -0.4]);
    slot.apply(UtilityAction::ToggleBypass).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.2, -0.2]);
    slot.apply(UtilityAction::Remove).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.4, -0.4]);
    assert_eq!(slot.state(), UtilityState::default());
    slot.apply(UtilityAction::Insert).unwrap();
    close(settle(&mut dsp, [0.4, -0.4]), [0.4, -0.4]);
}

#[test]
fn bypass_cannot_undo_upstream_track_mute_or_fader() {
    let slot = UtilitySlot::new();
    slot.apply(UtilityAction::SetGain(2.0)).unwrap();
    slot.apply(UtilityAction::ToggleBypass).unwrap();
    let mut dsp = slot.prepare(spec()).unwrap();
    for (gain, expected) in [(0.0, [0.0, 0.0]), (0.5, [0.2, -0.2])] {
        let mut track = Volume::with_gain(spec(), gain).unwrap();
        let mut block = [[0.4, -0.4]; 8192];
        track.process(&mut block);
        dsp.process(&mut block);
        close(block[8191], expected);
    }
}

#[test]
fn invalid_parameters_do_not_publish_and_state_survives_stream_recreation() {
    let slot = UtilitySlot::default();
    slot.apply(UtilityAction::SetGain(0.75)).unwrap();
    let state = slot.state();
    for value in [f32::NAN, f32::INFINITY, -1.0, 3.0] {
        assert!(slot.apply(UtilityAction::SetGain(value)).is_err());
        assert_eq!(slot.state(), state);
    }
    let mut first = slot.prepare(spec()).unwrap();
    let mut reopened = slot.prepare(ProcessSpec::new(96_000.0).unwrap()).unwrap();
    close(settle(&mut first, [0.4, -0.4]), [0.3, -0.3]);
    close(settle(&mut reopened, [0.4, -0.4]), [0.3, -0.3]);
    slot.apply(UtilityAction::Reset).unwrap();
    close(settle(&mut first, [0.4, -0.4]), [0.4, -0.4]);
}

#[test]
fn silent_utility_is_silent_from_the_first_sample_on_stream_start() {
    let slot = UtilitySlot::new();
    slot.apply(UtilityAction::SetGain(0.0)).unwrap();
    let mut processor = slot.prepare(spec()).unwrap();
    let mut block = [[0.5, -0.5]; 32];
    processor.process(&mut block);
    assert!(block.iter().all(|frame| *frame == [0.0, 0.0]));
}
