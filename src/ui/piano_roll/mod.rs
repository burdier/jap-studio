//! Estado visual del piano roll. El contenido sigue siendo un `MidiClip` en ticks.
mod actions;
mod choices;
mod clipboard;
mod commands;
mod control_actions;
mod demo;
mod drag;
mod edit;
pub(in crate::ui) mod geometry;
mod grid_actions;
mod keyboard;
pub(in crate::ui) mod lane;
pub(in crate::ui) mod motion;
mod navigation;
#[cfg(test)]
mod navigation_tests;
mod paint;
mod pointer;
mod properties;
mod push;
mod selection;
mod window;

pub(in crate::ui) use lane::{RollLane, RollLaneMenu};

use elevate::musical::{
    model::{NoteId, Pattern},
    time::Tick,
};
use geometry::Grid;
use gpui::{Pixels, Point};
use navigation::RollViewport;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RollTool {
    Select,
    Draw,
    Erase,
    Paint,
    Mute,
    Box,
    EraseRegion,
    Length,
}

pub(super) enum RollDrag {
    Box {
        origin: Point<Pixels>,
        current: Point<Pixels>,
        erase: bool,
    },
    Move {
        origin: Point<Pixels>,
        current: Point<Pixels>,
        original: Pattern,
        notes: Vec<NoteId>,
    },
    Length {
        origin: Point<Pixels>,
        current: Point<Pixels>,
        original: Pattern,
        note: NoteId,
    },
}

#[derive(Clone, Copy)]
pub(super) enum RollAction {
    Form(&'static str),
    Tool(RollTool),
    GhostNotes,
    Follow,
    Velocities,
    SetLane(RollLane),
    HighlightScale,
    ResetView,
    ZoomWidth(f64),
    ZoomHeight(f64),
    ToggleWindow,
    Close,
    PreviousClip,
    NextClip,
    Quantize,
    Humanize,
    Gate,
    Swing,
    ClampScale,
    Velocity,
    Random,
    ChooseDrawLength,
    ChooseRoot,
    ChooseScale,
    ToggleScaleKeyboard,
    NoteNames,
    ChooseLane,
    TrackSolo,
    ToggleClipLoop,
    ToggleClipMute,
    ChooseClipColor,
    ChooseClip,
    OpenSettings,
    InspectorTab(u8),
    Subdivide,
    GridFiner,
    GridCoarser,
    HumanizeAmount(u8),
    Copy,
    Cut,
    Paste,
    SelectAll,
    ClearSelection,
    DeleteSelection,
}

pub(super) struct PianoRoll {
    pub open: bool,
    pub opened_clip: Option<elevate::musical::clip::MidiClip>,
    pub tool: RollTool,
    pub cursor: Tick,
    pub selected: Option<NoteId>,
    pub selected_notes: Vec<NoteId>,
    pub hovered_note: Option<NoteId>,
    pub hovered_resize_edge: bool,
    pub drag: Option<RollDrag>,
    pub motion: Rc<RefCell<motion::NoteMotion>>,
    push: Option<push::PushGesture>,
    push_error: Option<String>,
    pub control_drag: bool,
    pub selected_control: Option<u64>,
    pub paint_stroke: Option<std::collections::HashSet<(i64, u8)>>,
    pub ghost_notes: bool,
    pub follow: bool,
    pub velocities: bool,
    pub active_lane: RollLane,
    pub lane_menu: Option<RollLaneMenu>,
    pub choices: Option<choices::RollChoices>,
    pub highlight_scale: bool,
    pub show_scale_keyboard: bool,
    pub show_note_names: bool,
    pub inspector_tab: u8,
    pub draw_length: elevate::musical::grid::NoteGrid,
    pub scale_root: u8,
    pub scale_minor: bool,
    pub humanize_amount: u8,
    pub resolution: elevate::musical::grid::NoteGrid,
    split_gesture: grid_actions::SplitGesture,
    pub grid: Rc<Cell<Grid>>,
    pub viewport: RollViewport,
    pub hint: String,
    pub copied_notes: Vec<(elevate::musical::model::NoteEvent, bool)>,
}

impl Default for PianoRoll {
    fn default() -> Self {
        Self {
            open: false,
            opened_clip: None,
            tool: RollTool::Select,
            cursor: Tick(0),
            selected: None,
            selected_notes: Vec::new(),
            hovered_note: None,
            hovered_resize_edge: false,
            drag: None,
            motion: Rc::default(),
            push: None,
            push_error: None,
            paint_stroke: None,
            control_drag: false,
            selected_control: None,
            ghost_notes: true,
            follow: true,
            velocities: true,
            active_lane: RollLane::Velocity,
            lane_menu: None,
            choices: None,
            highlight_scale: true,
            show_scale_keyboard: true,
            show_note_names: false,
            inspector_tab: 0,
            draw_length: elevate::musical::grid::NoteGrid::new(8).expect("rejilla 1/8"),
            scale_root: 0,
            scale_minor: true,
            humanize_amount: 50,
            resolution: Default::default(),
            split_gesture: Default::default(),
            grid: Rc::default(),
            viewport: RollViewport::default(),
            hint: "Alt+1/2: rejilla · Alt+rueda / Shift+pellizco: subdividir · Ctrl+rueda: zoom"
                .into(),
            copied_notes: Vec::new(),
        }
    }
}

impl PianoRoll {
    pub fn pitch_scale(&self) -> elevate::musical::scale::PitchScale {
        let intervals = if self.scale_minor {
            elevate::musical::scale::PitchScale::C_NATURAL_MINOR.intervals
        } else {
            (1 << 0) | (1 << 2) | (1 << 4) | (1 << 5) | (1 << 7) | (1 << 9) | (1 << 11)
        };
        elevate::musical::scale::PitchScale {
            root: self.scale_root,
            intervals,
        }
    }
}
