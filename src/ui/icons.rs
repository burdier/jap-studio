//! Catálogo de iconos SVG (`assets/icons/toolbar/` y `~/.local/share/hay-daw/icons/`)
//! y vinculación estilo REAPER (Icono → Acción) en áreas libres del Transporte y Piano Roll.
use crate::ui::DesignSystem;
use gpui::{Div, Svg, div, prelude::*, px, rgb, svg};
use std::{fs, path::PathBuf};

const ICON_CAT_SLEEPING: &[u8] = include_bytes!("../../assets/icons/toolbar/cat-sleeping.svg");
const ICON_METRONOME: &[u8] = include_bytes!("../../assets/icons/toolbar/metronome.svg");
const ICON_SCISSORS: &[u8] = include_bytes!("../../assets/icons/toolbar/scissors.svg");
const ICON_MAGNET: &[u8] = include_bytes!("../../assets/icons/toolbar/magnet.svg");
const ICON_WAVE: &[u8] = include_bytes!("../../assets/icons/toolbar/wave.svg");
const ICON_PIANO: &[u8] = include_bytes!("../../assets/icons/toolbar/piano.svg");
const ICON_BOLT: &[u8] = include_bytes!("../../assets/icons/toolbar/bolt.svg");
const ICON_HEART: &[u8] = include_bytes!("../../assets/icons/toolbar/heart.svg");

pub(in crate::ui) use super::toolbar::{ToolbarAction, ToolbarScope};

#[derive(Clone)]
pub(in crate::ui) struct ToolbarIcon {
    pub id: String,
    pub label: String,
    pub svg_bytes: &'static [u8],
}

#[derive(Clone)]
pub(in crate::ui) struct BoundToolButton {
    pub icon_id: String,
    pub action: ToolbarAction,
    /// Comando libre introducido por la persona en el selector.
    pub custom_command: Option<String>,
}

pub(in crate::ui) struct ToolbarBindings {
    pub icons: Vec<ToolbarIcon>,
    pub transport_buttons: Vec<BoundToolButton>,
    pub piano_roll_buttons: Vec<BoundToolButton>,
    pub picker_open: bool,
    pub picker_scope: ToolbarScope,
    pub selected_icon: usize,
    pub selected_action: ToolbarAction,
    pub search_query: String,
    pub action_query: String,
    pub searching_actions: bool,
    pub custom_command: String,
    pub editing_custom_command: bool,
}

impl ToolbarBindings {
    pub fn new() -> Self {
        let mut this = Self {
            icons: builtin_icons(),
            transport_buttons: vec![
                BoundToolButton {
                    icon_id: "cat-sleeping".into(),
                    action: ToolbarAction::InsertMidiClip,
                    custom_command: None,
                },
                BoundToolButton {
                    icon_id: "wave".into(),
                    action: ToolbarAction::DuplicateClip,
                    custom_command: None,
                },
                BoundToolButton {
                    icon_id: "bolt".into(),
                    action: ToolbarAction::MidiLearn,
                    custom_command: None,
                },
            ],
            piano_roll_buttons: vec![
                BoundToolButton {
                    icon_id: "cat-sleeping".into(),
                    action: ToolbarAction::RollGhostNotes,
                    custom_command: None,
                },
                BoundToolButton {
                    icon_id: "scissors".into(),
                    action: ToolbarAction::RollErase,
                    custom_command: None,
                },
                BoundToolButton {
                    icon_id: "magnet".into(),
                    action: ToolbarAction::RollHighlightScale,
                    custom_command: None,
                },
            ],
            picker_open: false,
            picker_scope: ToolbarScope::Transport,
            selected_icon: 0,
            selected_action: ToolbarAction::InsertMidiClip,
            search_query: String::new(),
            action_query: String::new(),
            searching_actions: false,
            custom_command: String::new(),
            editing_custom_command: false,
        };
        this.reload_custom_icons();
        this
    }

    pub fn open_picker(&mut self, scope: ToolbarScope) {
        self.picker_open = true;
        self.picker_scope = scope;
        self.action_query.clear();
        self.searching_actions = false;
        self.custom_command.clear();
        self.editing_custom_command = false;
        if self.selected_action.scope() != scope {
            self.selected_action = match scope {
                ToolbarScope::Transport => ToolbarAction::InsertMidiClip,
                ToolbarScope::PianoRoll => ToolbarAction::RollGhostNotes,
            };
        }
    }

    pub fn icon_by_id(&self, id: &str) -> &ToolbarIcon {
        self.icons
            .iter()
            .find(|icon| icon.id == id)
            .unwrap_or(&self.icons[0])
    }

    /// Vincula el icono seleccionado a la acción seleccionada.
    /// Si la acción es de tipo Piano Roll se agrega al área libre del Piano Roll;
    /// si es global/transporte se agrega al área libre del Transporte.
    pub fn bind_selected(&mut self) -> (ToolbarScope, String, &'static str) {
        let icon_id = self
            .icons
            .get(self.selected_icon)
            .map(|i| i.id.clone())
            .unwrap_or_else(|| "cat-sleeping".into());
        let icon_label = self
            .icons
            .get(self.selected_icon)
            .map(|i| i.label.clone())
            .unwrap_or_else(|| "Gato".into());
        let action = self.selected_action;
        let scope = action.scope();
        let list = match scope {
            ToolbarScope::Transport => &mut self.transport_buttons,
            ToolbarScope::PianoRoll => &mut self.piano_roll_buttons,
        };
        if let Some(existing) = list.iter_mut().find(|b| b.action == action) {
            existing.icon_id = icon_id;
        } else {
            list.push(BoundToolButton {
                icon_id,
                action,
                custom_command: None,
            });
        }
        (scope, icon_label, action.label())
    }

    pub fn bind_custom_command(&mut self) -> Option<(ToolbarScope, String, String)> {
        let command = self
            .custom_command
            .trim()
            .trim_start_matches(':')
            .trim()
            .to_string();
        if command.is_empty() || command.len() > 256 || command.contains(['\n', '\r', '\0']) {
            return None;
        }
        let icon = self.icons.get(self.selected_icon)?;
        let (icon_id, icon_label) = (icon.id.clone(), icon.label.clone());
        let scope = self.picker_scope;
        let list = match scope {
            ToolbarScope::Transport => &mut self.transport_buttons,
            ToolbarScope::PianoRoll => &mut self.piano_roll_buttons,
        };
        if let Some(existing) = list
            .iter_mut()
            .find(|button| button.custom_command.as_deref() == Some(&command))
        {
            existing.icon_id = icon_id;
        } else {
            list.push(BoundToolButton {
                icon_id,
                action: self.selected_action,
                custom_command: Some(command.clone()),
            });
        }
        Some((scope, icon_label, command))
    }

    pub fn remove_at(&mut self, scope: ToolbarScope, index: usize) {
        let list = match scope {
            ToolbarScope::Transport => &mut self.transport_buttons,
            ToolbarScope::PianoRoll => &mut self.piano_roll_buttons,
        };
        if index < list.len() {
            list.remove(index);
        }
    }

    /// Escanea `assets/icons/toolbar/` y `~/.local/share/hay-daw/icons/` para cargar `.svg` de usuario.
    pub fn reload_custom_icons(&mut self) -> usize {
        self.icons = builtin_icons();
        let mut dirs = vec![PathBuf::from("assets/icons/toolbar")];
        if let Some(home) = dirs::home_dir() {
            let user_dir = home.join(".local/share/hay-daw/icons");
            let _ = fs::create_dir_all(&user_dir);
            dirs.push(user_dir);
        }
        let mut discovered = Vec::new();
        for dir in dirs {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_none_or(|e| !e.eq_ignore_ascii_case("svg"))
                {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if self.icons.iter().any(|i| i.id == stem)
                    || discovered.iter().any(|(id, _): &(String, _)| id == stem)
                {
                    continue;
                }
                if let Ok(raw) = fs::read_to_string(&path) {
                    discovered.push((stem.to_string(), raw));
                }
            }
        }
        discovered.sort_by(|a, b| a.0.cmp(&b.0));
        let added = discovered.len();
        for (stem, raw) in discovered {
            let normalized = normalize_symbolic_svg(&raw);
            let leaked: &'static [u8] = Box::leak(normalized.into_bytes().into_boxed_slice());
            self.icons.push(ToolbarIcon {
                label: stem.replace(['-', '_'], " "),
                id: stem,
                svg_bytes: leaked,
            });
        }
        added
    }
}

fn builtin_icons() -> Vec<ToolbarIcon> {
    vec![
        ToolbarIcon {
            id: "cat-sleeping".into(),
            label: "Gato Durmiendo <3".into(),
            svg_bytes: ICON_CAT_SLEEPING,
        },
        ToolbarIcon {
            id: "metronome".into(),
            label: "Metrónomo".into(),
            svg_bytes: ICON_METRONOME,
        },
        ToolbarIcon {
            id: "scissors".into(),
            label: "Tijeras / Split".into(),
            svg_bytes: ICON_SCISSORS,
        },
        ToolbarIcon {
            id: "magnet".into(),
            label: "Imán / Snap".into(),
            svg_bytes: ICON_MAGNET,
        },
        ToolbarIcon {
            id: "wave".into(),
            label: "Onda / Audio".into(),
            svg_bytes: ICON_WAVE,
        },
        ToolbarIcon {
            id: "piano".into(),
            label: "Teclas MIDI".into(),
            svg_bytes: ICON_PIANO,
        },
        ToolbarIcon {
            id: "bolt".into(),
            label: "Rayo / Acción".into(),
            svg_bytes: ICON_BOLT,
        },
        ToolbarIcon {
            id: "heart".into(),
            label: "Favorito".into(),
            svg_bytes: ICON_HEART,
        },
    ]
}

/// Convierte atributos simbólicos GTK (`url("#gpa:foreground") rgb(0,0,0)` o `currentColor`)
/// a `#000000` para que GPUI los coloree con `.text_color(...)`.
fn normalize_symbolic_svg(raw: &str) -> String {
    raw.replace("url(&quot;#gpa:foreground&quot;) rgb(0,0,0)", "#000000")
        .replace("url(\"#gpa:foreground\") rgb(0,0,0)", "#000000")
        .replace("currentColor", "#000000")
}

pub(in crate::ui) fn render_svg_icon(bytes: &'static [u8], size_px: f32, color: u32) -> Svg {
    svg().data(bytes).size(px(size_px)).text_color(rgb(color))
}

pub(in crate::ui) fn toolbar_icon_button(
    ds: &DesignSystem,
    bytes: &'static [u8],
    tag: &str,
    color: u32,
    compact: bool,
) -> Div {
    let h = if compact {
        26.0
    } else {
        ds.transport_btn_size + 4.0
    };
    div()
        .h(px(h))
        .px(px(8.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(5.0))
        .bg(rgb(ds.panel_dark))
        .border_1()
        .border_color(rgb(ds.border))
        .rounded(px(ds.radius_md))
        .cursor_pointer()
        .child(render_svg_icon(
            bytes,
            if compact { 16.0 } else { 19.0 },
            color,
        ))
        .child(
            div()
                .text_size(px(if compact { 10.5 } else { 11.0 }))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(ds.text))
                .child(tag.to_owned()),
        )
}
