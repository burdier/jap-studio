//! Estado visual del piano roll. El contenido sigue siendo un `MidiClip` en ticks.
mod actions;
mod choices;
pub(in crate::ui) mod chord;
pub(in crate::ui) mod chord_sequence;
mod clipboard;
mod commands;
mod control_actions;
mod drag;
mod edit;
pub(in crate::ui) mod geometry;
mod grid_actions;
mod keyboard;
pub(in crate::ui) mod lane;
pub(in crate::ui) mod motion;
pub(in crate::ui) mod navigation;
#[cfg(test)]
mod navigation_tests;
mod paint;
pub(super) mod pitch;
mod pointer;
mod properties;
mod push;
mod selection;
mod window;

pub(in crate::ui) use crate::ui::components::curve_overlay::shapes::{ContourShape, LanePreset};
pub(in crate::ui) use lane::{RollLane, RollLaneMenu};

/// Alcance de un patrón de carril.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::ui) enum LaneScope {
    /// Una sola forma a lo largo de todas las notas.
    #[default]
    All,
    /// La forma se reinicia y repite en cada compás.
    PerBar,
    /// Solo el compás donde se hizo clic derecho.
    ThisBar,
}

impl LaneScope {
    pub const ALL: [Self; 3] = [Self::All, Self::PerBar, Self::ThisBar];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todo",
            Self::PerBar => "Cada compás",
            Self::ThisBar => "Este compás",
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct LaneSpanDrag {
    pub origin: Point<Pixels>,
    pub current: Point<Pixels>,
    pub tension: f32,
}

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
    Pitch,
}

pub(super) enum RollDrag {
    Box {
        origin: Point<Pixels>,
        current: Point<Pixels>,
        erase: bool,
        additive: Vec<NoteId>,
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
    CycleLaneContour,
    SetLaneContour(ContourShape),
    SetLaneScope(LaneScope),
    ApplyLanePreset(LanePreset),
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
    ChordDegree(u8),
    ChordColor(u8),
    ChordVoicing(u8),
    ChordToggleBass,
    ChordToggleMidiMode,
    ChordInversion(i8),
    ChordStrum(i16),
    ChordInsertAtCursor,
    ChordApplySelection,
    ChordSequenceUnit(chord_sequence::ChordUnit),
    ChordSequenceSlot(usize),
    ChordSequenceReplace(usize),
    ChordSequenceClear(usize),
    ChordSequenceInvert(usize, i8),
    ChordSequenceResize(i8),
    ChordSequenceInsert,
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
    pub pitch_drag: Option<pitch::PitchDrag>,
    pub motion: Rc<RefCell<motion::NoteMotion>>,
    push: Option<push::PushGesture>,
    push_error: Option<String>,
    pub snap_guide: crate::ui::components::snap_guide::SnapGuide,
    pub control_drag: bool,
    pub selected_control: Option<u64>,
    pub paint_stroke: Option<std::collections::HashSet<(i64, u8)>>,
    pub ghost_notes: bool,
    pub follow: bool,
    pub velocities: bool,
    pub active_lane: RollLane,
    pub lane_contour: ContourShape,
    pub lane_span: Option<LaneSpanDrag>,
    pub lane_menu: Option<RollLaneMenu>,
    /// Posición del menú de patrones (clic derecho en el carril de propiedades).
    pub lane_preset_menu: Option<Point<Pixels>>,
    pub lane_scope: LaneScope,
    /// Pulso (negras) bajo el clic derecho, para `LaneScope::ThisBar`.
    pub lane_preset_beat: f64,
    pub choices: Option<choices::RollChoices>,
    pub highlight_scale: bool,
    pub show_scale_keyboard: bool,
    pub show_note_names: bool,
    pub inspector_tab: u8,
    pub chord_pad: chord::ChordPadState,
    pub chord_sequence: chord_sequence::ChordSequenceState,
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
            pitch_drag: None,
            motion: Rc::default(),
            push: None,
            push_error: None,
            snap_guide: Default::default(),
            paint_stroke: None,
            control_drag: false,
            selected_control: None,
            ghost_notes: true,
            follow: true,
            velocities: true,
            active_lane: RollLane::Velocity,
            lane_contour: ContourShape::Smooth,
            lane_span: None,
            lane_menu: None,
            lane_preset_menu: None,
            lane_scope: LaneScope::All,
            lane_preset_beat: 0.0,
            choices: None,
            highlight_scale: true,
            show_scale_keyboard: true,
            show_note_names: false,
            inspector_tab: 0,
            chord_pad: Default::default(),
            chord_sequence: Default::default(),
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
