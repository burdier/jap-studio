//! Descubrimiento, lectura de créditos y caché de bancos SoundFont (.sf2).
//! Sin dispositivos ni widgets: `audio/piano` lo usa para abrir el sintetizador.
use rustysynth::SoundFont;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};

const DEFAULT_SOUNDFONT: &str = "UprightPianoKW-20220221.sf2";
// Una entrada débil: los bancos visitados no quedan retenidos en memoria.
static CACHE: OnceLock<Mutex<Option<(PathBuf, Weak<SoundFont>)>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SoundFontBank {
    pub path: PathBuf,
    pub name: String,
    pub credits: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresetEntry {
    pub bank: u8,
    pub patch: u8,
    pub name: String,
}

pub(crate) fn load_path(path: &Path) -> Result<Arc<SoundFont>, String> {
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock()
        && let Some((cached_path, cached_font)) = guard.as_ref()
        && cached_path == path
        && let Some(sound_font) = cached_font.upgrade()
    {
        return Ok(sound_font);
    }
    let mut file = File::open(path)
        .map_err(|error| format!("No se pudo abrir el SoundFont {}: {error}", path.display()))?;
    let sound_font = Arc::new(
        SoundFont::new(&mut file)
            .map_err(|error| format!("SoundFont inválido ({}): {error}", path.display()))?,
    );
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((path.to_path_buf(), Arc::downgrade(&sound_font)));
    }
    Ok(sound_font)
}

pub(crate) fn presets_of(sound_font: &SoundFont) -> Vec<PresetEntry> {
    let mut presets: Vec<PresetEntry> = sound_font
        .get_presets()
        .iter()
        .filter_map(|preset| {
            let bank = u8::try_from(preset.get_bank_number()).ok()?;
            let patch = u8::try_from(preset.get_patch_number()).ok()?;
            Some(PresetEntry {
                bank,
                patch,
                name: preset.get_name().trim().to_string(),
            })
        })
        .collect();
    presets.sort_by_key(|preset| (preset.bank, preset.patch));
    presets
}

/// Descubre todos los bancos `.sf2` en `assets/soundfonts` y `assets/soundfonts/freepats`,
/// colocando el piano vertical por defecto en primer lugar.
pub(crate) fn discover() -> Vec<SoundFontBank> {
    let mut files = Vec::new();
    if let Some(custom) = std::env::var_os("HAY_SOUNDFONT") {
        let path = PathBuf::from(custom);
        if path.is_file() {
            files.push(path);
        }
    }
    for root in candidate_roots() {
        collect_sf2(&root, &mut files);
        collect_sf2(&root.join("freepats"), &mut files);
    }
    let mut seen = Vec::new();
    let mut banks = Vec::new();
    for path in files {
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.contains(&canonical) {
            continue;
        }
        seen.push(canonical);
        banks.push(read_bank_metadata(path));
    }
    banks.sort_by(|left, right| {
        let left_default = left
            .path
            .file_name()
            .is_some_and(|name| name == DEFAULT_SOUNDFONT);
        let right_default = right
            .path
            .file_name()
            .is_some_and(|name| name == DEFAULT_SOUNDFONT);
        right_default
            .cmp(&left_default)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    banks
}

fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/soundfonts"),
        PathBuf::from("assets/soundfonts"),
    ];
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        roots.push(directory.join("assets/soundfonts"));
    }
    roots
}

fn collect_sf2(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("sf2"))
        {
            out.push(path);
        }
    }
}

fn read_bank_metadata(path: PathBuf) -> SoundFontBank {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("SoundFont")
        .to_string();
    let readme_candidates = [
        path.with_extension("readme.txt"),
        path.parent()
            .map(|dir| dir.join("readme.txt"))
            .unwrap_or_default(),
    ];
    let engine_label = if super::fluidsynth::is_available() {
        "motor FluidSynth (C)"
    } else {
        "motor RustySynth"
    };
    for readme in readme_candidates {
        if let Ok(text) = fs::read_to_string(&readme)
            && let Some((title, credits)) = parse_readme(&text, engine_label)
        {
            return SoundFontBank {
                path,
                name: title,
                credits,
            };
        }
    }
    SoundFontBank {
        path,
        name: stem,
        credits: format!("Banco SoundFont (FreePats) · {engine_label}"),
    }
}

fn parse_readme(text: &str, engine_label: &str) -> Option<(String, String)> {
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" "));
                current.clear();
            }
            continue;
        }
        if line.chars().all(|ch| ch == '-' || ch == '=') {
            continue;
        }
        current.push(line.to_string());
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }
    let title = paragraphs.first()?.clone();
    let body = paragraphs
        .iter()
        .skip(1)
        .find(|para| !para.starts_with("Version "))
        .cloned()
        .unwrap_or_else(|| "FreePats project".into());
    let license = if text.contains("CC0") {
        "CC0"
    } else if text.contains("General Public License") {
        "GPLv3+ con excepción FreePats"
    } else if text.contains("Attribution") {
        "CC-BY"
    } else {
        "FreePats"
    };
    let summary = truncate_sentence(&body, 140);
    Some((
        title,
        format!("{summary} · FreePats ({license}) · {engine_label}"),
    ))
}

fn truncate_sentence(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    if let Some((head, _)) = cut.rsplit_once(". ") {
        format!("{head}.")
    } else {
        format!("{}…", cut.trim_end())
    }
}
