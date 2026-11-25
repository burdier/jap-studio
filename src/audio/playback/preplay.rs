//! Transforma únicamente snapshots del worker; jamás el clip editable ni RT.
use crate::audio::TrackInput;
use elevate::scripting::live_preplay::transform_pattern;

pub(crate) fn apply_preplay(inputs: &mut [TrackInput]) -> Result<(), String> {
    let Some(path) = std::env::var_os("HAY_LUA_PREPLAY_SCRIPT") else {
        return Ok(());
    };
    let path = std::path::Path::new(&path);
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("Lua pre-Play ({}): {error}", path.display()))?;
    for input in inputs {
        let Some(midi) = input.midi.as_mut() else {
            continue;
        };
        for clip in &mut midi.clips {
            clip.pattern = transform_pattern(&clip.pattern, &source)?;
        }
        if let Some(hilo) = midi.hilo.as_mut() {
            for clip in &mut hilo.clips {
                clip.pattern = transform_pattern(&clip.pattern, &source)?;
            }
        }
    }
    Ok(())
}
