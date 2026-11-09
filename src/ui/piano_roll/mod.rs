//! Estado visual del mock de piano roll. El contenido sigue siendo un `MidiClip` en ticks.
mod actions;
mod demo;
pub(in crate::ui) mod geometry;
mod navigation;
#[cfg(test)]
mod navigation_tests;
mod window;

use elevate::musical::{
    model::NoteId,
    time::{PPQ, Tick},
};
use geometry::Grid;
use navigation::RollViewport;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RollTool {
    Select,
    Draw,
    Erase,
    Paint,
    Mute,
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
    Preview(&'static str),
}

pub(super) struct PianoRoll {
    pub open: bool,
    pub tool: RollTool,
    pub cursor: Tick,
    pub selected: Option<NoteId>,
    pub ghost_notes: bool,
    pub follow: bool,
    pub velocities: bool,
    pub highlight_scale: bool,
    pub grid: Rc<Cell<Grid>>,
    pub viewport: RollViewport,
    pub hint: String,
}

impl Default for PianoRoll {
    fn default() -> Self {
        Self {
            open: false,
            tool: RollTool::Select,
            cursor: Tick(13 * PPQ),
            selected: None,
            ghost_notes: true,
            follow: true,
            velocities: true,
            highlight_scale: true,
            grid: Rc::default(),
            viewport: RollViewport::default(),
            hint: "Super + 2 dedos: altura · Alt + 2 dedos: ancho · 0: ajustar vista".into(),
        }
    }
}
