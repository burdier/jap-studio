//! Estado visual del mock de piano roll. El contenido sigue siendo un `MidiClip` en ticks.
mod actions;
mod demo;
pub(in crate::ui) mod geometry;
mod window;

pub(super) use window::DetachedPianoRoll;

use elevate::musical::{
    model::NoteId,
    time::{PPQ, Tick},
};
use geometry::Grid;
use gpui::{Pixels, Point};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
pub(super) struct InsertMenu {
    pub position: Point<Pixels>,
    pub track: usize,
    pub beat: f64,
}

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
    ToggleWindow,
    Close,
    Preview(&'static str),
}

pub(super) struct PianoRoll {
    pub open: bool,
    pub menu: Option<InsertMenu>,
    pub tool: RollTool,
    pub cursor: Tick,
    pub selected: Option<NoteId>,
    pub ghost_notes: bool,
    pub follow: bool,
    pub velocities: bool,
    pub highlight_scale: bool,
    pub grid: Rc<Cell<Grid>>,
    pub hint: String,
}

impl Default for PianoRoll {
    fn default() -> Self {
        Self {
            open: false,
            menu: None,
            tool: RollTool::Select,
            cursor: Tick(13 * PPQ),
            selected: None,
            ghost_notes: true,
            follow: true,
            velocities: true,
            highlight_scale: true,
            grid: Rc::default(),
            hint: "h/l: tiempo · j/k: nota · v/b/e/p/m: herramienta · Esc: volver".into(),
        }
    }
}
