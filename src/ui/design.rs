//! Tokens de ~/.jaybu.profile y temas de color integrados.
use super::{cursor_fx::CursorFxMode, views::tracks::TrackHeaderConfig};
use elevate::musical::clip::ClipProps;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui) enum ThemePreset {
    #[default]
    Obsidian,
    AbletonLiveDark,
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
    DaynightStudio,
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
    pub const ALL: [Self; 16] = [
        Self::Obsidian,
        Self::AbletonLiveDark,
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
        Self::DaynightStudio,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Obsidian => "Obsidiana",
            Self::AbletonLiveDark => "Ableton Live Dark",
            Self::MameyStudio => "Mamey Studio",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::Cyberpunk => "Cyberpunk Studio",
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
            Self::DaynightStudio => "Daynight Studio",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Obsidian => "obsidian",
            Self::AbletonLiveDark => "ableton-live-dark",
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
            Self::DaynightStudio => "daynight",
        }
    }

    pub fn from_token(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ableton-live-dark" | "ableton-dark" | "live-dark" => Self::AbletonLiveDark,
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
            "daynight" | "daynight-studio" | "daylight-dark" => Self::DaynightStudio,
            _ => Self::Obsidian,
        }
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Obsidian => ThemePalette {
                bg: 0x303132,
                panel: 0x3b3c3d,
                panel_hover: 0x46484a,
                panel_dark: 0x252728,
                border: 0x55585a,
                text: 0xd3d4d2,
                muted: 0x9b9e9f,
                accent: 0x87a6b8,
                accent_dim: 0x3c4b54,
                success: 0x91ad88,
                warning: 0xc6ad70,
                danger: 0xc47e78,
                cyan: 0x80aaa5,
                orange: 0xbd9b7e,
                pink: 0xaa91a4,
                grid: 0x47494b,
            },
            Self::AbletonLiveDark => ThemePalette {
                bg: 0x282a2b,
                panel: 0x333537,
                panel_hover: 0x404345,
                panel_dark: 0x202223,
                border: 0x525659,
                text: 0xcbd0d1,
                muted: 0x969da1,
                accent: 0xc0a16c,
                accent_dim: 0x4b4233,
                success: 0x8cac91,
                warning: 0xb9ad79,
                danger: 0xc1837b,
                cyan: 0x7fa9a3,
                orange: 0xc19c83,
                pink: 0xa494b9,
                grid: 0x424649,
            },
            Self::MameyStudio => ThemePalette {
                bg: 0x161413,
                panel: 0x201d1c,
                panel_hover: 0x292524,
                panel_dark: 0x100e0e,
                border: 0x36302e,
                text: 0xeeeae7,
                muted: 0x948a86,
                accent: 0xbc8a78,
                accent_dim: 0x3e2921,
                success: 0x77b78d,
                warning: 0xc3aa84,
                danger: 0xc28282,
                cyan: 0x77b5b0,
                orange: 0xc69d8b,
                pink: 0xc68b9b,
                grid: 0x322c2a,
            },
            Self::CatppuccinMocha => ThemePalette {
                bg: 0x22222a,
                panel: 0x2a2b34,
                panel_hover: 0x36363f,
                panel_dark: 0x1b1b22,
                border: 0x4a4b55,
                text: 0xdadde7,
                muted: 0x878994,
                accent: 0xa68bc6,
                accent_dim: 0x3f394e,
                success: 0x94c190,
                warning: 0xc6b38b,
                danger: 0xc68b9b,
                cyan: 0x8ec2ba,
                orange: 0xc6a18b,
                pink: 0xc68bb6,
                grid: 0x3c3e49,
            },
            Self::Cyberpunk => ThemePalette {
                bg: 0x0d0b12,
                panel: 0x171320,
                panel_hover: 0x221d2f,
                panel_dark: 0x08070b,
                border: 0x352a46,
                text: 0xf3f0f8,
                muted: 0x8b8198,
                accent: 0xb9708f,
                accent_dim: 0x3e1f2f,
                success: 0x58ad8d,
                warning: 0xada458,
                danger: 0xad5874,
                cyan: 0x58a8ad,
                orange: 0xad8c58,
                pink: 0xb478bd,
                grid: 0x2d243c,
            },
            Self::GruvboxDark => ThemePalette {
                bg: 0x1e1f20,
                panel: 0x282828,
                panel_hover: 0x313030,
                panel_dark: 0x151616,
                border: 0x3a3837,
                text: 0xd8d3c5,
                muted: 0x928c86,
                accent: 0xad9058,
                accent_dim: 0x3d3320,
                success: 0xadac58,
                warning: 0xb9a370,
                danger: 0xad5b58,
                cyan: 0x96b08c,
                orange: 0xb38864,
                pink: 0xbd939f,
                grid: 0x4d4a48,
            },
            Self::TokyoNight => ThemePalette {
                bg: 0x1d1d23,
                panel: 0x2a2c35,
                panel_hover: 0x353842,
                panel_dark: 0x18181c,
                border: 0x454858,
                text: 0xd1d5e4,
                muted: 0x808598,
                accent: 0x8b9ec6,
                accent_dim: 0x343a4b,
                success: 0x9db682,
                warning: 0xc3aa85,
                danger: 0xc68b96,
                cyan: 0x8bb0c6,
                orange: 0xc6a18b,
                pink: 0xa08bc6,
                grid: 0x2f323c,
            },
            Self::RosePine => ThemePalette {
                bg: 0x1b1a21,
                panel: 0x22212a,
                panel_hover: 0x2a2934,
                panel_dark: 0x141318,
                border: 0x44424d,
                text: 0xe6e5ed,
                muted: 0x888692,
                accent: 0xc1928f,
                accent_dim: 0x40373b,
                success: 0x95b6bc,
                warning: 0xc6ad8b,
                danger: 0xc68b9b,
                cyan: 0x6290a2,
                orange: 0xc68d8b,
                pink: 0xa68fc2,
                grid: 0x3e3c4b,
            },
            Self::NordAurora => ThemePalette {
                bg: 0x282a2f,
                panel: 0x33363c,
                panel_hover: 0x41444c,
                panel_dark: 0x202227,
                border: 0x4a4e57,
                text: 0xeff0f1,
                muted: 0x858b97,
                accent: 0x95b3bc,
                accent_dim: 0x354048,
                success: 0xa4b298,
                warning: 0xc6b28b,
                danger: 0xa8787c,
                cyan: 0x9ab1b1,
                orange: 0xb99387,
                pink: 0xab97a7,
                grid: 0x545962,
            },
            Self::SilverHardware => ThemePalette {
                bg: 0xc6c9cd,
                panel: 0xdadce0,
                panel_hover: 0xd1d4d8,
                panel_dark: 0xbabdc2,
                border: 0x9da2a8,
                text: 0x1f2123,
                muted: 0x585b60,
                accent: 0xa16a4f,
                accent_dim: 0xdecec5,
                success: 0x397050,
                warning: 0x917247,
                danger: 0xa14f4f,
                cyan: 0x34626b,
                orange: 0xa16c4f,
                pink: 0x97567c,
                grid: 0xacb0b6,
            },
            Self::GruvboxPaper => ThemePalette {
                bg: 0xc8c0ae,
                panel: 0xddd5c0,
                panel_hover: 0xd3cab5,
                panel_dark: 0xbbb2a0,
                border: 0xb2ab9e,
                text: 0x3a3938,
                muted: 0x615d59,
                accent: 0x774e3b,
                accent_dim: 0xcebea9,
                success: 0x5a582d,
                warning: 0x876c42,
                danger: 0x693436,
                cyan: 0x506d5b,
                orange: 0x774e3b,
                pink: 0x7c526c,
                grid: 0xb2ab9e,
            },
            Self::MameyCream => ThemePalette {
                bg: 0xd9d3ce,
                panel: 0xeee9e5,
                panel_hover: 0xe3ddd8,
                panel_dark: 0xcdc6bf,
                border: 0xb6aca7,
                text: 0x272322,
                muted: 0x665d58,
                accent: 0xa1664f,
                accent_dim: 0xe5d4cc,
                success: 0x427354,
                warning: 0x95744a,
                danger: 0xa14f55,
                cyan: 0x35676b,
                orange: 0xa1654f,
                pink: 0x97596f,
                grid: 0xbdb4af,
            },
            Self::MatchaMist => ThemePalette {
                bg: 0xd3d7d0,
                panel: 0xe7eae4,
                panel_hover: 0xdce0d9,
                panel_dark: 0xc8ccc3,
                border: 0xaeb4aa,
                text: 0x2a2d2c,
                muted: 0x5c625e,
                accent: 0x44705a,
                accent_dim: 0xcad7d0,
                success: 0x44705a,
                warning: 0x896d43,
                danger: 0x9e5252,
                cyan: 0x3a676b,
                orange: 0xa16a4f,
                pink: 0x90607a,
                grid: 0xb4baaf,
            },
            Self::RosePineDawn => ThemePalette {
                bg: 0xe2ddd9,
                panel: 0xf4f1ee,
                panel_hover: 0xeae5e1,
                panel_dark: 0xd7d1cc,
                border: 0xc2bbb6,
                text: 0x4d4c57,
                muted: 0x7f7d8b,
                accent: 0x8e626e,
                accent_dim: 0xe4d7da,
                success: 0x3e606d,
                warning: 0xa17e4f,
                danger: 0x8e626e,
                cyan: 0x65858a,
                orange: 0x995a57,
                pink: 0x776a85,
                grid: 0xc9c2bf,
            },
            Self::DaylightLatte => ThemePalette {
                bg: 0xdfe1e5,
                panel: 0xf3f4f6,
                panel_hover: 0xe8eaed,
                panel_dark: 0xd5d8dc,
                border: 0xbdc0c7,
                text: 0x3f4047,
                muted: 0x73747e,
                accent: 0x4f6aa1,
                accent_dim: 0xd6dded,
                success: 0x46753c,
                warning: 0x937448,
                danger: 0x914757,
                cyan: 0x336669,
                orange: 0xa16d4f,
                pink: 0x9c548a,
                grid: 0xc0c2c8,
            },
            // Variante oscura de Daylight: superficies separadas y acentos mates.
            Self::DaynightStudio => ThemePalette {
                bg: 0x252930,
                panel: 0x383e48,
                panel_hover: 0x484f5b,
                panel_dark: 0x1b1f26,
                border: 0x606a79,
                text: 0xdfe3ea,
                muted: 0xa8b0bd,
                accent: 0x94abd2,
                accent_dim: 0x414e65,
                success: 0x98b28e,
                warning: 0xc5ad83,
                danger: 0xc68f9b,
                cyan: 0x8ab5b8,
                orange: 0xc3a08a,
                pink: 0xbb97b1,
                grid: 0x464e5b,
            },
        }
    }
}

#[derive(Clone, Copy)]
struct ThemeLayout {
    selection_contrast: f32,
    track_header_width: f32,
    track_header_height: f32,
    timeline_bar_height: f32,
    timeline_pixels_per_beat: f32,
    title_bar_height: f32,
    transport_height: f32,
    status_bar_height: f32,
    pan_bar_width: f32,
    vol_bar_width: f32,
    control_bar_height: f32,
    transport_btn_size: f32,
    font_size_xs: f32,
    padding_xs: f32,
    padding_sm: f32,
    padding_lg: f32,
    radius_sm: f32,
    radius_md: f32,
    radius_lg: f32,
}

impl ThemeLayout {
    const STANDARD: Self = Self {
        selection_contrast: 0.18,
        track_header_width: 200.0,
        track_header_height: 76.0,
        timeline_bar_height: 28.0,
        timeline_pixels_per_beat: 32.0,
        title_bar_height: 36.0,
        transport_height: 44.0,
        status_bar_height: 28.0,
        pan_bar_width: 40.0,
        vol_bar_width: 40.0,
        control_bar_height: 6.0,
        transport_btn_size: 28.0,
        font_size_xs: 11.0,
        padding_xs: 4.0,
        padding_sm: 8.0,
        padding_lg: 16.0,
        radius_sm: 3.0,
        radius_md: 4.0,
        radius_lg: 6.0,
    };

    const ABLETON: Self = Self {
        selection_contrast: 0.22,
        track_header_width: 180.0,
        track_header_height: 58.0,
        timeline_bar_height: 24.0,
        timeline_pixels_per_beat: 32.0,
        title_bar_height: 28.0,
        transport_height: 36.0,
        status_bar_height: 22.0,
        pan_bar_width: 36.0,
        vol_bar_width: 40.0,
        control_bar_height: 5.0,
        transport_btn_size: 24.0,
        font_size_xs: 10.0,
        padding_xs: 3.0,
        padding_sm: 6.0,
        padding_lg: 12.0,
        radius_sm: 1.0,
        radius_md: 2.0,
        radius_lg: 3.0,
    };
}

impl ThemePreset {
    fn layout_defaults(self) -> ThemeLayout {
        if self == Self::AbletonLiveDark {
            ThemeLayout::ABLETON
        } else {
            ThemeLayout::STANDARD
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
        let layout = theme.layout_defaults();
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
            selection_contrast: get("SELECTION_CONTRAST", layout.selection_contrast).min(0.4),
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
            track_header_width: get("TRACK_HEADER_WIDTH", layout.track_header_width),
            track_header_height: get("TRACK_HEADER_HEIGHT", layout.track_header_height).max(1.0),
            timeline_bar_height: get("TIMELINE_BAR_HEIGHT", layout.timeline_bar_height).max(1.0),
            timeline_pixels_per_beat: get(
                "TIMELINE_PIXELS_PER_BEAT",
                layout.timeline_pixels_per_beat,
            ),
            title_bar_height: get("TITLE_BAR_HEIGHT", layout.title_bar_height),
            transport_height: get("TRANSPORT_HEIGHT", layout.transport_height),
            status_bar_height: get("STATUS_BAR_HEIGHT", layout.status_bar_height),
            pan_bar_width: get("PAN_BAR_WIDTH", layout.pan_bar_width),
            vol_bar_width: get("VOL_BAR_WIDTH", layout.vol_bar_width),
            control_bar_height: get("CONTROL_BAR_HEIGHT", layout.control_bar_height),
            knob_size: get("KNOB_SIZE", 28.0).clamp(18.0, 36.0),
            transport_btn_size: get("TRANSPORT_BTN_SIZE", layout.transport_btn_size),
            font_size_xs: get("FONT_SIZE_XS", layout.font_size_xs),
            padding_xs: get("PADDING_XS", layout.padding_xs),
            padding_sm: get("PADDING_SM", layout.padding_sm),
            padding_lg: get("PADDING_LG", layout.padding_lg),
            radius_sm: get("RADIUS_SM", layout.radius_sm),
            radius_md: get("RADIUS_MD", layout.radius_md),
            radius_lg: get("RADIUS_LG", layout.radius_lg),
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

    pub(in crate::ui) fn clip_color(&self, track: usize, props: ClipProps) -> u32 {
        let palette = self.clip_palette();
        props
            .color_index
            .map(|index| palette[usize::from(index) % palette.len()])
            .unwrap_or_else(|| self.track_color(track))
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
