//! Ubicación de un proyecto nuevo y limpieza si falla el primer guardado.
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn save_in_new_folder<T>(
    selected: &Path,
    save: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<(PathBuf, T), String> {
    if !selected.is_absolute() {
        return Err("Elige una ubicación absoluta para el proyecto".into());
    }
    let name = match selected.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("loro") => selected.file_stem(),
        _ => selected.file_name(),
    }
    .and_then(|name| name.to_str())
    .filter(|name| !name.trim().is_empty() && !name.starts_with('.'))
    .ok_or("Escribe un nombre visible para el proyecto")?;
    let folder = selected.with_file_name(name);
    fs::create_dir(&folder)
        .map_err(|error| format!("No se pudo crear la carpeta {}: {error}", folder.display()))?;
    let file = folder.join(format!("{name}.loro"));
    match save(&file) {
        Ok(result) => Ok((file, result)),
        Err(error) => {
            let _ = fs::remove_dir(&folder);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_named_project_without_reusing_existing_folders() {
        let root =
            std::env::temp_dir().join(format!("hay-project-location-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let result = (|| {
            let selected = root.join("Mi canción.loro");
            let (file, ()) = save_in_new_folder(&selected, |file| {
                fs::write(file, b"session").map_err(|e| e.to_string())
            })?;
            assert_eq!(file, root.join("Mi canción/Mi canción.loro"));
            assert!(save_in_new_folder(&selected, |_| Ok(())).is_err());
            let failed = root.join("Fallido.loro");
            assert!(
                save_in_new_folder::<()>(&failed, |_| Err("escritura fallida".into())).is_err()
            );
            assert!(!root.join("Fallido").exists());
            assert!(!selected.exists());
            assert_eq!(fs::read(&file).unwrap(), b"session");
            Ok::<_, String>(())
        })();
        fs::remove_dir_all(&root).unwrap();
        result.unwrap();
    }
}
