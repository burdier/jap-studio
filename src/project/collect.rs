//! Copia recursos de audio fuera del motor y publica un paquete de sesión.
use super::{NEXT_TEMP, ProjectAudioClip, ProjectDoc};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

const MANIFEST: &str = "session.loro";

pub struct CollectedAudio {
    pub files: usize,
    /// Las rutas de estos clips son relativas a la carpeta publicada.
    pub clips: Vec<ProjectAudioClip>,
}

impl ProjectDoc {
    /// Crea una carpeta nueva con `Audio/` y referencias relativas en `session.loro`.
    /// `source_root` resuelve rutas relativas de un documento cargado previamente.
    /// El documento original queda intacto; nunca se sobrescribe una carpeta existente.
    pub fn collect_audio_into(
        &self,
        destination: &Path,
        source_root: &Path,
    ) -> Result<CollectedAudio, String> {
        let name = destination
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or("Elige una carpeta de proyecto nueva")?;
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if fs::symlink_metadata(destination).is_ok() {
            return Err("La carpeta de destino ya existe".into());
        }
        let mut staging = None;
        for _ in 0..8 {
            let number = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{}.collect-{}-{number}",
                name.to_string_lossy(),
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    staging = Some(path);
                    break;
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        let staging = staging.ok_or("No se pudo reservar la carpeta temporal")?;
        let result = self.collect_staged(&staging, destination, source_root);
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    fn collect_staged(
        &self,
        staging: &Path,
        destination: &Path,
        source_root: &Path,
    ) -> Result<CollectedAudio, String> {
        let audio_dir = staging.join("Audio");
        fs::create_dir(&audio_dir).map_err(|error| error.to_string())?;
        let candidate = Self::import(&self.snapshot()?)?;
        let mut clips = candidate.audio_clips()?;
        let mut copied: HashMap<PathBuf, String> = HashMap::new();
        for clip in &mut clips {
            let path = Path::new(&clip.source);
            let source = fs::canonicalize(if path.is_absolute() {
                path.to_path_buf()
            } else {
                source_root.join(path)
            })
            .map_err(|error| format!("Audio «{}»: {error}", clip.name))?;
            if !source.is_file() {
                return Err(format!("Audio «{}»: la fuente no es un archivo", clip.name));
            }
            let relative = if let Some(existing) = copied.get(&source) {
                existing.clone()
            } else {
                let stem = format!("{:016x}", clip.id.value());
                let filename = source
                    .extension()
                    .and_then(|value| value.to_str())
                    .filter(|extension| {
                        !extension.is_empty()
                            && extension.len() <= 16
                            && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    })
                    .map_or(stem.clone(), |extension| format!("{stem}.{extension}"));
                let target = audio_dir.join(&filename);
                fs::copy(&source, &target)
                    .map_err(|error| format!("Audio «{}»: {error}", clip.name))?;
                File::open(&target)
                    .and_then(|file| file.sync_all())
                    .map_err(|error| error.to_string())?;
                let relative = format!("Audio/{filename}");
                copied.insert(source, relative.clone());
                relative
            };
            clip.source = relative;
        }
        candidate.reconcile_audio_clips(&clips)?;
        candidate.save(&staging.join(MANIFEST))?;
        fs::create_dir(destination).map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                "La carpeta de destino apareció durante la copia".to_string()
            } else {
                error.to_string()
            }
        })?;
        if let Err(error) = fs::rename(staging, destination) {
            let _ = fs::remove_dir(destination);
            return Err(error.to_string());
        }
        Ok(CollectedAudio {
            files: copied.len(),
            clips,
        })
    }
}
