//! Transforma únicamente snapshots del worker; jamás el clip editable ni RT.
use crate::audio::TrackInput;
use elevate::scripting::live_preplay::transform_pattern;

pub(super) fn transform_inputs(inputs: &mut [TrackInput], source: &str) -> Result<(), String> {
    for input in inputs {
        let Some(midi) = input.midi.as_mut() else {
            continue;
        };
        for clip in &mut midi.clips {
            clip.pattern = transform_pattern(&clip.pattern, source)?;
        }
        if let Some(hilo) = midi.hilo.as_mut() {
            for clip in &mut hilo.clips {
                clip.pattern = transform_pattern(&clip.pattern, source)?;
            }
        }
    }
    Ok(())
}
