//! Tokens de ~/.jaybu.profile y temas de color integrados.
use super::{cursor_fx::CursorFxMode, views::tracks::TrackHeaderConfig};
use elevate::musical::clip::ClipProps;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui) enum ThemePreset {
    #[default]
    Cyberpunk,
    DaylightStudio,
    Cuaderno,
    CuadernoOscuro,
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
    pub const ALL: [Self; 4] = [
        Self::Cyberpunk,
        Self::DaylightStudio,
        Self::Cuaderno,
        Self::CuadernoOscuro,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cyberpunk => "Cyberpunk Studio",
            Self::DaylightStudio => "Daylight Studio",
            Self::Cuaderno => "Cuaderno Claro",
            Self::CuadernoOscuro => "Cuaderno Oscuro",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::Cyberpunk => "cyberpunk",
            Self::DaylightStudio => "daylight",
            Self::Cuaderno => "cuaderno",
            Self::CuadernoOscuro => "cuaderno-oscuro",
        }
    }

    pub fn from_token(raw: &str) -> Self {
        let cleaned = raw
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .trim()
            .to_ascii_lowercase();
        match cleaned.as_str() {
            "daylight" | "daylight-studio" | "daylight studio" | "daylight_studio"
            | "daylightlatte" | "latte" | "light" | "claro" => Self::DaylightStudio,
            "cuaderno" | "cuaderno-claro" | "cuaderno claro" | "cuaderno_claro" | "sketchbook"
            | "sketchbook-light" | "sketchbook light" | "paper" | "papel" => Self::Cuaderno,
            "cuaderno-oscuro" | "cuaderno oscuro" | "cuaderno_oscuro" | "cuaderno-dark"
            | "cuaderno dark" | "sketchbook-dark" | "sketchbook dark" | "pizarra" => {
                Self::CuadernoOscuro
            }
            _ => Self::Cyberpunk,
        }
    }

    pub fn palette(self) -> ThemePalette {
        match self {
            Self::Cyberpunk => ThemePalette {
                // Chasis continuo mate índigo-obsidiana: los objetos salen de la misma base.
                bg: 0x171522,
                panel: 0x191725,
                panel_hover: 0x221f33,
                panel_dark: 0x110f1a,
                border: 0x2c2742,
                text: 0xf4f2fc,
                muted: 0x9990b8,
                accent: 0xff2e93,
                accent_dim: 0x421633,
                success: 0x00e699,
                warning: 0xffc43d,
                danger: 0xff3b5c,
                cyan: 0x00e5ff,
                orange: 0xff7a2f,
                pink: 0xd86bff,
                grid: 0x262238,
            },
            Self::DaylightStudio => ThemePalette {
                // Chasis continuo aluminio/cerámica mate (Dieter Rams / Soft UI):
                // bg y panel comparten el mismo material para que cada botón nazca de la base.
                bg: 0xe3e7ee,
                panel: 0xe5e9f0,
                panel_hover: 0xedf0f5,
                panel_dark: 0xd5dbe5,
                border: 0xc4ccd9,
                text: 0x171b24,
                muted: 0x5c6578,
                accent: 0x2563eb,
                accent_dim: 0xdbeafe,
                success: 0x0fa958,
                warning: 0xe08200,
                danger: 0xe63946,
                cyan: 0x0284c7,
                orange: 0xff5a1f,
                pink: 0x9333ea,
                grid: 0xcbd1de,
            },
            Self::Cuaderno => ThemePalette {
                // Cuaderno de bocetos claro (landing/index.html): papel crema, grafito y tintas cálidas.
                bg: 0xf1e8d4,
                panel: 0xf4ecda,
                panel_hover: 0xfbf4de,
                panel_dark: 0xe4d9c0,
                border: 0xc9bba0,
                text: 0x3a3835,
                muted: 0x6b6458,
                accent: 0xc84c14,
                accent_dim: 0xf2d7c2,
                success: 0x357547,
                warning: 0x9c6b0e,
                danger: 0xb8321f,
                cyan: 0x2f4f7a,
                orange: 0xba5e14,
                pink: 0x8f3f6c,
                grid: 0xd8cfbc,
            },
            Self::CuadernoOscuro => ThemePalette {
                // Cuaderno de bocetos oscuro (landing/index.html dark): papel carbón cálido y trazos tiza.
                bg: 0x1b1a18,
                panel: 0x211f1c,
                panel_hover: 0x2c2925,
                panel_dark: 0x141311,
                border: 0x3d3832,
                text: 0xe8e0d2,
                muted: 0xa39b8e,
                accent: 0xff8243,
                accent_dim: 0x4a2919,
                success: 0x6fbe82,
                warning: 0xe6bf55,
                danger: 0xee6b57,
                cyan: 0x7ca7db,
                orange: 0xe8a454,
                pink: 0xd67fa8,
                grid: 0x302c27,
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
    padding_xs: f32,
    padding_sm: f32,
    padding_lg: f32,
    radius_sm: f32,
    radius_md: f32,
    radius_lg: f32,
}

impl ThemeLayout {
    const STANDARD: Self = Self {
        selection_contrast: 0.22,
        track_header_width: 204.0,
        track_header_height: 74.0,
        timeline_bar_height: 34.0,
        timeline_pixels_per_beat: 32.0,
        title_bar_height: 34.0,
        transport_height: 48.0,
        status_bar_height: 28.0,
        pan_bar_width: 40.0,
        vol_bar_width: 40.0,
        control_bar_height: 6.0,
        transport_btn_size: 28.0,
        padding_xs: 4.0,
        padding_sm: 8.0,
        padding_lg: 16.0,
        radius_sm: 6.0,
        radius_md: 8.0,
        radius_lg: 12.0,
    };
}

impl ThemePreset {
    fn layout_defaults(self) -> ThemeLayout {
        let _ = self;
        ThemeLayout::STANDARD
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
                .and_then(|value| {
                    let hex = value.trim().trim_start_matches('#');
                    u32::from_str_radix(hex, 16).ok()
                })
                .filter(|value| *value <= 0xffffff)
                .unwrap_or(default)
        };
        let theme = map
            .get("THEME")
            .copied()
            .map(ThemePreset::from_token)
            .unwrap_or_default();
        const DEFAULT_UI_FONT_SIZE: f32 = 12.0;
        const MIN_UI_FONT_SIZE: f32 = 10.0;
        const MAX_UI_FONT_SIZE: f32 = 20.0;
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
            font_size_xs: get("FONT_SIZE_XS", DEFAULT_UI_FONT_SIZE)
                .clamp(MIN_UI_FONT_SIZE, MAX_UI_FONT_SIZE),
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

    /// Tamaño base `rem` de GPUI derivado de `font_size_xs` (12 px → 16 rem = 100%).
    pub(in crate::ui) fn rem_size(&self) -> f32 {
        (self.font_size_xs * (16.0 / 12.0)).clamp(13.0, 27.0)
    }

    pub(in crate::ui) fn font_scale_percent(&self) -> u32 {
        ((self.font_size_xs / 12.0) * 100.0).round() as u32
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

    /// True para temas oscuros (como Cyberpunk Studio), false para temas claros (Daylight Studio).
    pub(in crate::ui) fn is_dark(&self) -> bool {
        let r = ((self.bg >> 16) & 0xff) as f32;
        let g = ((self.bg >> 8) & 0xff) as f32;
        let b = (self.bg & 0xff) as f32;
        (0.299 * r + 0.587 * g + 0.114 * b) < 128.0
    }

    pub(in crate::ui) fn matches_theme_palette(&self) -> bool {
        let pal = self.theme.palette();
        self.bg == pal.bg
            && self.panel == pal.panel
            && self.panel_hover == pal.panel_hover
            && self.panel_dark == pal.panel_dark
            && self.border == pal.border
            && self.text == pal.text
            && self.muted == pal.muted
            && self.accent == pal.accent
            && self.accent_dim == pal.accent_dim
            && self.success == pal.success
            && self.warning == pal.warning
            && self.danger == pal.danger
            && self.cyan == pal.cyan
            && self.orange == pal.orange
            && self.pink == pal.pink
            && self.grid == pal.grid
            && self.playhead_particle_color == pal.success
    }

    /// Color de la luz suave superior-izquierda (Soft UI / Dieter Rams).
    pub(in crate::ui) fn shadow_light(&self) -> u32 {
        if self.is_dark() {
            (blend_color(self.bg, 0xffffff, 16) << 8) | 0x66
        } else {
            0xfffffff4
        }
    }

    /// Color de la sombra suave inferior-derecha (Soft UI / Dieter Rams).
    pub(in crate::ui) fn shadow_dark(&self) -> u32 {
        if self.is_dark() {
            0x05040ad4
        } else {
            (blend_color(self.panel_dark, 0x000000, 22) << 8) | 0x92
        }
    }

    /// Borde superior ligeramente iluminado para superficies esculpidas.
    pub(in crate::ui) fn panel_top(&self) -> u32 {
        if self.is_dark() {
            blend_color(self.panel, 0xffffff, 3)
        } else {
            blend_color(self.panel, 0xffffff, 22)
        }
    }

    /// Borde inferior suavemente sombreado (muy sutil, sin efecto bisel metálico duro).
    pub(in crate::ui) fn panel_bottom(&self) -> u32 {
        if self.is_dark() {
            blend_color(self.panel, self.panel_dark, 14)
        } else {
            blend_color(self.panel, self.bg, 16)
        }
    }

    /// Superficie continua del chasis base sobre la que emergen paneles y controles.
    pub(in crate::ui) fn gutter_color(&self) -> u32 {
        self.bg
    }

    /// Contorno ultrasutil de separación (sin bordes duros tipo GTK/Ardour).
    pub(in crate::ui) fn frame_border(&self) -> u32 {
        if self.is_dark() {
            blend_color(self.panel, self.border, 45)
        } else {
            blend_color(self.panel, self.border, 38)
        }
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

    pub(in crate::ui) fn band_color(&self, selected: bool) -> u32 {
        if selected { self.accent } else { self.cyan }
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

pub(in crate::ui) fn blend_color(base: u32, tint: u32, tint_percent: u32) -> u32 {
    let blend = |shift: u32| -> u32 {
        let base = (base >> shift) & 0xff_u32;
        let tint = (tint >> shift) & 0xff_u32;
        (base * (100_u32 - tint_percent) + tint * tint_percent) / 100_u32
    };
    (blend(16) << 16) | (blend(8) << 8) | blend(0)
}

#[cfg(test)]
mod tests {
    use super::{DesignSystem, ThemePreset};
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
        assert_eq!(design.font_size_xs, 12.0);
        assert_eq!(design.cursor_fx, CursorFxMode::NeonComet);
        assert_eq!(design.track_header.preset(), Some(TcpLayoutPreset::Compact));
        assert!(!design.track_header.is_visible(TcpWidget::Rec));
    }

    #[test]
    fn cuaderno_light_and_dark_themes_parse_and_classify_polarity() {
        let light = DesignSystem::from_profile("THEME=cuaderno\n");
        assert_eq!(light.theme, ThemePreset::Cuaderno);
        assert!(!light.is_dark());
        assert_eq!(light.bg, 0xf1e8d4);
        assert_eq!(ThemePreset::from_token("sketchbook"), ThemePreset::Cuaderno);

        let dark = DesignSystem::from_profile("THEME=cuaderno-oscuro\n");
        assert_eq!(dark.theme, ThemePreset::CuadernoOscuro);
        assert!(dark.is_dark());
        assert_eq!(dark.bg, 0x1b1a18);
        assert_eq!(
            ThemePreset::from_token("sketchbook-dark"),
            ThemePreset::CuadernoOscuro
        );
    }
}
