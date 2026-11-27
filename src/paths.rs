//! Configuración centralizada y multiplataforma de rutas (VST3, VST2, CLAP, SoundFonts,
//! Presets de Synth, Samples y Drum Kits).
//! Headless: 0 dependencias de GPUI/CPAL; persiste en `<config>/hay-daw/paths.profile`.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathGroup {
    Vst3,
    Vst2,
    Clap,
    SoundFonts,
    SynthPresets,
    Samples,
    DrumKits,
}

impl PathGroup {
    pub const ALL: [Self; 7] = [
        Self::Vst3,
        Self::Vst2,
        Self::Clap,
        Self::SoundFonts,
        Self::SynthPresets,
        Self::Samples,
        Self::DrumKits,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Vst3 => "vst3",
            Self::Vst2 => "vst2",
            Self::Clap => "clap",
            Self::SoundFonts => "soundfonts",
            Self::SynthPresets => "presets",
            Self::Samples => "samples",
            Self::DrumKits => "kits",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim().to_ascii_lowercase().as_str() {
            "vst3" => Some(Self::Vst3),
            "vst2" => Some(Self::Vst2),
            "clap" => Some(Self::Clap),
            "soundfonts" | "sf2" => Some(Self::SoundFonts),
            "presets" | "synth_presets" => Some(Self::SynthPresets),
            "samples" | "library" => Some(Self::Samples),
            "kits" | "drum_kits" => Some(Self::DrumKits),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Vst3 => "Plugins VST3",
            Self::Vst2 => "Plugins VST2",
            Self::Clap => "Plugins CLAP",
            Self::SoundFonts => "SoundFonts (.sf2)",
            Self::SynthPresets => "Presets Synth (.vital / .fxp)",
            Self::Samples => "Samples y Librerías",
            Self::DrumKits => "Drum Kits (.haykit)",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Vst3 => "Carpetas donde buscar instrumentos y efectos .vst3",
            Self::Vst2 => "Carpetas reservadas para plugins VST2 (.so / .dll / .vst)",
            Self::Clap => "Carpetas reservadas para plugins CLAP (.clap)",
            Self::SoundFonts => "Bancos .sf2 para el piano/sintetizador SoundFont",
            Self::SynthPresets => "Presets y bancos externos para Hay Synth",
            Self::Samples => "Carpetas de audio para Sampler, Drums y Explorer",
            Self::DrumKits => "Kits de batería guardados (.haykit)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub vst3: Vec<PathBuf>,
    pub vst2: Vec<PathBuf>,
    pub clap: Vec<PathBuf>,
    pub soundfonts: Vec<PathBuf>,
    pub synth_presets: Vec<PathBuf>,
    pub samples: Vec<PathBuf>,
    pub drum_kits: Vec<PathBuf>,
}

impl Default for AppPaths {
    fn default() -> Self {
        Self::defaults_for_os()
    }
}

impl AppPaths {
    pub fn defaults_for_os() -> Self {
        Self {
            vst3: default_group_paths(PathGroup::Vst3),
            vst2: default_group_paths(PathGroup::Vst2),
            clap: default_group_paths(PathGroup::Clap),
            soundfonts: default_group_paths(PathGroup::SoundFonts),
            synth_presets: default_group_paths(PathGroup::SynthPresets),
            samples: default_group_paths(PathGroup::Samples),
            drum_kits: default_group_paths(PathGroup::DrumKits),
        }
    }

    pub fn config_file() -> Option<PathBuf> {
        config_base_dir().map(|d| d.join("hay-daw").join("paths.profile"))
    }

    pub fn load() -> Self {
        let Some(file) = Self::config_file() else {
            return Self::defaults_for_os();
        };
        let Ok(content) = fs::read_to_string(file) else {
            return Self::defaults_for_os();
        };
        Self::parse(&content)
    }

    pub fn save(&self) -> Result<(), String> {
        let file = Self::config_file()
            .ok_or_else(|| "No se encontró carpeta de configuración del usuario".to_string())?;
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = file.with_extension("profile.tmp");
        fs::write(&tmp, self.serialize()).map_err(|e| e.to_string())?;
        if let Err(err) = fs::rename(&tmp, &file) {
            // En Windows `rename` puede fallar si el destino ya existe.
            let _ = fs::remove_file(&file);
            fs::rename(&tmp, &file).map_err(|e| format!("{err}; {e}"))?;
        }
        Ok(())
    }

    pub fn serialize(&self) -> String {
        let mut out = String::from(
            "# Jap Studio · Rutas configurables (Linux / Windows / macOS)\n\
             # Una entrada por línea: grupo = /ruta/absoluta\n",
        );
        for group in PathGroup::ALL {
            out.push('\n');
            out.push_str(&format!("# {}\n", group.label()));
            let list = self.paths(group);
            if list.is_empty() {
                out.push_str(&format!("{} =\n", group.key()));
            } else {
                for p in list {
                    out.push_str(&format!("{} = {}\n", group.key(), p.display()));
                }
            }
        }
        out
    }

    pub fn parse(content: &str) -> Self {
        let mut cfg = Self::defaults_for_os();
        let mut seen_groups: HashSet<PathGroup> = HashSet::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((k, v)) = trimmed.split_once('=') else {
                continue;
            };
            let Some(group) = PathGroup::from_key(k) else {
                continue;
            };
            if seen_groups.insert(group) {
                cfg.paths_mut(group).clear();
            }
            let val = v.trim();
            if !val.is_empty() {
                cfg.add_path(group, PathBuf::from(val));
            }
        }
        cfg
    }

    pub fn paths(&self, group: PathGroup) -> &[PathBuf] {
        match group {
            PathGroup::Vst3 => &self.vst3,
            PathGroup::Vst2 => &self.vst2,
            PathGroup::Clap => &self.clap,
            PathGroup::SoundFonts => &self.soundfonts,
            PathGroup::SynthPresets => &self.synth_presets,
            PathGroup::Samples => &self.samples,
            PathGroup::DrumKits => &self.drum_kits,
        }
    }

    pub fn paths_mut(&mut self, group: PathGroup) -> &mut Vec<PathBuf> {
        match group {
            PathGroup::Vst3 => &mut self.vst3,
            PathGroup::Vst2 => &mut self.vst2,
            PathGroup::Clap => &mut self.clap,
            PathGroup::SoundFonts => &mut self.soundfonts,
            PathGroup::SynthPresets => &mut self.synth_presets,
            PathGroup::Samples => &mut self.samples,
            PathGroup::DrumKits => &mut self.drum_kits,
        }
    }

    pub fn add_path(&mut self, group: PathGroup, path: PathBuf) -> bool {
        if path.as_os_str().is_empty() {
            return false;
        }
        let list = self.paths_mut(group);
        if list.iter().any(|p| p == &path) {
            return false;
        }
        list.push(path);
        true
    }

    pub fn remove_path(&mut self, group: PathGroup, index: usize) -> bool {
        let list = self.paths_mut(group);
        if index < list.len() {
            list.remove(index);
            true
        } else {
            false
        }
    }

    pub fn reset_group(&mut self, group: PathGroup) {
        *self.paths_mut(group) = default_group_paths(group);
    }

    pub fn reset_all(&mut self) {
        *self = Self::defaults_for_os();
    }

    /// Devuelve las rutas configuradas para `group` combinadas con variables de entorno estándar
    /// (`VST3_PATH`, `VST_PATH`, `CLAP_PATH`) sin duplicados.
    pub fn effective_paths(group: PathGroup) -> Vec<PathBuf> {
        let cfg = Self::load();
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for p in cfg.paths(group) {
            if seen.insert(p.clone()) {
                out.push(p.clone());
            }
        }
        let env_var = match group {
            PathGroup::Vst3 => Some("VST3_PATH"),
            PathGroup::Vst2 => Some("VST_PATH"),
            PathGroup::Clap => Some("CLAP_PATH"),
            _ => None,
        };
        if let Some(var) = env_var
            && let Some(raw) = std::env::var_os(var)
        {
            for p in std::env::split_paths(&raw) {
                if !p.as_os_str().is_empty() && seen.insert(p.clone()) {
                    out.push(p);
                }
            }
        }
        out
    }
}

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

pub fn config_base_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join("AppData").join("Roaming")))
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".config")))
    }
}

pub fn data_local_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join("AppData").join("Local")))
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".local").join("share")))
    }
}

pub fn default_group_paths(group: PathGroup) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut push = |p: PathBuf| {
        if !p.as_os_str().is_empty() && !out.contains(&p) {
            out.push(p);
        }
    };
    let home = home_dir();
    let data = data_local_dir();
    let config = config_base_dir();

    match group {
        PathGroup::Vst3 => {
            #[cfg(target_os = "linux")]
            {
                if let Some(h) = &home {
                    push(h.join(".vst3"));
                }
                push(PathBuf::from("/usr/lib/vst3"));
                push(PathBuf::from("/usr/lib64/vst3"));
                push(PathBuf::from("/usr/local/lib/vst3"));
            }
            #[cfg(target_os = "windows")]
            {
                if let Some(common) = std::env::var_os("CommonProgramFiles") {
                    push(PathBuf::from(common).join("VST3"));
                } else {
                    push(PathBuf::from(r"C:\Program Files\Common Files\VST3"));
                }
                if let Some(d) = &data {
                    push(d.join("Programs").join("Common").join("VST3"));
                }
            }
            #[cfg(target_os = "macos")]
            {
                if let Some(h) = &home {
                    push(h.join("Library/Audio/Plug-Ins/VST3"));
                }
                push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
            }
        }
        PathGroup::Vst2 => {
            #[cfg(target_os = "linux")]
            {
                if let Some(h) = &home {
                    push(h.join(".vst"));
                }
                push(PathBuf::from("/usr/lib/vst"));
                push(PathBuf::from("/usr/local/lib/vst"));
            }
            #[cfg(target_os = "windows")]
            {
                push(PathBuf::from(r"C:\Program Files\VSTPlugins"));
                push(PathBuf::from(r"C:\Program Files\Steinberg\VSTPlugins"));
                push(PathBuf::from(r"C:\Program Files\Common Files\VST2"));
            }
            #[cfg(target_os = "macos")]
            {
                if let Some(h) = &home {
                    push(h.join("Library/Audio/Plug-Ins/VST"));
                }
                push(PathBuf::from("/Library/Audio/Plug-Ins/VST"));
            }
        }
        PathGroup::Clap => {
            #[cfg(target_os = "linux")]
            {
                if let Some(h) = &home {
                    push(h.join(".clap"));
                }
                push(PathBuf::from("/usr/lib/clap"));
                push(PathBuf::from("/usr/local/lib/clap"));
            }
            #[cfg(target_os = "windows")]
            {
                if let Some(common) = std::env::var_os("CommonProgramFiles") {
                    push(PathBuf::from(common).join("CLAP"));
                } else {
                    push(PathBuf::from(r"C:\Program Files\Common Files\CLAP"));
                }
                if let Some(d) = &data {
                    push(d.join("Programs").join("Common").join("CLAP"));
                }
            }
            #[cfg(target_os = "macos")]
            {
                if let Some(h) = &home {
                    push(h.join("Library/Audio/Plug-Ins/CLAP"));
                }
                push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
            }
        }
        PathGroup::SoundFonts => {
            if let Ok(exe) = std::env::current_exe()
                && let Some(dir) = exe.parent()
            {
                push(dir.join("assets").join("soundfonts"));
            }
            push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("soundfonts"));
            if let Some(d) = &data {
                push(d.join("jap-studio").join("soundfonts"));
            }
            if let Some(h) = &home {
                push(h.join("Soundfonts"));
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                push(h.join("Documents").join("Soundfonts"));
            }
        }
        PathGroup::SynthPresets => {
            if let Some(d) = &data {
                push(d.join("jap-studio").join("presets").join("vital"));
                #[cfg(target_os = "linux")]
                push(d.join("vital"));
            }
            if let Some(h) = &home {
                #[cfg(target_os = "linux")]
                {
                    push(h.join("library"));
                    push(h.join("Music"));
                }
                #[cfg(target_os = "windows")]
                {
                    push(h.join("Documents").join("Vital"));
                    push(h.join("Documents").join("Xfer").join("Serum Presets").join("Presets"));
                    push(h.join("Music"));
                }
                #[cfg(target_os = "macos")]
                {
                    push(h.join("Music").join("Vital"));
                    push(PathBuf::from("/Library/Audio/Presets/Xfer Records/Serum Presets/Presets"));
                    push(h.join("Music"));
                }
            }
            #[cfg(target_os = "linux")]
            if let Ok(media) = fs::read_dir("/run/media") {
                for user_dir in media.flatten() {
                    if let Ok(mounts) = fs::read_dir(user_dir.path()) {
                        for mount in mounts.flatten() {
                            let lib = mount.path().join("library");
                            if lib.is_dir() {
                                push(lib);
                            }
                        }
                    }
                }
            }
        }
        PathGroup::Samples => {
            if let Some(d) = &data {
                push(d.join("jap-studio").join("samples"));
            }
            if let Some(h) = &home {
                push(h.join("Music"));
                push(h.join("Samples"));
                #[cfg(target_os = "linux")]
                push(h.join("library"));
            }
        }
        PathGroup::DrumKits => {
            if let Some(c) = &config {
                push(c.join("hay-daw").join("kits"));
            }
            if let Some(d) = &data {
                push(d.join("jap-studio").join("kits"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_custom_paths_preserving_all_groups() {
        let mut cfg = AppPaths::defaults_for_os();
        cfg.vst3 = vec![PathBuf::from("/opt/vst3"), PathBuf::from("/home/u/.vst3")];
        cfg.vst2 = vec![PathBuf::from("/opt/vst2")];
        cfg.clap = vec![PathBuf::from("/opt/clap")];
        cfg.soundfonts = vec![PathBuf::from("/data/sf2")];
        cfg.synth_presets = vec![PathBuf::from("/data/presets")];
        cfg.samples = vec![PathBuf::from("/data/samples")];
        cfg.drum_kits = vec![PathBuf::from("/data/kits")];

        let text = cfg.serialize();
        let parsed = AppPaths::parse(&text);
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn adding_duplicate_or_empty_path_is_ignored() {
        let mut cfg = AppPaths::defaults_for_os();
        cfg.vst3.clear();
        assert!(!cfg.add_path(PathGroup::Vst3, PathBuf::new()));
        assert!(cfg.add_path(PathGroup::Vst3, PathBuf::from("/custom/vst3")));
        assert!(!cfg.add_path(PathGroup::Vst3, PathBuf::from("/custom/vst3")));
        assert_eq!(cfg.vst3.len(), 1);
    }
}
