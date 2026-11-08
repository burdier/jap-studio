//! Tokens de ~/.jaybu.profile. Valores musicales y preferencias no pertenecen aquí.
use super::{cursor_fx::CursorFxMode, views::tracks::TrackHeaderConfig};
use std::{collections::HashMap, fs};

#[derive(Clone, Copy)]
pub(super) struct DesignSystem {
    pub bg: u32,
    pub panel: u32,
    pub panel_hover: u32,
    pub panel_dark: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub accent_dim: u32,
    pub success: u32,
    pub warning: u32,
    pub danger: u32,
    pub cyan: u32,
    pub orange: u32,
    pub pink: u32,
    pub grid: u32,
    pub track_header_width: f32,
    pub track_header_height: f32,
    pub timeline_bar_height: f32,
    pub timeline_pixels_per_beat: f32,
    pub title_bar_height: f32,
    pub transport_height: f32,
    pub status_bar_height: f32,
    pub pan_bar_width: f32,
    pub vol_bar_width: f32,
    pub control_bar_height: f32,
    pub knob_size: f32,
    pub transport_btn_size: f32,
    pub font_size_xs: f32,
    pub padding_xs: f32,
    pub padding_sm: f32,
    pub padding_lg: f32,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub playhead_particle_rate: f32,
    pub playhead_particle_life: f32,
    pub playhead_particle_size: f32,
    pub playhead_particle_speed: f32,
    pub playhead_particle_gravity: f32,
    pub playhead_particle_color: u32,
    pub cursor_fx: CursorFxMode,
    pub track_header: TrackHeaderConfig,
}

impl DesignSystem {
    pub(super) fn load() -> Self {
        let path = dirs::home_dir()
            .map(|d| d.join(".jaybu.profile"))
            .unwrap_or_default();
        Self::from_profile(&fs::read_to_string(&path).unwrap_or_default())
    }
    fn from_profile(content: &str) -> Self {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim(), value.trim());
            }
        }
        let get = |key: &str, default: f32| -> f32 {
            map.get(key)
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .unwrap_or(default)
        };
        let color = |key: &str, default: u32| -> u32 {
            map.get(key)
                .and_then(|value| u32::from_str_radix(value, 16).ok())
                .filter(|value| *value <= 0xffffff)
                .unwrap_or(default)
        };
        let success = color("SUCCESS", 0x52c77a);
        let cursor_fx = map
            .get("CURSOR_FX")
            .copied()
            .map(CursorFxMode::from_token)
            .unwrap_or_default();
        let track_header = TrackHeaderConfig::from_profile(
            map.get("TRACK_HEADER_LAYOUT").copied(),
            map.get("TRACK_HEADER_HIDE").copied(),
            map.get("TRACK_CONTROL_STYLE").copied(),
        );
        Self {
            bg: color("BG", 0x141618),
            panel: color("PANEL", 0x1c1e20),
            panel_hover: color("PANEL_HOVER", 0x222527),
            panel_dark: color("PANEL_DARK", 0x111314),
            border: color("BORDER", 0x2a2d30),
            text: color("TEXT", 0xd4d8db),
            muted: color("MUTED", 0x6b7280),
            accent: color("ACCENT", 0x5b9cf6),
            accent_dim: color("ACCENT_DIM", 0x1e3a5f),
            success,
            warning: color("WARNING", 0xe8b656),
            danger: color("DANGER", 0xe05252),
            cyan: color("CYAN", 0x64d9ca),
            orange: color("ORANGE", 0xe9a77a),
            pink: color("PINK", 0xd98bc7),
            grid: color("GRID", 0x3a3d40),
            track_header_width: get("TRACK_HEADER_WIDTH", 200.0),
            track_header_height: get("TRACK_HEADER_HEIGHT", 76.0).max(1.0),
            timeline_bar_height: get("TIMELINE_BAR_HEIGHT", 28.0).max(1.0),
            timeline_pixels_per_beat: get("TIMELINE_PIXELS_PER_BEAT", 32.0),
            title_bar_height: get("TITLE_BAR_HEIGHT", 36.0),
            transport_height: get("TRANSPORT_HEIGHT", 44.0),
            status_bar_height: get("STATUS_BAR_HEIGHT", 28.0),
            pan_bar_width: get("PAN_BAR_WIDTH", 40.0),
            vol_bar_width: get("VOL_BAR_WIDTH", 40.0),
            control_bar_height: get("CONTROL_BAR_HEIGHT", 6.0),
            knob_size: get("KNOB_SIZE", 22.0).clamp(14.0, 36.0),
            transport_btn_size: get("TRANSPORT_BTN_SIZE", 28.0),
            font_size_xs: get("FONT_SIZE_XS", 11.0),
            padding_xs: get("PADDING_XS", 4.0),
            padding_sm: get("PADDING_SM", 8.0),
            padding_lg: get("PADDING_LG", 16.0),
            radius_sm: get("RADIUS_SM", 3.0),
            radius_md: get("RADIUS_MD", 4.0),
            radius_lg: get("RADIUS_LG", 6.0),
            playhead_particle_rate: get("PLAYHEAD_PARTICLE_RATE", 120.0).min(2000.0),
            playhead_particle_life: get("PLAYHEAD_PARTICLE_LIFE", 0.6).clamp(0.05, 3.0),
            playhead_particle_size: get("PLAYHEAD_PARTICLE_SIZE", 2.0).clamp(0.5, 12.0),
            playhead_particle_speed: get("PLAYHEAD_PARTICLE_SPEED", 70.0).min(1000.0),
            playhead_particle_gravity: get("PLAYHEAD_PARTICLE_GRAVITY", 60.0).min(1000.0),
            playhead_particle_color: color("PLAYHEAD_PARTICLE_COLOR", success),
            cursor_fx,
            track_header,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DesignSystem;
    use crate::ui::{
        cursor_fx::CursorFxMode,
        views::tracks::{TcpLayoutPreset, TcpWidget},
    };
    #[test]
    fn profile_dimensions_are_decimal_and_colors_are_hexadecimal() {
        let design = DesignSystem::from_profile(
            "TRACK_HEADER_WIDTH=200\nTRACK_HEADER_HEIGHT=72\nTIMELINE_PIXELS_PER_BEAT=48\nBG=141618\nACCENT=5b9cf6\nRADIUS_SM=3.5\nFONT_SIZE_XS=NaN\nCURSOR_FX=neon\nTRACK_HEADER_LAYOUT=compact\nTRACK_HEADER_HIDE=rec",
        );
        assert_eq!(design.track_header_width, 200.0);
        assert_eq!(design.track_header_height, 72.0);
        assert_eq!(design.timeline_pixels_per_beat, 48.0);
        assert_eq!(design.bg, 0x141618);
        assert_eq!(design.accent, 0x5b9cf6);
        assert_eq!(design.radius_sm, 3.5);
        assert_eq!(design.font_size_xs, 11.0);
        assert_eq!(design.cursor_fx, CursorFxMode::NeonComet);
        assert_eq!(design.track_header.preset(), Some(TcpLayoutPreset::Compact));
        assert!(!design.track_header.is_visible(TcpWidget::Rec));
    }
}
