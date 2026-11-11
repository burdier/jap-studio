//! Perfil visual: preservar claves ajenas y reemplazar el archivo solo tras escribirlo completo.
use super::DesignSystem;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;

fn path() -> io::Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".jaybu.profile"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "No se encontró la carpeta personal",
            )
        })
}

pub(super) fn load() -> io::Result<String> {
    fs::read_to_string(path()?)
}

pub(super) fn save_appearance(design: &DesignSystem) -> io::Result<()> {
    save_appearance_at(&path()?, design)
}

fn save_appearance_at(path: &Path, design: &DesignSystem) -> io::Result<()> {
    // Guardar también los colores: los perfiles antiguos contienen una paleta explícita
    // que, de guardar solo THEME, volvería a imponerse al reiniciar o recargar.
    let mut values = vec![
        ("THEME", design.theme.token().to_string()),
        ("CURSOR_FX", design.cursor_fx.token().to_string()),
        ("SELECTION_CONTRAST", design.selection_contrast.to_string()),
    ];
    values.extend(
        [
            ("BG", design.bg),
            ("PANEL", design.panel),
            ("PANEL_HOVER", design.panel_hover),
            ("PANEL_DARK", design.panel_dark),
            ("BORDER", design.border),
            ("TEXT", design.text),
            ("MUTED", design.muted),
            ("ACCENT", design.accent),
            ("ACCENT_DIM", design.accent_dim),
            ("SUCCESS", design.success),
            ("WARNING", design.warning),
            ("DANGER", design.danger),
            ("CYAN", design.cyan),
            ("ORANGE", design.orange),
            ("PINK", design.pink),
            ("GRID", design.grid),
            ("PLAYHEAD_PARTICLE_COLOR", design.playhead_particle_color),
        ]
        .map(|(key, value)| (key, format!("{value:06x}"))),
    );
    values.extend(
        [
            ("TRACK_HEADER_WIDTH", design.track_header_width),
            ("TRACK_HEADER_HEIGHT", design.track_header_height),
            ("TIMELINE_BAR_HEIGHT", design.timeline_bar_height),
            ("TIMELINE_PIXELS_PER_BEAT", design.timeline_pixels_per_beat),
            ("TITLE_BAR_HEIGHT", design.title_bar_height),
            ("TRANSPORT_HEIGHT", design.transport_height),
            ("STATUS_BAR_HEIGHT", design.status_bar_height),
            ("PAN_BAR_WIDTH", design.pan_bar_width),
            ("VOL_BAR_WIDTH", design.vol_bar_width),
            ("CONTROL_BAR_HEIGHT", design.control_bar_height),
            ("TRANSPORT_BTN_SIZE", design.transport_btn_size),
            ("FONT_SIZE_XS", design.font_size_xs),
            ("PADDING_XS", design.padding_xs),
            ("PADDING_SM", design.padding_sm),
            ("PADDING_LG", design.padding_lg),
            ("RADIUS_SM", design.radius_sm),
            ("RADIUS_MD", design.radius_md),
            ("RADIUS_LG", design.radius_lg),
        ]
        .map(|(key, value)| (key, value.to_string())),
    );
    update_at(path, &values)
}

pub(super) fn save_values(values: &[(&str, String)]) -> io::Result<()> {
    update_at(&path()?, values)
}

fn update_at(path: &Path, values: &[(&str, String)]) -> io::Result<()> {
    // Mantener enlaces simbólicos: reemplazar su destino, no el enlace del perfil.
    let target = match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path)?,
        Ok(_) => path.to_path_buf(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error),
    };
    let content = match fs::read_to_string(&target) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let updated = replace_values(&content, values);
    if updated == content {
        return Ok(());
    }
    let permissions = fs::metadata(&target).ok().map(|meta| meta.permissions());
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("Ruta de perfil inválida"))?;
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    let (temporary, mut file) = loop {
        let sequence = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".hay-profile-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.write_all(updated.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn replace_values(content: &str, values: &[(&str, String)]) -> String {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut seen = vec![false; values.len()];
    let mut output = String::new();
    for line in content.split_inclusive('\n') {
        let key = line.split_once('=').map(|(key, _)| key.trim());
        if let Some(index) = values.iter().position(|(name, _)| Some(*name) == key) {
            let (name, value) = &values[index];
            output.push_str(&format!("{name}={value}"));
            if line.ends_with('\n') {
                output.push_str(newline);
            }
            seen[index] = true;
        } else {
            output.push_str(line);
        }
    }
    for (index, (name, value)) in values.iter().enumerate() {
        if !seen[index] {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push_str(newline);
            }
            output.push_str(&format!("{name}={value}{newline}"));
        }
    }
    output
}
