//! Nombre de archivo para Guardar y Guardar como.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn project_file_path(selected: &Path) -> Result<PathBuf, String> {
    if !selected.is_absolute() {
        return Err("Elige una ubicación absoluta para el proyecto".into());
    }
    let name = selected
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty() && !name.starts_with('.'))
        .ok_or("Escribe un nombre visible para el proyecto")?;
    if selected
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("loro"))
    {
        return Ok(selected.to_owned());
    }

    let file = selected.with_file_name(format!("{name}.loro"));
    // El selector pudo confirmar el nombre original, pero no el nombre con la extensión añadida.
    match fs::symlink_metadata(&file) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(file),
        Ok(_) => Err(format!(
            "Ya existe {}. Selecciona ese archivo explícitamente o elige otro nombre",
            file.display()
        )),
        Err(error) => Err(format!("{}: {error}", file.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_selected_directory_and_checks_conflicts_when_adding_loro() {
        let root =
            std::env::temp_dir().join(format!("hay-project-location-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let result = (|| {
            let selected = root.join("Mi canción.loro");
            assert_eq!(project_file_path(&selected)?, selected);
            assert_eq!(project_file_path(&root.join("Mi canción"))?, selected);
            assert_eq!(
                project_file_path(&root.join("Mi canción.v2"))?,
                root.join("Mi canción.v2.loro")
            );
            fs::write(&selected, b"session").unwrap();
            assert!(project_file_path(&root.join("Mi canción")).is_err());
            assert_eq!(project_file_path(&selected)?, selected);
            assert_eq!(fs::read(&selected).unwrap(), b"session");
            Ok::<_, String>(())
        })();
        fs::remove_dir_all(&root).unwrap();
        result.unwrap();
    }
}
