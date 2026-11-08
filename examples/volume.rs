use elevate::primitives::amplitude::Volume;
use elevate::{Component, ControlEvent, ProcessSpec, StereoFrame};
use fundsp::prelude64::sine_hz;

fn peak(frames: &[StereoFrame]) -> f32 {
    frames
        .iter()
        .flatten()
        .fold(0.0, |peak, sample| peak.max(sample.abs()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let specification = ProcessSpec::new(48_000.0)?;
    let mut oscillator = sine_hz(440.0);
    oscillator.set_sample_rate(specification.sample_rate());
    let input: Vec<StereoFrame> = (0..4800)
        .map(|_| {
            let sample = oscillator.get_mono();
            [sample * 0.8, sample * 0.4]
        })
        .collect();

    let mut component = Component::new("Demostración de amplitud", specification);
    let volume = component.add(Box::new(Volume::new(specification)))?;
    component.apply(ControlEvent::SetParameter {
        node: volume,
        parameter: Volume::GAIN,
        value: 0.5,
    })?;

    let mut output = input.clone();
    for block in output.chunks_mut(64) {
        component.process(block);
    }
    println!("Elevate / FunDSP — procesamiento offline, sin dispositivo de audio");
    println!("Entrada: pico {:.3}", peak(&input));
    println!("Entrada → Volume(0.5) → salida: pico {:.3}", peak(&output));

    component.add(Box::new(Volume::with_gain(specification, 0.5)?))?;
    output.copy_from_slice(&input);
    for block in output.chunks_mut(64) {
        component.process(block);
    }
    println!(
        "Entrada → Volume(0.5) → Volume(0.5) → salida: pico {:.3}",
        peak(&output)
    );
    for (id, primitive) in component.nodes() {
        println!(
            "Nodo {}: {} / {:?}",
            id.0,
            primitive.info().id,
            primitive.info().properties
        );
    }
    Ok(())
}
