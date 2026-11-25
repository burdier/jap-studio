use super::*;
use crate::ui::{cursor_fx::CursorFxMode, design::ThemePreset};

struct TestProfile(PathBuf);

impl TestProfile {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hay-profile-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> PathBuf {
        self.0.join("profile")
    }

    fn reload(&self) -> DesignSystem {
        DesignSystem::from_profile(&fs::read_to_string(self.path()).unwrap())
    }
}

impl Drop for TestProfile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn every_theme_survives_reload_with_legacy_color_overrides_and_unrelated_settings() {
    let fixture = TestProfile::new();
    let original = "# Mi perfil\nBG=123456\nACCENT=abcdef\nPANEL=111111\nGRID=222222\nPLAYHEAD_PARTICLE_COLOR=777777\nTRACK_HEADER_WIDTH=237\nTRACK_HEADER_HIDE=rec\nCUSTOM_KEY=conservar\n";
    fs::write(fixture.path(), original).unwrap();
    let mut design = DesignSystem::from_profile(original);
    design.cursor_fx = CursorFxMode::Starlight;
    for theme in ThemePreset::ALL {
        design.apply_theme(theme);
        save_appearance_at(&fixture.path(), &design).unwrap();
        let reloaded = fixture.reload();
        assert_eq!(reloaded.theme, theme);
        assert_eq!(reloaded.cursor_fx, CursorFxMode::Starlight);
        assert_eq!(
            (reloaded.bg, reloaded.accent, reloaded.panel, reloaded.grid),
            (design.bg, design.accent, design.panel, design.grid)
        );
        assert_eq!(
            reloaded.playhead_particle_color,
            design.playhead_particle_color
        );
        assert_eq!(reloaded.track_header_width, 237.0);
        let written = fs::read_to_string(fixture.path()).unwrap();
        assert!(written.contains("# Mi perfil\n"));
        assert!(written.contains("TRACK_HEADER_HIDE=rec\nCUSTOM_KEY=conservar\n"));
        save_appearance_at(&fixture.path(), &reloaded).unwrap();
        assert_eq!(fs::read_to_string(fixture.path()).unwrap(), written);
    }
}

#[test]
fn every_cursor_mode_is_saved_without_resetting_custom_colors_or_dimensions() {
    let fixture = TestProfile::new();
    let original =
        "THEME=cyberpunk\nBG=abcdef\nACCENT=123456\nPLAYHEAD_PARTICLE_COLOR=aa1100\nKNOB_SIZE=31\n";
    fs::write(fixture.path(), original).unwrap();
    let mut design = DesignSystem::from_profile(original);
    for cursor in CursorFxMode::ALL {
        design.cursor_fx = cursor;
        save_appearance_at(&fixture.path(), &design).unwrap();
        let reloaded = fixture.reload();
        assert_eq!(reloaded.cursor_fx, cursor);
        assert_eq!(reloaded.theme, ThemePreset::Cyberpunk);
        assert_eq!(reloaded.bg, 0xabcdef);
        assert_eq!(reloaded.accent, 0x123456);
        assert_eq!(reloaded.playhead_particle_color, 0xaa1100);
        assert_eq!(reloaded.knob_size, 31.0);
    }
}

#[test]
fn editing_theme_in_profile_switches_palette_immediately() {
    let fixture = TestProfile::new();
    let mut design = DesignSystem::from_profile("");
    design.apply_theme(ThemePreset::Cyberpunk);
    save_appearance_at(&fixture.path(), &design).unwrap();
    let content = fs::read_to_string(fixture.path()).unwrap();
    // No debe escribir líneas BG=/PANEL= que bloqueen cambios manuales de THEME=.
    assert!(!content.contains("BG="));
    let edited = content.replace("THEME=cyberpunk", "THEME=daylight studio");
    fs::write(fixture.path(), edited).unwrap();
    let reloaded = fixture.reload();
    assert_eq!(reloaded.theme, ThemePreset::DaylightStudio);
    assert_eq!(reloaded.bg, ThemePreset::DaylightStudio.palette().bg);
    assert_eq!(
        reloaded.accent,
        ThemePreset::DaylightStudio.palette().accent
    );
}

#[test]
fn first_save_creates_profile_and_duplicate_keys_cannot_override_last_choice() {
    let fixture = TestProfile::new();
    let mut design = DesignSystem::from_profile("");
    design.apply_theme(ThemePreset::DaylightStudio);
    design.cursor_fx = CursorFxMode::Off;
    save_appearance_at(&fixture.path(), &design).unwrap();
    assert_eq!(fixture.reload().theme, ThemePreset::DaylightStudio);
    fs::write(fixture.path(), "# THEME=no cambiar\r\nTHEME=cyberpunk\r\n THEME = cyberpunk\r\nCURSOR_FX=fire\r\nCURSOR_FX=stars").unwrap();
    save_appearance_at(&fixture.path(), &design).unwrap();
    let reloaded = fixture.reload();
    assert_eq!(reloaded.theme, ThemePreset::DaylightStudio);
    assert_eq!(reloaded.cursor_fx, CursorFxMode::Off);
    assert!(
        fs::read_to_string(fixture.path())
            .unwrap()
            .starts_with("# THEME=no cambiar\r\n")
    );
}

#[test]
fn failed_save_reports_error_without_truncating_existing_profile_or_leaving_temporary_files() {
    let fixture = TestProfile::new();
    let invalid_utf8 = [0xff, 0xfe, 0x61];
    fs::write(fixture.path(), invalid_utf8).unwrap();
    let design = DesignSystem::from_profile("");
    assert!(save_appearance_at(&fixture.path(), &design).is_err());
    assert_eq!(fs::read(fixture.path()).unwrap(), invalid_utf8);
    assert!(save_appearance_at(&fixture.path().join("invalid-child"), &design).is_err());
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn saving_preserves_profile_symlink_and_target_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = TestProfile::new();
    let target = fixture.0.join("real-profile");
    fs::write(&target, "# Compartido\nTHEME=cyberpunk\n").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, fixture.path()).unwrap();
    let mut design = DesignSystem::from_profile("");
    design.apply_theme(ThemePreset::DaylightStudio);
    save_appearance_at(&fixture.path(), &design).unwrap();
    assert!(
        fs::symlink_metadata(fixture.path())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fixture.reload().theme, ThemePreset::DaylightStudio);
    assert_eq!(
        fs::metadata(target).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 2);
}
