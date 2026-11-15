//! Carga la regla Lua al abrir la sesión, lejos del callback de audio.
use elevate::musical::weave::WeaveRule;
use std::io::Read;

pub(super) fn load_from_env() -> Result<Option<WeaveRule>, String> {
    let Some(path) = std::env::var_os("HAY_WEAVE_RULE_SCRIPT").map(std::path::PathBuf::from) else {
        return Ok(None);
    };
    let file = std::fs::File::open(&path)
        .map_err(|error| format!("Tejer ({}): {error}", path.display()))?;
    let mut source = String::new();
    file.take(16 * 1024 + 1)
        .read_to_string(&mut source)
        .map_err(|error| format!("Tejer ({}): {error}", path.display()))?;
    elevate::scripting::weave::compile(&source).map(Some)
}
