//! Frontera GPUI. Estado visual aquí; datos musicales en elevate::musical.
mod app;
mod audio_actions;
mod components;
mod cursor_fx;
mod design;
mod detached;
mod input;
mod palette;
mod pattern_editor;
mod render;
mod settings;
mod timeline;
mod views;

use crate::audio::{DrumPlayer, Player};
use crate::instruments::Piano;
use cursor_fx::CursorFx;
use design::DesignSystem;
use detached::DetachedPanel;
use elevate::musical::{
    clip::MidiCapture,
    controller::ComputerKeyboard,
    midi::MidiServer,
    track::{TRACK_COUNT, Track},
};
use gpui::{FocusHandle, ScrollHandle, WindowHandle};
use palette::Palette;
use pattern_editor::PatternEditor;
use settings::SettingsUi;
use std::time::Instant;
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
    instrument: Piano,
    controller: ComputerKeyboard,
    midi_server: MidiServer,
    midi_capture: MidiCapture,
    session_epoch: Instant,
    instrument_window: Option<WindowHandle<DetachedPanel>>,
    window_request: Option<bool>,
    palette: Palette,
    settings: SettingsUi,
    cursor_fx: CursorFx,
    _soundfont_task: Option<gpui::Task<Result<(), String>>>,
    import_task: Option<gpui::Task<()>>,
    gain: f32,
    bypass: bool,
    ds: DesignSystem,
    focus_handle: FocusHandle,
    timeline: Timeline,
    pattern_editor: PatternEditor,
    tracks: [Track; TRACK_COUNT],
    track_levels: [i32; TRACK_COUNT],
    track_pans: [i32; TRACK_COUNT],
    track_scroll: ScrollHandle,
    mode: VimMode,
    center_view: CenterView,
    cursor_row: usize,
    cursor_col: usize,
    command_buffer: String,
    last_key: String,
    status_message: String,
}
