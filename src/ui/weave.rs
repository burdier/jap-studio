//! Carga la regla Lua al abrir la sesión, lejos del callback de audio.
use elevate::musical::track::TrackId;
use elevate::musical::weave::WeaveRule;
use std::io::Read;

impl super::DawApp {
    pub(super) fn rule_for_hilo(&self, id: TrackId) -> Result<Option<WeaveRule>, String> {
        if let Some(rule) = self
            .tracks
            .iter()
            .find(|track| track.id == id)
            .and_then(|track| track.weave_rule.clone())
        {
            return Ok(Some(rule));
        }
        self.weave_rule
            .as_ref()
            .map(Clone::clone)
            .map_err(Clone::clone)
    }
}

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
