//! Superficies del Arrangement derivadas del tema activo.
use crate::ui::{DesignSystem, design::blend_color};

impl DesignSystem {
    pub(in crate::ui) fn arrangement_surface(&self) -> u32 {
        let brightness = |color: u32| {
            54 * ((color >> 16) & 255) + 183 * ((color >> 8) & 255) + 19 * (color & 255)
        };
        if brightness(self.text) > brightness(self.panel) {
            blend_color(self.panel, self.muted, 24)
        } else {
            self.bg
        }
    }

    pub(in crate::ui) fn arrangement_header(&self) -> u32 {
        blend_color(self.panel, self.arrangement_surface(), 48)
    }

    pub(in crate::ui) fn arrangement_selection(&self, base: u32) -> u32 {
        blend_color(
            base,
            self.accent,
            (self.selection_contrast * 65.0).round() as u32,
        )
    }
}
