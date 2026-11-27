//! Presets de atajos de teclado (Jap Studio, Ableton Live, FL Studio) y tabla dinámica de ayuda.
use super::{
    i18n::{Language, tr},
    profile,
};

pub(super) const PROFILE_KEY: &str = "SHORTCUT_PRESET";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ui) enum ShortcutPreset {
    #[default]
    JapStudio,
    AbletonLive,
    FlStudio,
}

impl ShortcutPreset {
    pub const ALL: [Self; 3] = [Self::JapStudio, Self::AbletonLive, Self::FlStudio];

    pub fn token(self) -> &'static str {
        match self {
            Self::JapStudio => "jap",
            Self::AbletonLive => "ableton",
            Self::FlStudio => "fl",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::JapStudio => "Jap Studio",
            Self::AbletonLive => "Ableton Live",
            Self::FlStudio => "FL Studio",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "jap" | "jap-studio" | "default" | "hay" | "elevate" => Some(Self::JapStudio),
            "ableton" | "ableton-live" | "live" => Some(Self::AbletonLive),
            "fl" | "fl-studio" | "flstudio" | "fruity" => Some(Self::FlStudio),
            _ => None,
        }
    }

    pub fn load() -> Self {
        let Ok(content) = profile::load() else {
            return Self::default();
        };
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("SHORTCUT_PRESET=")
                && let Some(preset) = Self::parse(rest)
            {
                return preset;
            }
        }
        Self::default()
    }

    pub fn save(self) -> std::io::Result<()> {
        profile::save_values(&[(PROFILE_KEY, self.token().to_string())])
    }

    /// Resuelve un atajo global/de workspace respetando el preset activo y evitando conflictos con
    /// el teclado MIDI QWERTY cuando está activo.
    pub fn resolve(
        self,
        input: KeyInput<'_>,
        midi_qwerty_enabled: bool,
        arrangement_focused: bool,
    ) -> Option<ShortcutAction> {
        let k = input.key.to_ascii_lowercase();
        let key = k.as_str();
        let ctrl = input.ctrl;
        let shift = input.shift;
        let alt = input.alt;

        // Atajos universales con Ctrl/Cmd (consistentes en todos los presets).
        if ctrl && !alt {
            match (key, shift) {
                ("s", false) => return Some(ShortcutAction::ProjectSave),
                ("s", true) => return Some(ShortcutAction::ProjectSaveAs),
                ("o", false) => return Some(ShortcutAction::ProjectOpen),
                ("z", false) => return Some(ShortcutAction::Undo),
                ("z", true) | ("y", false) => return Some(ShortcutAction::Redo),
                ("h", false) => return Some(ShortcutAction::ToggleHistory),
                ("t", false) => return Some(ShortcutAction::NewAudioTrack),
                ("t", true) => return Some(ShortcutAction::NewMidiTrack),
                ("g", false) => return Some(ShortcutAction::GroupTracks),
                ("l", false) => return Some(ShortcutAction::ToggleLoop),
                (",", false) => return Some(ShortcutAction::OpenSettings),
                ("/", false) => return Some(ShortcutAction::ShowShortcuts),
                _ => {}
            }
        }

        // Atajos con Ctrl+Alt.
        if ctrl && alt && !shift {
            match key {
                "t" => return Some(ShortcutAction::NewReturnBus),
                "b" if self == Self::AbletonLive => return Some(ShortcutAction::ToggleExplorer),
                "l" if self == Self::AbletonLive => return Some(ShortcutAction::ToggleTrackFx),
                "c" if self == Self::FlStudio => return Some(ShortcutAction::Consolidate),
                "b" if self == Self::FlStudio => return Some(ShortcutAction::BounceMidi),
                "z" if self == Self::FlStudio => return Some(ShortcutAction::Redo),
                _ => {}
            }
        }

        // Atajos con Alt (pistas y grabación rápida, sin colisionar con QWERTY MIDI).
        if alt && !ctrl && !shift {
            match key {
                "m" => return Some(ShortcutAction::ToggleTrackMute),
                "s" => return Some(ShortcutAction::ToggleTrackSolo),
                "f" => return Some(ShortcutAction::ToggleTrackFreeze),
                "r" => return Some(ShortcutAction::ToggleAudioRecord),
                "i" => return Some(ShortcutAction::ToggleInputMonitor),
                "t" => return Some(ShortcutAction::TapTempo),
                "f8" if self == Self::FlStudio => return Some(ShortcutAction::ToggleExplorer),
                "f9" if self == Self::FlStudio => return Some(ShortcutAction::ToggleTrackFx),
                _ => {}
            }
        }

        // Diferencias específicas por preset (Ctrl / teclas de función / teclas rápidas sin MIDI).
        match self {
            Self::JapStudio => {
                if ctrl && !alt {
                    match (key, shift) {
                        ("p", false) => return Some(ShortcutAction::OpenCommandPalette),
                        ("p", true) => return Some(ShortcutAction::OpenAddPalette),
                        ("e", false) => return Some(ShortcutAction::ToggleExplorer),
                        ("e", true) => return Some(ShortcutAction::ExportMidi),
                        ("f", true) => return Some(ShortcutAction::ToggleTrackFx),
                        ("m", true) => return Some(ShortcutAction::TogglePianoRoll),
                        ("r", true) => return Some(ShortcutAction::ExportAudio),
                        ("k", false) => return Some(ShortcutAction::ToggleMetronome),
                        ("a", true) => return Some(ShortcutAction::ToggleAutomation),
                        ("x", true) if arrangement_focused => {
                            return Some(ShortcutAction::SplitClipAtCursor);
                        }
                        ("j", false) if arrangement_focused => {
                            return Some(ShortcutAction::Consolidate);
                        }
                        ("b", false) if arrangement_focused => {
                            return Some(ShortcutAction::BounceMidi);
                        }
                        ("d", false) => return Some(ShortcutAction::Duplicate),
                        _ => {}
                    }
                }
                if !ctrl && !alt {
                    match (key, shift) {
                        ("f1", false) => return Some(ShortcutAction::ViewArrangement),
                        ("f2", false) => return Some(ShortcutAction::ViewDrums),
                        ("f3", false) => return Some(ShortcutAction::ViewMixer),
                        ("f4", false) => return Some(ShortcutAction::TogglePianoRoll),
                        ("?", _) => return Some(ShortcutAction::ShowShortcuts),
                        ("r", false) if !midi_qwerty_enabled => {
                            return Some(ShortcutAction::ToggleMidiRecord);
                        }
                        _ => {}
                    }
                }
            }
            Self::AbletonLive => {
                if ctrl && !alt {
                    match (key, shift) {
                        ("p", false) => return Some(ShortcutAction::OpenCommandPalette),
                        ("f", false) => return Some(ShortcutAction::OpenAddPalette),
                        ("f", true) => return Some(ShortcutAction::ToggleTrackFx),
                        ("e", false) if arrangement_focused => {
                            return Some(ShortcutAction::SplitClipAtCursor);
                        }
                        ("e", false) => return Some(ShortcutAction::ToggleExplorer),
                        ("e", true) => return Some(ShortcutAction::ExportMidi),
                        ("r", true) => return Some(ShortcutAction::ExportAudio),
                        ("k", false) | ("k", true) => return Some(ShortcutAction::ToggleMetronome),
                        ("a", true) => return Some(ShortcutAction::ToggleAutomation),
                        ("j", false) if arrangement_focused => {
                            return Some(ShortcutAction::Consolidate);
                        }
                        ("b", false) if arrangement_focused => {
                            return Some(ShortcutAction::BounceMidi);
                        }
                        ("d", false) => return Some(ShortcutAction::Duplicate),
                        _ => {}
                    }
                }
                if !ctrl && !alt {
                    match (key, shift) {
                        ("tab", false) => return Some(ShortcutAction::ToggleArrangementMixer),
                        ("tab", true) => return Some(ShortcutAction::TogglePianoRoll),
                        ("f1", false) => return Some(ShortcutAction::ViewArrangement),
                        ("f2", false) => return Some(ShortcutAction::ViewDrums),
                        ("f3", false) => return Some(ShortcutAction::ViewMixer),
                        ("f4", false) => return Some(ShortcutAction::TogglePianoRoll),
                        ("f9", false) => return Some(ShortcutAction::ToggleMidiRecord),
                        ("a", false) if !midi_qwerty_enabled => {
                            return Some(ShortcutAction::ToggleAutomation);
                        }
                        ("r", false) if !midi_qwerty_enabled => {
                            return Some(ShortcutAction::ToggleMidiRecord);
                        }
                        ("s", false) if !midi_qwerty_enabled => {
                            return Some(ShortcutAction::ToggleTrackSolo);
                        }
                        ("0", false) if !midi_qwerty_enabled && arrangement_focused => {
                            return Some(ShortcutAction::ToggleTrackMute);
                        }
                        ("?", _) => return Some(ShortcutAction::ShowShortcuts),
                        _ => {}
                    }
                }
            }
            Self::FlStudio => {
                if ctrl && !alt {
                    match (key, shift) {
                        ("p", false) => return Some(ShortcutAction::OpenCommandPalette),
                        ("r", false) => return Some(ShortcutAction::ExportAudio),
                        ("m", true) => return Some(ShortcutAction::ExportMidi),
                        ("e", false) => return Some(ShortcutAction::ToggleExplorer),
                        ("f", true) => return Some(ShortcutAction::ToggleTrackFx),
                        ("m", false) | ("k", false) => {
                            return Some(ShortcutAction::ToggleMetronome);
                        }
                        ("a", true) => return Some(ShortcutAction::ToggleAutomation),
                        ("b", false) | ("d", false) => return Some(ShortcutAction::Duplicate),
                        ("x", true) if arrangement_focused => {
                            return Some(ShortcutAction::SplitClipAtCursor);
                        }
                        ("j", false) if arrangement_focused => {
                            return Some(ShortcutAction::Consolidate);
                        }
                        _ => {}
                    }
                }
                if !ctrl && !alt {
                    match (key, shift) {
                        ("f5", false) => return Some(ShortcutAction::ViewArrangement),
                        ("f6", false) => return Some(ShortcutAction::ViewDrums),
                        ("f7", false) => return Some(ShortcutAction::TogglePianoRoll),
                        ("f8", false) => return Some(ShortcutAction::OpenAddPalette),
                        ("f9", false) => return Some(ShortcutAction::ViewMixer),
                        ("f10", false) => return Some(ShortcutAction::OpenSettings),
                        ("g", true) => return Some(ShortcutAction::GroupTracks),
                        ("r", false) if !midi_qwerty_enabled => {
                            return Some(ShortcutAction::ToggleMidiRecord);
                        }
                        ("?", _) => return Some(ShortcutAction::ShowShortcuts),
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Etiqueta corta para menús y ayuda contextual según el preset activo.
    pub fn hint(self, action: ShortcutAction) -> &'static str {
        use ShortcutAction::*;
        match (self, action) {
            (_, ProjectSave) => "Ctrl+S",
            (_, ProjectSaveAs) => "Ctrl+Shift+S",
            (_, ProjectOpen) => "Ctrl+O",
            (Self::FlStudio, Undo) => "Ctrl+Z",
            (_, Undo) => "Ctrl+Z",
            (Self::FlStudio, Redo) => "Ctrl+Alt+Z / Ctrl+Y",
            (_, Redo) => "Ctrl+Shift+Z / Ctrl+Y",
            (_, ToggleHistory) => "Ctrl+H",
            (_, OpenCommandPalette) => "Ctrl+P / :",
            (Self::JapStudio, OpenAddPalette) => "Ctrl+Shift+P / Ins",
            (Self::AbletonLive, OpenAddPalette) => "Ctrl+F / Ins",
            (Self::FlStudio, OpenAddPalette) => "F8 / Ins",
            (Self::FlStudio, OpenSettings) => "F10 / Ctrl+,",
            (_, OpenSettings) => "Ctrl+,",
            (_, ShowShortcuts) => "Ctrl+/ / ?",
            (Self::JapStudio, ViewArrangement) => "F1",
            (Self::AbletonLive, ViewArrangement) => "Tab / F1",
            (Self::FlStudio, ViewArrangement) => "F5",
            (Self::JapStudio, ViewDrums) => "F2",
            (Self::AbletonLive, ViewDrums) => "F2",
            (Self::FlStudio, ViewDrums) => "F6",
            (Self::JapStudio, ViewMixer) => "F3",
            (Self::AbletonLive, ViewMixer) => "Tab / F3",
            (Self::FlStudio, ViewMixer) => "F9",
            (Self::JapStudio, TogglePianoRoll) => "F4 / Ctrl+Shift+M",
            (Self::AbletonLive, TogglePianoRoll) => "Shift+Tab / F4",
            (Self::FlStudio, TogglePianoRoll) => "F7",
            (Self::AbletonLive, ToggleExplorer) => "Ctrl+Alt+B",
            (Self::FlStudio, ToggleExplorer) => "Alt+F8 / Ctrl+E",
            (Self::JapStudio, ToggleExplorer) => "Ctrl+E",
            (Self::AbletonLive, ToggleTrackFx) => "Ctrl+Alt+L / Ctrl+Shift+F",
            (Self::FlStudio, ToggleTrackFx) => "Alt+F9 / Ctrl+Shift+F",
            (Self::JapStudio, ToggleTrackFx) => "Ctrl+Shift+F",
            (Self::FlStudio, ExportAudio) => "Ctrl+R",
            (_, ExportAudio) => "Ctrl+Shift+R",
            (Self::FlStudio, ExportMidi) => "Ctrl+Shift+M",
            (_, ExportMidi) => "Ctrl+Shift+E",
            (_, NewAudioTrack) => "Ctrl+T",
            (_, NewMidiTrack) => "Ctrl+Shift+T",
            (_, NewReturnBus) => "Ctrl+Alt+T",
            (Self::FlStudio, GroupTracks) => "Shift+G / Ctrl+G",
            (_, GroupTracks) => "Ctrl+G",
            (Self::AbletonLive, ToggleTrackMute) => "0 / Alt+M",
            (_, ToggleTrackMute) => "Alt+M",
            (Self::AbletonLive, ToggleTrackSolo) => "S / Alt+S",
            (_, ToggleTrackSolo) => "Alt+S",
            (_, ToggleTrackFreeze) => "Alt+F",
            (Self::FlStudio, Duplicate) => "Ctrl+B / Ctrl+D",
            (_, Duplicate) => "Ctrl+D",
            (Self::AbletonLive, SplitClipAtCursor) => "Ctrl+E",
            (_, SplitClipAtCursor) => "Ctrl+Shift+X",
            (Self::FlStudio, Consolidate) => "Ctrl+Alt+C / Ctrl+J",
            (_, Consolidate) => "Ctrl+J",
            (Self::FlStudio, BounceMidi) => "Ctrl+Alt+B",
            (_, BounceMidi) => "Ctrl+B",
            (Self::AbletonLive, ToggleAutomation) => "A / Ctrl+Shift+A",
            (_, ToggleAutomation) => "Ctrl+Shift+A",
            (_, ToggleLoop) => "Ctrl+L",
            (Self::FlStudio, ToggleMetronome) => "Ctrl+M / Ctrl+K",
            (_, ToggleMetronome) => "Ctrl+K",
            (Self::AbletonLive, ToggleMidiRecord) => "F9 / R",
            (_, ToggleMidiRecord) => "R",
            (_, ToggleAudioRecord) => "Alt+R",
            (_, ToggleInputMonitor) => "Alt+I",
            (_, TapTempo) => "Alt+T",
            (_, ToggleArrangementMixer) => "Tab",
        }
    }

    /// Tabla completa de atajos localizada para la vista Configuración → Atajos.
    pub fn help_groups(
        self,
        lang: Language,
    ) -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
        use ShortcutAction::*;
        vec![
            (
                tr(lang, "shortcuts.group.general"),
                vec![
                    (
                        self.hint(OpenCommandPalette),
                        tr(lang, "shortcuts.desc.command_palette"),
                    ),
                    (
                        self.hint(OpenAddPalette),
                        tr(lang, "shortcuts.desc.add_browser"),
                    ),
                    (
                        self.hint(OpenSettings),
                        tr(lang, "shortcuts.desc.open_settings"),
                    ),
                    (
                        self.hint(ShowShortcuts),
                        tr(lang, "shortcuts.desc.shortcut_help"),
                    ),
                    (
                        self.hint(ViewArrangement),
                        tr(lang, "shortcuts.desc.view_arrangement"),
                    ),
                    (self.hint(ViewDrums), tr(lang, "shortcuts.desc.view_drums")),
                    (self.hint(ViewMixer), tr(lang, "shortcuts.desc.view_mixer")),
                    (
                        self.hint(TogglePianoRoll),
                        tr(lang, "shortcuts.desc.toggle_piano_roll"),
                    ),
                    (
                        self.hint(ToggleExplorer),
                        tr(lang, "shortcuts.desc.toggle_explorer"),
                    ),
                    (
                        self.hint(ToggleTrackFx),
                        tr(lang, "shortcuts.desc.toggle_track_fx"),
                    ),
                    ("F11", tr(lang, "shortcuts.desc.maximize_panel")),
                    (self.hint(Undo), tr(lang, "shortcuts.desc.undo_redo")),
                    (
                        self.hint(ToggleHistory),
                        tr(lang, "shortcuts.desc.history_panel"),
                    ),
                    (
                        "Ctrl+S / Ctrl+Shift+S / Ctrl+O",
                        tr(lang, "shortcuts.desc.save_open"),
                    ),
                    (
                        self.hint(ExportAudio),
                        tr(lang, "shortcuts.desc.export_audio"),
                    ),
                    (
                        self.hint(ExportMidi),
                        tr(lang, "shortcuts.desc.export_midi"),
                    ),
                ],
            ),
            (
                tr(lang, "shortcuts.group.tracks"),
                vec![
                    (
                        self.hint(NewAudioTrack),
                        tr(lang, "shortcuts.desc.new_audio_track"),
                    ),
                    (
                        self.hint(NewMidiTrack),
                        tr(lang, "shortcuts.desc.new_midi_track"),
                    ),
                    (
                        self.hint(NewReturnBus),
                        tr(lang, "shortcuts.desc.new_return_bus"),
                    ),
                    (
                        self.hint(GroupTracks),
                        tr(lang, "shortcuts.desc.group_tracks"),
                    ),
                    (
                        self.hint(ToggleTrackMute),
                        tr(lang, "shortcuts.desc.mute_track"),
                    ),
                    (
                        self.hint(ToggleTrackSolo),
                        tr(lang, "shortcuts.desc.solo_track"),
                    ),
                    (
                        self.hint(ToggleTrackFreeze),
                        tr(lang, "shortcuts.desc.freeze_track"),
                    ),
                ],
            ),
            (
                tr(lang, "shortcuts.group.arrangement"),
                vec![
                    (self.hint(Duplicate), tr(lang, "shortcuts.desc.duplicate")),
                    (
                        self.hint(SplitClipAtCursor),
                        tr(lang, "shortcuts.desc.split_clip"),
                    ),
                    (
                        self.hint(Consolidate),
                        tr(lang, "shortcuts.desc.consolidate"),
                    ),
                    (
                        self.hint(BounceMidi),
                        tr(lang, "shortcuts.desc.bounce_midi"),
                    ),
                    (
                        self.hint(ToggleAutomation),
                        tr(lang, "shortcuts.desc.toggle_automation"),
                    ),
                    ("← / →", tr(lang, "shortcuts.desc.move_cursor")),
                    ("↑ / ↓", tr(lang, "shortcuts.desc.select_track")),
                    (
                        "Delete / Backspace",
                        tr(lang, "shortcuts.desc.delete_selected"),
                    ),
                    ("+ / -", tr(lang, "shortcuts.desc.zoom_in_out")),
                    ("Home", tr(lang, "shortcuts.desc.return_home")),
                ],
            ),
            (
                tr(lang, "shortcuts.group.transport"),
                vec![
                    ("Space", tr(lang, "shortcuts.desc.play_stop")),
                    (
                        self.hint(ToggleLoop),
                        tr(lang, "shortcuts.desc.toggle_loop"),
                    ),
                    (
                        self.hint(ToggleMetronome),
                        tr(lang, "shortcuts.desc.toggle_metronome"),
                    ),
                    (
                        self.hint(ToggleMidiRecord),
                        tr(lang, "shortcuts.desc.midi_record"),
                    ),
                    (
                        self.hint(ToggleAudioRecord),
                        tr(lang, "shortcuts.desc.audio_record"),
                    ),
                    (
                        self.hint(ToggleInputMonitor),
                        tr(lang, "shortcuts.desc.input_monitor"),
                    ),
                    (self.hint(TapTempo), tr(lang, "shortcuts.desc.tap_tempo")),
                    ("M", tr(lang, "shortcuts.desc.midi_qwerty")),
                    ("Esc", tr(lang, "shortcuts.desc.escape")),
                ],
            ),
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::ui) struct KeyInput<'a> {
    pub key: &'a str,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum ShortcutAction {
    ProjectSave,
    ProjectSaveAs,
    ProjectOpen,
    Undo,
    Redo,
    ToggleHistory,
    OpenCommandPalette,
    OpenAddPalette,
    OpenSettings,
    ShowShortcuts,
    ViewArrangement,
    ViewDrums,
    ViewMixer,
    ToggleArrangementMixer,
    TogglePianoRoll,
    ToggleExplorer,
    ToggleTrackFx,
    ExportAudio,
    ExportMidi,
    NewAudioTrack,
    NewMidiTrack,
    NewReturnBus,
    GroupTracks,
    ToggleTrackMute,
    ToggleTrackSolo,
    ToggleTrackFreeze,
    Duplicate,
    SplitClipAtCursor,
    Consolidate,
    BounceMidi,
    ToggleAutomation,
    ToggleLoop,
    ToggleMetronome,
    ToggleMidiRecord,
    ToggleAudioRecord,
    ToggleInputMonitor,
    TapTempo,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: &str) -> KeyInput<'_> {
        KeyInput {
            key: k,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    fn ctrl(k: &str) -> KeyInput<'_> {
        KeyInput {
            key: k,
            ctrl: true,
            shift: false,
            alt: false,
        }
    }

    #[test]
    fn ableton_preset_maps_split_tab_and_respects_midi_qwerty() {
        assert_eq!(
            ShortcutPreset::AbletonLive.resolve(ctrl("e"), false, true),
            Some(ShortcutAction::SplitClipAtCursor)
        );
        assert_eq!(
            ShortcutPreset::AbletonLive.resolve(
                KeyInput {
                    key: "b",
                    ctrl: true,
                    shift: false,
                    alt: true
                },
                false,
                true
            ),
            Some(ShortcutAction::ToggleExplorer)
        );
        assert_eq!(
            ShortcutPreset::AbletonLive.resolve(key("tab"), false, true),
            Some(ShortcutAction::ToggleArrangementMixer)
        );
        // 'a' activa automatización solo cuando MIDI QWERTY está desactivado.
        assert_eq!(
            ShortcutPreset::AbletonLive.resolve(key("a"), false, true),
            Some(ShortcutAction::ToggleAutomation)
        );
        assert_eq!(
            ShortcutPreset::AbletonLive.resolve(key("a"), true, true),
            None
        );
    }

    #[test]
    fn fl_studio_preset_maps_function_keys_duplicate_and_export() {
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(key("f5"), false, true),
            Some(ShortcutAction::ViewArrangement)
        );
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(key("f6"), false, true),
            Some(ShortcutAction::ViewDrums)
        );
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(key("f7"), false, true),
            Some(ShortcutAction::TogglePianoRoll)
        );
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(key("f8"), false, true),
            Some(ShortcutAction::OpenAddPalette)
        );
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(key("f9"), false, true),
            Some(ShortcutAction::ViewMixer)
        );
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(ctrl("b"), false, true),
            Some(ShortcutAction::Duplicate)
        );
        assert_eq!(
            ShortcutPreset::FlStudio.resolve(ctrl("r"), false, true),
            Some(ShortcutAction::ExportAudio)
        );
    }

    #[test]
    fn preset_tokens_round_trip() {
        for preset in ShortcutPreset::ALL {
            assert_eq!(ShortcutPreset::parse(preset.token()), Some(preset));
        }
    }
}
