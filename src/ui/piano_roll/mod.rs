//! Estado visual del mock de piano roll. El contenido sigue siendo un `MidiClip` en ticks.
mod actions;
mod demo;
mod edit;
pub(in crate::ui) mod geometry;
mod navigation;
#[cfg(test)]
mod navigation_tests;
mod window;

use elevate::musical::{
    model::{NoteId, Pattern},
    time::{PPQ, Tick},
};
use geometry::Grid;
use gpui::{Pixels, Point};
use navigation::RollViewport;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RollTool {
    Select,
    Draw,
    Erase,
    Paint,
    Mute,
    Box,
    Length,
}

pub(super) enum RollDrag {
    Box {
        origin: Point<Pixels>,
        current: Point<Pixels>,
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
    Tool(RollTool),
    GhostNotes,
    Follow,
    Velocities,
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
    HumanizeAmount(u8),
    Copy,
    Cut,
    Paste,
    Preview(&'static str),
}

pub(super) struct PianoRoll {
    pub open: bool,
    pub tool: RollTool,
    pub cursor: Tick,
    pub selected: Option<NoteId>,
    pub selected_notes: Vec<NoteId>,
    pub hovered_note: Option<NoteId>,
    pub hovered_resize_edge: bool,
    pub drag: Option<RollDrag>,
    pub ghost_notes: bool,
    pub follow: bool,
    pub velocities: bool,
    pub highlight_scale: bool,
    pub humanize_amount: u8,
    pub grid: Rc<Cell<Grid>>,
    pub viewport: RollViewport,
    pub hint: String,
    pub copied_note: Option<elevate::musical::model::NoteEvent>,
}

impl Default for PianoRoll {
    fn default() -> Self {
        Self {
            open: false,
            tool: RollTool::Select,
            cursor: Tick(13 * PPQ),
            selected: None,
            selected_notes: Vec::new(),
            hovered_note: None,
            hovered_resize_edge: false,
            drag: None,
            ghost_notes: true,
            follow: true,
            velocities: true,
            highlight_scale: true,
            humanize_amount: 50,
            grid: Rc::default(),
            viewport: RollViewport::default(),
            hint: "Super + 2 dedos: altura · Alt + 2 dedos: ancho · 0: ajustar vista".into(),
            copied_note: None,
        }
    }
}
