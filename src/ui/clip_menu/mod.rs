//! Menú del Arrangement: contexto común de clips de audio/MIDI e inserción.
mod input;
mod items;
mod opening;

use super::timeline::DragKind;
use gpui::{Pixels, Point, Size};

pub(super) const MENU_WIDTH: f32 = 248.0;
pub(super) const MENU_HEIGHT: f32 = 430.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuPage {
    Main,
    Gain,
    Rename,
    Route,
}

#[derive(Clone, Copy)]
pub(super) enum ClipAction {
    EditMidi,
    Bounce,
    ExportMidi,
    AudioToMidi,
    SendToSampler,
    ReverseChords,
    Duplicate,
    Copy,
    Cut,
    Materialize,
    Consolidate,
    Split,
    RestoreTrim,
    Mute,
    Loop,
    Lock,
    Color(Option<u8>),
    Gain(Option<f32>),
    Move(isize),
    RouteMidi(Option<elevate::musical::track::TrackId>),
    Delete,
    Page(MenuPage),
    Rename,
    InsertMidi,
    InsertCapture,
}

pub(super) struct MenuItem {
    pub label: String,
    pub action: ClipAction,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
}

pub(super) struct ClipMenu {
    pub position: Point<Pixels>,
    pub viewport: Size<Pixels>,
    pub track: usize,
    /// Cursor de edición capturado antes de abrir sobre un clip.
    pub beat: f64,
    pub clip_kind: Option<DragKind>,
    pub page: MenuPage,
    pub selected: usize,
    pub rename: String,
    pub select_name: bool,
}
