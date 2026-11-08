//! Frontera GPUI. Estado visual aquí; datos musicales en elevate::musical.
mod app;
mod audio_actions;
mod design;
mod input;
mod pattern_editor;
mod piano_keys;
mod render;
mod timeline;
mod views;

use crate::audio::{DrumPlayer, Player, piano::PianoPlayer};
use design::DesignSystem;
use gpui::{FocusHandle, ScrollHandle};
use pattern_editor::PatternEditor;
use std::collections::HashSet;
use timeline::Timeline;

pub(crate) use app::run;

#[derive(Debug, Clone, Copy, PartialEq)]
enum VimMode {
    Normal,
    Insert,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CenterView {
    Arrangement,
    Instrument,
    Session,
    Mixer,
}

// Privado al árbol UI: las vistas no exponen este estado a audio o al núcleo musical.
struct DawApp {
    player: Option<Player>,
    pattern_player: Option<DrumPlayer>,
    piano: Option<PianoPlayer>,
    held_notes: HashSet<u8>,
    mouse_note: Option<u8>,
    _soundfont_task: Option<gpui::Task<Result<(), String>>>,
    import_task: Option<gpui::Task<()>>,
    gain: f32,
    bypass: bool,
    ds: DesignSystem,
    focus_handle: FocusHandle,
    timeline: Timeline,
    pattern_editor: PatternEditor,
    track_scroll: ScrollHandle,
    mode: VimMode,
    center_view: CenterView,
    cursor_row: usize,
    cursor_col: usize,
    command_buffer: String,
    last_key: String,
    status_message: String,
}
