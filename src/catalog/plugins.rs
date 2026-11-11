//! Descubrimiento por archivos; no carga código del plugin ni abre Wine.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct InstalledInstrument {
    pub name: String,
    pub bundle: PathBuf,
    pub bridged: bool,
    pub class_id: String,
    pub instrument: bool,
}

impl InstalledInstrument {
    pub fn matches(&self, query: &str) -> bool {
        let description = format!(
            "{} vst3 {} {}",
            self.name,
            if self.instrument {
                "instrumento"
            } else {
                "efecto"
            },
            if self.bridged {
                "yabridge wine"
            } else {
                "nativo"
            }
        );
        super::normalize(&description).contains(&super::normalize(query))
    }
}

/// Enumera bundles candidatos sin ejecutar código. El adaptador de audio consulta
/// la fábrica en un proceso separado antes de ofrecer sus clases en el catálogo.
pub fn discover_instruments() -> Vec<InstalledInstrument> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".vst3"));
    }
    roots.extend([
        PathBuf::from("/usr/lib/vst3"),
        PathBuf::from("/usr/lib64/vst3"),
        PathBuf::from("/usr/local/lib/vst3"),
    ]);
    if let Some(paths) = std::env::var_os("VST3_PATH") {
        roots.extend(std::env::split_paths(&paths));
    }
    let mut seen = HashSet::new();
    let mut instruments = Vec::new();
    let mut budget = 8192;
    for root in roots {
        visit(&root, 0, &mut budget, &mut seen, &mut instruments);
    }
    instruments.sort_by(|a, b| a.name.cmp(&b.name).then(a.bundle.cmp(&b.bundle)));
    instruments
}

fn visit(
    path: &Path,
    depth: usize,
    budget: &mut usize,
    seen: &mut HashSet<PathBuf>,
    instruments: &mut Vec<InstalledInstrument>,
) {
    if depth > 12 || *budget == 0 {
        return;
    }
    *budget -= 1;
    let Ok(canonical) = path.canonicalize() else {
        return;
    };
    if !seen.insert(canonical.clone()) {
        return;
    }
    if path
        .extension()
        .is_some_and(|extension| extension == "vst3")
    {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        let module = path
            .join("Contents")
            .join(format!("{}-linux", std::env::consts::ARCH))
            .join(format!("{name}.so"));
        if module.is_file() {
            let bridged = path
                .components()
                .chain(canonical.components())
                .any(|component| component.as_os_str() == "yabridge");
            instruments.push(InstalledInstrument {
                name: name.into_owned(),
                bundle: canonical,
                bridged,
                class_id: String::new(),
                instrument: false,
            });
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        visit(&entry.path(), depth + 1, budget, seen, instruments);
    }
}
