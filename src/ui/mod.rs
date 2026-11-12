//! Frontera GPUI. Estado visual aquí; datos musicales en elevate::musical.
mod action_form;
mod app;
mod audio_actions;
mod audio_preferences;
mod file_playback;
mod clip_actions;
mod clip_menu;
mod command_suggestions;
mod components;
mod cursor_fx;
mod design;
mod detached;
mod explorer;
mod explorer_host;
mod export;
mod export_preferences;
mod file_import;
mod general_note;
mod fx;
mod icons;
mod input;
mod kit;
mod keyboard_preferences;
mod lua_tools;
mod workspace_focus;
mod navigation;
mod palette;
mod pattern_editor;
mod piano_roll;
mod profile;
mod render;
mod sampler;
mod vst;
mod scripting;
mod spectrum;
mod settings;
mod time_grid;
mod timing;
mod timeline;
mod toolbar;
mod track_actions;
mod track_batch;
mod track_commit;
mod track_scripts;
mod track_drag;
mod track_layout;
mod track_menu;
mod track_presentation;
mod transport;
mod utility;
mod views;

use crate::audio::{DrumPlayer, MidiHardwareIn, Player};
use crate::instruments::Piano;
use components::splitter::SplitDrag;
use cursor_fx::CursorFx;
use design::DesignSystem;
use elevate::musical::{
    clip::MidiCapture,
    controller::ComputerKeyboard,
    midi::MidiServer,
    midi_map::MidiMapper,
    track::{Track, TrackId},
};
use gpui::{FocusHandle, Pixels, ScrollHandle, WindowHandle};
use palette::Palette;
use pattern_editor::PatternEditor;
use piano_roll::PianoRoll;
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
    Session,
    Mixer,
}

// Privado al árbol UI: las vistas no exponen este estado a audio o al núcleo musical.
struct DawApp {
    player: Option<Player>,
    file_playback: Option<file_playback::PendingPlayback>,
    pattern_player: Option<DrumPlayer>,
    instrument: Piano,
    sampler: crate::instruments::sampler::Sampler,
    sampler_ui: sampler::SamplerUi,
    vst: crate::instruments::vst::VstInstrument,
    vst_effects: vst::effects::VstEffects,
    vst_task: Option<gpui::Task<()>>,
    controller: ComputerKeyboard,
    midi_server: MidiServer,
    midi_mapper: MidiMapper,
    midi_hw: MidiHardwareIn,
    midi_capture: MidiCapture,
    session_epoch: Instant,
    instrument_window: Option<WindowHandle<gpui::base::Root>>,
    window_request: Option<bool>,
    palette: Palette,
    action_form: Option<action_form::ActionForm>,
    general_note: Option<general_note::GeneralNote>,
    settings: SettingsUi,
    toolbar_icons: icons::ToolbarBindings,
    cursor_fx: CursorFx,
    split_drag: Option<SplitDrag>,
    workspace_split_ratio: f32,
    explorer: explorer_host::ExplorerDock,
    fx: fx::FxDock,
    piano_ui: views::instrument_params::PianoUi,
    import_task: Option<gpui::Task<()>>,
    script_task: Option<gpui::Task<()>>,
    lua_tools: lua_tools::LuaTools,
    export_job: Option<crate::audio::export::ExportJob>,
    utility: elevate::devices::utility_slot::UtilitySlot,
    utility_open: bool,
    ds: DesignSystem,
    focus_handle: FocusHandle,
    timeline: Timeline,
    pattern_editor: PatternEditor,
    piano_roll: PianoRoll,
    clip_menu: Option<clip_menu::ClipMenu>,
    /// Menú de color abierto con clic derecho sobre la cabecera de una pista.
    track_color_menu: Option<(gpui::Point<gpui::Pixels>, usize)>,
    piano_roll_focus: FocusHandle,
    piano_roll_window: Option<WindowHandle<gpui::base::Root>>,
    tracks: Vec<Track>,
    track_revision: u64,
    selected_tracks: Vec<TrackId>,
    track_anchor: Option<TrackId>,
    track_menu: Option<track_menu::TrackMenu>,
    track_presentation: std::collections::HashMap<TrackId, track_layout::TrackPresentation>,
    track_delete_pending: bool,
    track_levels: Vec<i32>,
    track_pans: Vec<i32>,
    track_control_drag: Option<(usize, bool, Pixels, i32, bool)>,
    track_reorder_drag: Option<track_drag::TrackReorderDrag>,
    track_scroll: ScrollHandle,
    mode: VimMode,
    vim_enabled: bool,
    workspace_focus: workspace_focus::WorkspaceFocus,
    center_view: CenterView,
    cursor_row: usize,
    cursor_col: usize,
    command_buffer: String,
    cmd_selected: Option<usize>,
    last_key: String,
    status_message: String,
}
