use elevate::primitives::amplitude::Volume;
use elevate::{
    Component, ComponentError, ControlEvent, NodeId, ParameterId, Primitive, PrimitiveError,
    ProcessSpec, SignalProperty, StereoFrame,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn assert_frames_close(actual: &[StereoFrame], expected: &[StereoFrame]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().flatten().zip(expected.iter().flatten()) {
        assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
    }
}

#[test]
fn volume_scales_both_channels_without_clipping_or_changing_polarity() -> TestResult {
    let input = [[0.75, -0.25], [-0.5, 0.8], [0.0, 0.0]];
    for sample_rate in [44_100.0, 48_000.0, 96_000.0] {
        let specification = ProcessSpec::new(sample_rate)?;
        for gain in [0.0, 0.5, 1.0, 2.0] {
            let mut volume = Volume::with_gain(specification, gain)?;
            let mut output = input;
            volume.process(&mut output);
            let expected = input.map(|[left, right]| [left * gain, right * gain]);
            assert_frames_close(&output, &expected);
        }
    }
    Ok(())
}

#[test]
fn parameters_describe_the_same_contract_used_by_the_processor() -> TestResult {
    let mut volume = Volume::new(ProcessSpec::new(48_000.0)?);
    assert_eq!(volume.info().properties, &[SignalProperty::Amplitude]);
    for parameter in volume.info().parameters {
        assert_eq!(volume.parameter(parameter.id)?, parameter.default);
        volume.set_parameter(parameter.id, parameter.min)?;
        assert_eq!(volume.parameter(parameter.id)?, parameter.min);
        volume.set_parameter(parameter.id, parameter.max)?;
        assert_eq!(volume.parameter(parameter.id)?, parameter.max);
    }
    Ok(())
}

#[test]
fn invalid_parameters_are_rejected_without_corrupting_audio() -> TestResult {
    let specification = ProcessSpec::new(48_000.0)?;
    let mut volume = Volume::with_gain(specification, 0.5)?;
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 2.01] {
        assert!(Volume::with_gain(specification, value).is_err());
        assert!(volume.set_parameter(Volume::GAIN, value).is_err());
        assert_eq!(volume.gain(), 0.5);
    }
    let unknown = ParameterId(999);
    assert_eq!(
        volume.set_parameter(unknown, 1.0),
        Err(PrimitiveError::UnknownParameter(unknown))
    );
    assert_eq!(
        volume.parameter(unknown),
        Err(PrimitiveError::UnknownParameter(unknown))
    );
    let mut output = [[1.0, -0.5]];
    volume.process(&mut output);
    assert_frames_close(&output, &[[0.5, -0.25]]);
    Ok(())
}

#[test]
fn sample_rate_is_validated_before_dsp_construction() {
    for sample_rate in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(ProcessSpec::new(sample_rate).is_err());
    }
}

#[test]
fn gain_changes_are_smoothed_in_seconds_and_keep_channels_linked() -> TestResult {
    for sample_rate in [44_100.0, 48_000.0, 96_000.0] {
        let mut volume = Volume::new(ProcessSpec::new(sample_rate)?);
        volume.process(&mut [[1.0, -0.5]]);
        volume.set_gain(0.0)?;

        let mut output = vec![[1.0, -0.5]; (sample_rate * 0.1) as usize];
        volume.process(&mut output);
        let first = output.first().ok_or("Falta la primera muestra")?;
        assert!(first[0] > 0.99);
        let halfway = output
            .get((sample_rate * 0.005).round() as usize - 1)
            .ok_or("Falta la muestra de respuesta media")?;
        // FunDSP define response_time como tiempo hasta la mitad del cambio, no hasta el final.
        assert!((halfway[0] - 0.5).abs() < 0.02);
        assert!(output.last().ok_or("Falta la última muestra")?[0] < 1e-5);
        for [left, right] in &output {
            assert!((0.0..=1.0).contains(left));
            assert!((right + left * 0.5).abs() < 1e-6);
        }
        for pair in output.windows(2) {
            assert!(pair[1][0] <= pair[0][0] + 1e-6);
        }
    }
    Ok(())
}

#[test]
fn smoothing_does_not_restart_at_block_boundaries() -> TestResult {
    let specification = ProcessSpec::new(48_000.0)?;
    let mut whole = Volume::with_gain(specification, 0.25)?;
    let mut partitioned = Volume::with_gain(specification, 0.25)?;
    whole.process(&mut [[1.0, 0.25]; 8]);
    partitioned.process(&mut [[1.0, 0.25]; 8]);
    whole.set_gain(1.5)?;
    partitioned.set_gain(1.5)?;

    let mut whole_output = [[1.0, 0.25]; 509];
    let mut partitioned_output = whole_output;
    whole.process(&mut whole_output);
    for block in partitioned_output.chunks_mut(17) {
        partitioned.process(&mut []);
        partitioned.process(block);
    }
    assert_eq!(whole_output, partitioned_output);
    Ok(())
}

#[test]
fn bypass_preserves_the_parameter_and_smoothly_returns_to_it() -> TestResult {
    let mut volume = Volume::with_gain(ProcessSpec::new(48_000.0)?, 0.25)?;
    volume.process(&mut [[1.0, -0.5]]);
    volume.set_bypassed(true);
    assert!(volume.is_bypassed());
    volume.set_gain(0.5)?;
    assert_eq!(volume.gain(), 0.5);

    let mut output = [[1.0, -0.5]; 4800];
    volume.process(&mut output);
    assert!((output[0][0] - 0.25).abs() < 0.001);
    assert_frames_close(&output[4799..], &[[1.0, -0.5]]);

    volume.set_bypassed(false);
    assert!(!volume.is_bypassed());
    output.fill([1.0, -0.5]);
    volume.process(&mut output);
    assert!(output[0][0] > 0.99);
    assert_frames_close(&output[4799..], &[[0.5, -0.25]]);
    Ok(())
}

#[test]
fn reset_starts_at_the_requested_gain_without_resetting_parameters() -> TestResult {
    let mut volume = Volume::new(ProcessSpec::new(44_100.0)?);
    volume.process(&mut [[1.0, -1.0]]);
    volume.set_gain(0.5)?;
    volume.prepare(ProcessSpec::new(96_000.0)?);
    let mut output = [[1.0, -1.0]];
    volume.process(&mut output);
    assert_eq!(volume.gain(), 0.5);
    assert_frames_close(&output, &[[0.5, -0.5]]);
    Ok(())
}

#[test]
fn serial_component_applies_events_to_the_addressed_instance() -> TestResult {
    let specification = ProcessSpec::new(48_000.0)?;
    let mut component = Component::new("Amplitud", specification);
    let first = component.add(Box::new(Volume::new(specification)))?;
    let second = component.add(Box::new(Volume::with_gain(specification, 0.5)?))?;
    component.apply(ControlEvent::SetParameter {
        node: first,
        parameter: Volume::GAIN,
        value: 0.5,
    })?;
    let mut output = [[1.0, -0.5], [-0.25, 0.75]];
    component.process(&mut output);
    assert_frames_close(&output, &[[0.25, -0.125], [-0.0625, 0.1875]]);

    component.apply(ControlEvent::SetBypassed {
        node: second,
        bypassed: true,
    })?;
    component.reset();
    let mut output = [[1.0, -0.5]];
    component.process(&mut output);
    assert_frames_close(&output, &[[0.5, -0.25]]);
    Ok(())
}

#[test]
fn node_addresses_survive_removal_and_are_not_reused() -> TestResult {
    let specification = ProcessSpec::new(48_000.0)?;
    let mut component = Component::new("Amplitud", specification);
    let first = component.add(Box::new(Volume::new(specification)))?;
    let second = component.add(Box::new(Volume::new(specification)))?;
    assert!(component.remove(first).is_some());
    let third = component.add(Box::new(Volume::new(specification)))?;
    assert_ne!(first, third);
    assert_eq!(
        component.nodes().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![second, third]
    );
    component.apply(ControlEvent::SetParameter {
        node: second,
        parameter: Volume::GAIN,
        value: 0.5,
    })?;
    let mut output = [[1.0, -0.5]];
    component.process(&mut output);
    assert_frames_close(&output, &[[0.5, -0.25]]);
    Ok(())
}

#[test]
fn component_errors_are_visible_and_empty_components_pass_through() -> TestResult {
    let specification = ProcessSpec::new(48_000.0)?;
    let mut component = Component::new("Vacío", specification);
    let input = [[0.25, -0.75]];
    let mut output = input;
    component.process(&mut []);
    component.process(&mut output);
    assert_eq!(output, input);
    let missing = NodeId(999);
    assert_eq!(
        component.apply(ControlEvent::SetBypassed {
            node: missing,
            bypassed: true
        }),
        Err(ComponentError::UnknownNode(missing)),
    );
    let node = component.add(Box::new(Volume::new(specification)))?;
    assert!(
        component
            .apply(ControlEvent::SetParameter {
                node,
                parameter: Volume::GAIN,
                value: f32::NAN,
            })
            .is_err()
    );
    assert_eq!(
        component.apply(ControlEvent::SetParameter {
            node,
            parameter: ParameterId(999),
            value: 0.5
        }),
        Err(ComponentError::Primitive(PrimitiveError::UnknownParameter(
            ParameterId(999)
        ))),
    );
    component.process(&mut output);
    assert_eq!(output, input);
    Ok(())
}
