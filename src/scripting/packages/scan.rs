//! Descubrimiento no recursivo y lectura acotada; un error descarta todo el candidato.
use super::{Package, PackageDirectories, PackageError, descriptor, fields::Location};
use crate::scripting::sandbox::MAX_SOURCE_BYTES;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_ENTRIES: usize = 1024;
const MAX_PACKAGES: usize = 128;
const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn load(
    directories: &PackageDirectories,
) -> Result<BTreeMap<String, Arc<Package>>, PackageError> {
    let mut paths = Vec::new();
    scan(&directories.bundled, true, &mut paths)?;
    if let Some(user) = &directories.user {
        scan(user, false, &mut paths)?;
    }
    paths.sort();
    let mut packages: BTreeMap<String, Arc<Package>> = BTreeMap::new();
    let mut total = 0;
    for (path, bundled) in paths {
        let source = read(&path)?;
        total += source.len();
        if total > MAX_TOTAL_BYTES {
            return Err(PackageError::load(
                &path,
                "La fuente acumulada supera 8 MiB",
            ));
        }
        let descriptor = descriptor::read(&source, &path, bundled)?;
        if let Some(previous) = packages.get(&descriptor.id) {
            return Err(Location {
                path: &path,
                id: Some(descriptor.id.clone()),
            }
            .error(
                "id",
                format!("ID duplicado; ya declarado en {}", previous.path.display()),
            ));
        }
        packages.insert(
            descriptor.id.clone(),
            Arc::new(Package {
                path,
                source,
                descriptor,
            }),
        );
    }
    Ok(packages)
}

fn scan(
    directory: &Path,
    bundled: bool,
    paths: &mut Vec<(PathBuf, bool)>,
) -> Result<(), PackageError> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(PackageError::load(directory, error)),
    };
    if !metadata.is_dir() {
        return Err(PackageError::load(
            directory,
            "Se esperaba un directorio sin enlace simbólico",
        ));
    }
    let entries = fs::read_dir(directory).map_err(|e| PackageError::load(directory, e))?;
    for (index, entry) in entries.enumerate() {
        if index == MAX_ENTRIES {
            return Err(PackageError::load(
                directory,
                "El directorio supera 1024 entradas",
            ));
        }
        let entry = entry.map_err(|e| PackageError::load(directory, e))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| PackageError::load(&path, e))?;
        if !kind.is_file() || path.extension().is_none_or(|extension| extension != "lua") {
            continue;
        }
        paths.push((path, bundled));
    }
    // Orden estable también cuando el error es el número de paquetes.
    paths.sort();
    if paths.len() > MAX_PACKAGES {
        return Err(PackageError::load(
            &paths[MAX_PACKAGES].0,
            "La carga supera 128 paquetes",
        ));
    }
    Ok(())
}

fn read(path: &Path) -> Result<String, PackageError> {
    let metadata = fs::symlink_metadata(path).map_err(|e| PackageError::load(path, e))?;
    if !metadata.is_file() {
        return Err(PackageError::load(
            path,
            "Se esperaba un archivo regular sin enlace simbólico",
        ));
    }
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err(PackageError::load(path, "El paquete supera 256 KiB"));
    }
    let file = File::open(path).map_err(|e| PackageError::load(path, e))?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| PackageError::load(path, e))?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(PackageError::load(path, "El paquete supera 256 KiB"));
    }
    String::from_utf8(bytes).map_err(|e| PackageError::load(path, e))
}
