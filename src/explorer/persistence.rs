//! Mapeos de bibliotecas. No elimina ni modifica los archivos de las bibliotecas.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn load(file: &Path) -> Result<Vec<PathBuf>, String> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut roots = Vec::new();
    for line in text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        if line.len() % 2 != 0 || !line.is_ascii() {
            return Err("Archivo de bibliotecas inválido".into());
        }
        let bytes = (0..line.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&line[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let path = PathBuf::from(String::from_utf8(bytes).map_err(|e| e.to_string())?);
        if !path.is_absolute() {
            return Err("Una biblioteca guardada no tiene ruta absoluta".into());
        }
        if !roots.contains(&path) {
            roots.push(path);
        }
    }
    Ok(roots)
}

pub fn save(file: &Path, roots: &[PathBuf]) -> Result<(), String> {
    let mut text =
        String::from("# Hay libraries v1: UTF-8 hexadecimal, una ruta absoluta por línea\n");
    for root in roots {
        let path = root
            .to_str()
            .ok_or("La ruta de biblioteca debe ser UTF-8")?;
        for byte in path.as_bytes() {
            text.push_str(&format!("{byte:02x}"));
        }
        text.push('\n');
    }
    let parent = file.parent().ok_or("Ruta de configuración inválida")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = file.with_extension(format!("{}.tmp", std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut output = fs::File::create(&temporary)?;
        output.write_all(text.as_bytes())?;
        output.sync_all()?;
        fs::rename(&temporary, file)
    };
    let result = write().map_err(|e| e.to_string());
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn resolve_library(path: PathBuf) -> Result<PathBuf, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.is_dir() {
        return Err("Selecciona una carpeta".into());
    }
    if path.to_str().is_none() {
        return Err("La ruta de biblioteca debe ser UTF-8".into());
    }
    Ok(path)
}
