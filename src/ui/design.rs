//! Tokens de ~/.jaybu.profile y temas de color integrados.
use super::{cursor_fx::CursorFxMode, views::tracks::TrackHeaderConfig};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui) enum ThemePreset {
    #[default]
    Obsidian,
    MameyStudio,
    CatppuccinMocha,
    Cyberpunk,
    GruvboxDark,
    TokyoNight,
    RosePine,
    NordAurora,
    SilverHardware,
    GruvboxPaper,
    MameyCream,
    MatchaMist,
    RosePineDawn,
    DaylightLatte,
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct ThemePalette {
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
}

impl ThemePreset {
    pub const ALL: [Self; 14] = [
        Self::Obsidian,
        Self::MameyStudio,
        Self::CatppuccinMocha,
        Self::Cyberpunk,
        Self::GruvboxDark,
        Self::TokyoNight,
        Self::RosePine,
        Self::NordAurora,
        Self::SilverHardware,
        Self::GruvboxPaper,
        Self::MameyCream,
        Self::MatchaMist,
        Self::RosePineDawn,
        Self::DaylightLatte,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Obsidian => "Obsidiana",
            Self::MameyStudio => "Mamey Studio",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::Cyberpunk => "Cyberpunk Neon",
            Self::GruvboxDark => "Gruvbox Tape",
            Self::TokyoNight => "Tokyo Night",
            Self::RosePine => "Rosé Pine",
            Self::NordAurora => "Nord Aurora",
            Self::SilverHardware => "Aluminio OP-1",
            Self::GruvboxPaper => "Gruvbox Arena",
            Self::MameyCream => "Mamey Crema",
            Self::MatchaMist => "Matcha Bruma",
            Self::RosePineDawn => "Rosé Pine Dawn",
            Self::DaylightLatte => "Daylight Studio",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Obsidian => "obsidian",
            Self::MameyStudio => "mamey",
            Self::CatppuccinMocha => "mocha",
            Self::Cyberpunk => "cyberpunk",
            Self::GruvboxDark => "gruvbox",
            Self::TokyoNight => "tokyonight",
            Self::RosePine => "rosepine",
            Self::NordAurora => "nord",
            Self::SilverHardware => "silver",
            Self::GruvboxPaper => "sand",
            Self::MameyCream => "mamey-cream",
            Self::MatchaMist => "matcha",
            Self::RosePineDawn => "dawn",
            Self::DaylightLatte => "latte",
        }
    }

    pub fn from_token(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "mamey" | "mamei" | "carrot" => Self::MameyStudio,
            "mocha" | "catppuccin" => Self::CatppuccinMocha,
            "cyberpunk" | "neon" => Self::Cyberpunk,
            "gruvbox" | "tape" | "warm" => Self::GruvboxDark,
            "tokyonight" | "tokyo" => Self::TokyoNight,
            "rosepine" | "rose" => Self::RosePine,
            "nord" | "aurora" => Self::NordAurora,
            "silver" | "op1" | "aluminio" | "hardware" => Self::SilverHardware,
            "sand" | "arena" | "paper" | "gruvbox-light" => Self::GruvboxPaper,
            "mamey-cream" | "crema" | "peach" => Self::MameyCream,
            "matcha" | "sage" | "bruma" => Self::MatchaMist,
            "dawn" | "rosepine-dawn" => Self::RosePineDawn,
            "latte" | "daylight" | "light" | "claro" => Self::DaylightLatte,
            _ => Self::Obsidian,
        }
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Obsidian => ThemePalette {
                bg: 0x141618,
                panel: 0x1c1e20,
                panel_hover: 0x222527,
                panel_dark: 0x111314,
                border: 0x2a2d30,
                text: 0xd4d8db,
                muted: 0x6b7280,
                accent: 0x5b9cf6,
                accent_dim: 0x1e3a5f,
                success: 0x52c77a,
                warning: 0xe8b656,
                danger: 0xe05252,
                cyan: 0x64d9ca,
                orange: 0xe9a77a,
                pink: 0xd98bc7,
                grid: 0x3a3d40,
            },
            Self::MameyStudio => ThemePalette {
                bg: 0x171312,
                panel: 0x221c1a,
                panel_hover: 0x2c2421,
                panel_dark: 0x110e0d,
                border: 0x3a2e2a,
                text: 0xf4e8e1,
                muted: 0x9c877e,
                accent: 0xff6b35,
                accent_dim: 0x4d2212,
                success: 0x5ad484,
                warning: 0xffb648,
                danger: 0xf05454,
                cyan: 0x5ad2c8,
                orange: 0xff8a54,
                pink: 0xf28ba8,
                grid: 0x362a26,
            },
            Self::CatppuccinMocha => ThemePalette {
                bg: 0x1e1e2e,
                panel: 0x24273a,
                panel_hover: 0x313244,
                panel_dark: 0x181825,
                border: 0x45475a,
                text: 0xcdd6f4,
                muted: 0x7f849c,
                accent: 0xcba6f7,
                accent_dim: 0x3b2e58,
                success: 0xa6e3a1,
                warning: 0xf9e2af,
                danger: 0xf38ba8,
                cyan: 0x94e2d5,
                orange: 0xfab387,
                pink: 0xf5c2e7,
                grid: 0x363a4f,
            },
            Self::Cyberpunk => ThemePalette {
                bg: 0x0b0716,
                panel: 0x140d26,
                panel_hover: 0x1f1438,
                panel_dark: 0x07040e,
                border: 0x331c54,
                text: 0xf2e9ff,
                muted: 0x8a73a6,
                accent: 0xff2a85,
                accent_dim: 0x4d1030,
                success: 0x05ffa1,
                warning: 0xffe600,
                danger: 0xff0055,
                cyan: 0x00f0ff,
                orange: 0xff9e00,
                pink: 0xd946ef,
                grid: 0x291749,
            },
            Self::GruvboxDark => ThemePalette {
                bg: 0x1d2021,
                panel: 0x282828,
                panel_hover: 0x32302f,
                panel_dark: 0x141617,
                border: 0x3c3836,
                text: 0xebdbb2,
                muted: 0x928374,
                accent: 0xd79921,
                accent_dim: 0x4c3811,
                success: 0x98971a,
                warning: 0xfabd2f,
                danger: 0xcc241d,
                cyan: 0x8ec07c,
                orange: 0xfe8019,
                pink: 0xd3869b,
                grid: 0x504945,
            },
            Self::TokyoNight => ThemePalette {
                bg: 0x1a1b26,
                panel: 0x24283b,
                panel_hover: 0x2f3549,
                panel_dark: 0x16161e,
                border: 0x3b4261,
                text: 0xc0caf5,
                muted: 0x565f89,
                accent: 0x7aa2f7,
                accent_dim: 0x283457,
                success: 0x9ece6a,
                warning: 0xe0af68,
                danger: 0xf7768e,
                cyan: 0x7dcfff,
                orange: 0xff9e64,
                pink: 0xbb9af7,
                grid: 0x292e42,
            },
            Self::RosePine => ThemePalette {
                bg: 0x191724,
                panel: 0x1f1d2e,
                panel_hover: 0x26233a,
                panel_dark: 0x12101b,
                border: 0x403d52,
                text: 0xe0def4,
                muted: 0x6e6a86,
                accent: 0xebbcba,
                accent_dim: 0x44323b,
                success: 0x9ccfd8,
                warning: 0xf6c177,
                danger: 0xeb6f92,
                cyan: 0x31748f,
                orange: 0xea9a97,
                pink: 0xc4a7e7,
                grid: 0x393552,
            },
            Self::NordAurora => ThemePalette {
                bg: 0x242933,
                panel: 0x2e3440,
                panel_hover: 0x3b4252,
                panel_dark: 0x1d212a,
                border: 0x434c5e,
                text: 0xeceff4,
                muted: 0x7b88a1,
                accent: 0x88c0d0,
                accent_dim: 0x2b4252,
                success: 0xa3be8c,
                warning: 0xebcb8b,
                danger: 0xbf616a,
                cyan: 0x8fbcbb,
                orange: 0xd08770,
                pink: 0xb48ead,
                grid: 0x4c566a,
            },
            Self::SilverHardware => ThemePalette {
                bg: 0xc2c8d1,
                panel: 0xd7dce3,
                panel_hover: 0xcdd3dc,
                panel_dark: 0xb6bcc6,
                border: 0x98a1ae,
                text: 0x1c2026,
                muted: 0x525a66,
                accent: 0xff5500,
                accent_dim: 0xebcab8,
                success: 0x1f8a4c,
                warning: 0xc87a10,
                danger: 0xc92a2a,
                cyan: 0x0f7c90,
                orange: 0xe8590c,
                pink: 0xb53882,
                grid: 0xa6afbc,
            },
            Self::GruvboxPaper => ThemePalette {
                bg: 0xd5c4a1,
                panel: 0xebdbb2,
                panel_hover: 0xe2d0a6,
                panel_dark: 0xc8b693,
                border: 0xbdae93,
                text: 0x3c3836,
                muted: 0x665c54,
                accent: 0xaf3a03,
                accent_dim: 0xe0c097,
                success: 0x79740e,
                warning: 0xb57614,
                danger: 0x9d0006,
                cyan: 0x427b58,
                orange: 0xaf3a03,
                pink: 0x8f3f71,
                grid: 0xbdae93,
            },
            Self::MameyCream => ThemePalette {
                bg: 0xdfd3c8,
                panel: 0xf2e9e1,
                panel_hover: 0xe8ddd3,
                panel_dark: 0xd4c5b8,
                border: 0xbdaaa0,
                text: 0x2d211c,
                muted: 0x6e5a50,
                accent: 0xe85a24,
                accent_dim: 0xf2cfc0,
                success: 0x2b8a4e,
                warning: 0xc97a16,
                danger: 0xc92a35,
                cyan: 0x188088,
                orange: 0xff6b35,
                pink: 0xc44d78,
                grid: 0xc4b3a8,
            },
            Self::MatchaMist => ThemePalette {
                bg: 0xd3dbcc,
                panel: 0xe7ede1,
                panel_hover: 0xdce4d5,
                panel_dark: 0xc7d1bf,
                border: 0xaebaa4,
                text: 0x27302c,
                muted: 0x58665d,
                accent: 0x2f855a,
                accent_dim: 0xc3decf,
                success: 0x2f855a,
                warning: 0xb87614,
                danger: 0xc53030,
                cyan: 0x237a82,
                orange: 0xd96026,
                pink: 0xa84a7e,
                grid: 0xb4c0aa,
            },
            Self::RosePineDawn => ThemePalette {
                bg: 0xe6dcd5,
                panel: 0xf7f1eb,
                panel_hover: 0xeee5dd,
                panel_dark: 0xdcd0c7,
                border: 0xc8bab0,
                text: 0x464261,
                muted: 0x797593,
                accent: 0xb4637a,
                accent_dim: 0xebd0d6,
                success: 0x286983,
                warning: 0xea9d34,
                danger: 0xb4637a,
                cyan: 0x56949f,
                orange: 0xd7827e,
                pink: 0x907aa9,
                grid: 0xcebfba,
            },
            Self::DaylightLatte => ThemePalette {
                bg: 0xdce0e8,
                panel: 0xf2f4f8,
                panel_hover: 0xe6e9ef,
                panel_dark: 0xd2d6e0,
                border: 0xb8becc,
                text: 0x383a4e,
                muted: 0x6c6f85,
                accent: 0x1e66f5,
                accent_dim: 0xcbd9f8,
                success: 0x358f22,
                warning: 0xc97d12,
                danger: 0xc81038,
                cyan: 0x148288,
                orange: 0xfe640b,
                pink: 0xd255b2,
                grid: 0xbcc0cc,
            },
        }
    }
}

#[derive(Clone)]
pub(super) struct DesignSystem {
    pub theme: ThemePreset,
    /// Intensidad del contraste de selección (0–0.4), independiente del hover.
    pub selection_contrast: f32,
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
    /// Grosor del marco de los clips en px (`CLIP_BORDER`). Seleccionado refuerza el marco según SELECTION_CONTRAST.
    pub clip_border: f32,
    pub playhead_particle_rate: f32,
    pub playhead_particle_life: f32,
    pub playhead_particle_size: f32,
    pub playhead_particle_speed: f32,
    pub playhead_particle_gravity: f32,
    pub playhead_particle_color: u32,
    pub cursor_fx: CursorFxMode,
    pub track_header: TrackHeaderConfig,
    /// Color elegido por el usuario por pista (clic derecho). `None` = color del tema.
    pub track_color_override: Vec<Option<u32>>,
}

impl DesignSystem {
    pub(super) fn load() -> Self {
        Self::from_profile(&super::profile::load().unwrap_or_default())
    }
    pub(super) fn from_profile(content: &str) -> Self {
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
        let theme = map
            .get("THEME")
            .copied()
            .map(ThemePreset::from_token)
            .unwrap_or_default();
        let pal = theme.palette();
        let success = color("SUCCESS", pal.success);
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
            theme,
            selection_contrast: get("SELECTION_CONTRAST", 0.18).min(0.4),
            bg: color("BG", pal.bg),
            panel: color("PANEL", pal.panel),
            panel_hover: color("PANEL_HOVER", pal.panel_hover),
            panel_dark: color("PANEL_DARK", pal.panel_dark),
            border: color("BORDER", pal.border),
            text: color("TEXT", pal.text),
            muted: color("MUTED", pal.muted),
            accent: color("ACCENT", pal.accent),
            accent_dim: color("ACCENT_DIM", pal.accent_dim),
            success,
            warning: color("WARNING", pal.warning),
            danger: color("DANGER", pal.danger),
            cyan: color("CYAN", pal.cyan),
            orange: color("ORANGE", pal.orange),
            pink: color("PINK", pal.pink),
            grid: color("GRID", pal.grid),
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
            knob_size: get("KNOB_SIZE", 28.0).clamp(18.0, 36.0),
            transport_btn_size: get("TRANSPORT_BTN_SIZE", 28.0),
            font_size_xs: get("FONT_SIZE_XS", 11.0),
            padding_xs: get("PADDING_XS", 4.0),
            padding_sm: get("PADDING_SM", 8.0),
            padding_lg: get("PADDING_LG", 16.0),
            radius_sm: get("RADIUS_SM", 3.0),
            radius_md: get("RADIUS_MD", 4.0),
            radius_lg: get("RADIUS_LG", 6.0),
            clip_border: get("CLIP_BORDER", 1.0).clamp(0.0, 4.0),
            playhead_particle_rate: get("PLAYHEAD_PARTICLE_RATE", 120.0).min(2000.0),
            playhead_particle_life: get("PLAYHEAD_PARTICLE_LIFE", 0.6).clamp(0.05, 3.0),
            playhead_particle_size: get("PLAYHEAD_PARTICLE_SIZE", 2.0).clamp(0.5, 12.0),
            playhead_particle_speed: get("PLAYHEAD_PARTICLE_SPEED", 70.0).min(1000.0),
            playhead_particle_gravity: get("PLAYHEAD_PARTICLE_GRAVITY", 60.0).min(1000.0),
            playhead_particle_color: color("PLAYHEAD_PARTICLE_COLOR", success),
            cursor_fx,
            track_header,
            track_color_override: vec![None; 8],
        }
    }

    /// Acerca el fondo al color de texto: aclara temas oscuros y oscurece temas claros.
    pub(in crate::ui) fn selection_color(&self, base: u32) -> u32 {
        let channel = |shift: u32| {
            let from = ((base >> shift) & 255) as f32;
            let to = ((self.text >> shift) & 255) as f32;
            (from + (to - from) * self.selection_contrast).round() as u32
        };
        (channel(16) << 16) | (channel(8) << 8) | channel(0)
    }

    pub(in crate::ui) fn selection_background(&self) -> u32 {
        self.selection_color(self.panel)
    }

    /// Colores seleccionables para una pista (el mismo orden que el color automático).
    /// Orden estable de los índices de color almacenados en cada clip.
    pub(in crate::ui) fn clip_palette(&self) -> [u32; 7] {
        [
            self.orange,
            self.cyan,
            self.success,
            self.pink,
            self.warning,
            self.danger,
            self.accent,
        ]
    }

    pub(in crate::ui) fn track_palette(&self) -> [u32; 7] {
        [
            self.accent,
            self.success,
            self.warning,
            self.danger,
            self.cyan,
            self.orange,
            self.pink,
        ]
    }

    /// Color de una pista: el elegido por el usuario o el automático del tema.
    pub(in crate::ui) fn track_color(&self, track: usize) -> u32 {
        self.track_color_override
            .get(track)
            .copied()
            .flatten()
            .unwrap_or_else(|| {
                let palette = self.track_palette();
                palette[track % palette.len()]
            })
    }

    pub(in crate::ui) fn apply_theme(&mut self, theme: ThemePreset) {
        let pal = theme.palette();
        self.theme = theme;
        self.bg = pal.bg;
        self.panel = pal.panel;
        self.panel_hover = pal.panel_hover;
        self.panel_dark = pal.panel_dark;
        self.border = pal.border;
        self.text = pal.text;
        self.muted = pal.muted;
        self.accent = pal.accent;
        self.accent_dim = pal.accent_dim;
        self.success = pal.success;
        self.warning = pal.warning;
        self.danger = pal.danger;
        self.cyan = pal.cyan;
        self.orange = pal.orange;
        self.pink = pal.pink;
        self.grid = pal.grid;
        self.playhead_particle_color = pal.success;
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
