//! Frontera GPUI. Estado visual aquí; datos musicales en elevate::musical.
mod action_form;
mod app;
mod audio_actions;
mod audio_preferences;
mod clip_actions;
mod clip_menu;
mod collect;
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
mod file_playback;
mod fx;
mod general_note;
mod icons;
mod input;
mod keyboard_preferences;
mod kit;
mod lua_components;
mod lua_tools;
mod navigation;
mod palette;
mod pattern_editor;
mod piano_instances;
mod piano_roll;
mod profile;
mod project;
mod render;
mod sampler;
mod scripting;
mod settings;
mod spectrum;
mod time_grid;
mod timeline;
mod timing;
mod toolbar;
mod track_actions;
mod track_batch;
mod track_commit;
mod track_drag;
mod track_layout;
mod track_menu;
mod track_presentation;
mod track_scripts;
mod transport;
mod utility;
mod views;
mod vst;
mod weave;
mod workspace_focus;

use crate::audio::{MidiHardwareIn, Player};
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
use gpui::{FocusHandle, ScrollHandle, WindowHandle};
use palette::Palette;
use pattern_editor::PatternEditor;
use piano_roll::PianoRoll;
use settings::SettingsUi;
use std::collections::HashMap;
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
    audio_loop: Option<(f64, f64)>,
    pattern_loop_beats: Option<f64>,
    instrument: Piano,
    piano_instances: HashMap<elevate::musical::model::InstrumentId, Piano>,
    sampler: crate::instruments::sampler::Sampler,
    active_sampler_id: elevate::musical::model::InstrumentId,
    sampler_instances:
        HashMap<elevate::musical::model::InstrumentId, crate::instruments::sampler::Sampler>,
    sampler_ui: sampler::SamplerUi,
    vst: crate::instruments::vst::VstInstrument,
    vst_instances:
        HashMap<elevate::musical::model::InstrumentId, crate::instruments::vst::VstInstrument>,
    vst_reloads: std::collections::VecDeque<(TrackId, std::path::PathBuf, Option<String>)>,
    vst_effects: vst::effects::VstEffects,
    vst_task: Option<gpui::Task<()>>,
    controller: ComputerKeyboard,
    midi_server: MidiServer,
    weave_rule: Result<Option<elevate::musical::weave::WeaveRule>, String>,
    midi_armed_tracks: std::collections::HashSet<TrackId>,
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
    collect_task: Option<gpui::Task<()>>,
    project: project::ProjectUi,
    script_task: Option<gpui::Task<()>>,
    lua_tools: lua_tools::LuaTools,
    lua_components: lua_components::LuaComponents,
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
    bar_menu: Option<views::menu_bar::BarMenu>,
    track_menu: Option<track_menu::TrackMenu>,
    track_presentation: std::collections::HashMap<TrackId, track_layout::TrackPresentation>,
    track_delete_pending: bool,
    track_levels: Vec<i32>,
    track_pans: Vec<i32>,
    track_control_drag: Option<views::tracks::TrackControlDrag>,
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
