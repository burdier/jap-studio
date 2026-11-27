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
    let roots = crate::paths::AppPaths::effective_paths(crate::paths::PathGroup::Vst3);
    let mut seen = HashSet::new();
    let mut instruments = Vec::new();
    let mut budget = 8192;
    for root in roots {
        visit(&root, 0, &mut budget, &mut seen, &mut instruments);
    }
    instruments.sort_by(|a, b| a.name.cmp(&b.name).then(a.bundle.cmp(&b.bundle)));
    instruments
}

fn vst3_binary_exists(bundle: &Path, name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        bundle
            .join("Contents")
            .join(format!("{}-linux", std::env::consts::ARCH))
            .join(format!("{name}.so"))
            .is_file()
    }
    #[cfg(target_os = "windows")]
    {
        if bundle.is_file() {
            return true;
        }
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64-win",
            "aarch64" => "arm64-win",
            other => other,
        };
        bundle
            .join("Contents")
            .join(arch)
            .join(format!("{name}.vst3"))
            .is_file()
    }
    #[cfg(target_os = "macos")]
    {
        bundle.join("Contents").join("MacOS").join(name).is_file()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (bundle, name);
        false
    }
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
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vst3"))
    {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        if vst3_binary_exists(path, &name) {
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
