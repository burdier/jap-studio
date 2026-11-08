//! Descubrimiento y precarga del banco; sin dispositivo ni widgets.
use rustysynth::SoundFont;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

const SOUNDFONT_FILE: &str = "UprightPianoKW-20220221.sf2";
static SOUNDFONT: OnceLock<Result<Arc<SoundFont>, String>> = OnceLock::new();

pub(crate) fn preload() -> Result<(), String> {
    load().map(|_| ())
}

pub(super) fn load() -> Result<Arc<SoundFont>, String> {
    SOUNDFONT
        .get_or_init(|| {
            let path = find_soundfont()?;
            let mut file = File::open(&path).map_err(|error| {
                format!("No se pudo abrir el SoundFont {}: {error}", path.display())
            })?;
            let sound_font = SoundFont::new(&mut file)
                .map_err(|error| format!("SoundFont inválido ({}): {error}", path.display()))?;
            Ok(Arc::new(sound_font))
        })
        .clone()
}

fn find_soundfont() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("HAY_SOUNDFONT") {
        return Ok(PathBuf::from(path));
    }
    let mut candidates = vec![
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/soundfonts")
            .join(SOUNDFONT_FILE),
        Path::new("assets/soundfonts").join(SOUNDFONT_FILE),
    ];
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join("assets/soundfonts").join(SOUNDFONT_FILE));
    }
    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| {
            let paths = candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("No se encontró {SOUNDFONT_FILE}; define HAY_SOUNDFONT o colócalo en: {paths}")
        })
}
