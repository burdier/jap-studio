//! Adaptador FX para el TreeView compartido; estado visual independiente por pista.
mod audio;
mod commands;
mod drag;
mod host;
mod input;
mod menu;
mod presentation;
mod view;
use super::components::tree::TreeStyle;
use elevate::musical::{
    fx::{FxChain, FxEdit, FxId},
    track::TrackId,
};
use gpui::{Context, EventEmitter, FocusHandle, Pixels, Point, UniformListScrollHandle};
pub(super) use host::FxDock;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
pub(super) struct FxTarget {
    pub track: TrackId,
    pub node: Option<FxId>,
}
pub(super) enum FxEvent {
    Focused,
    Selection(FxTarget),
    Close,
    Add(FxTarget),
    Action(FxTarget, &'static str),
    Edit(TrackId, FxEdit),
    Menu(FxTarget, Point<Pixels>),
}
#[derive(Default)]
struct Presentation {
    selected: Option<FxId>,
    collapsed: HashSet<FxId>,
    root_closed: bool,
    scroll: UniformListScrollHandle,
}
#[derive(Clone)]
struct Row {
    id: Option<FxId>,
    depth: usize,
    name: String,
    container: bool,
    enabled: bool,
    parallel: bool,
}
pub(super) struct FxTree {
    style: TreeStyle,
    pub focus: FocusHandle,
    pub track: Option<TrackId>,
    name: String,
    chain: FxChain,
    views: HashMap<TrackId, Presentation>,
    rows: Vec<Row>,
    suspended: bool,
}
impl EventEmitter<FxEvent> for FxTree {}
impl FxTree {
    fn new(style: TreeStyle, cx: &mut Context<Self>) -> Self {
        Self {
            style,
            focus: cx.focus_handle(),
            track: None,
            name: String::new(),
            chain: FxChain::new(),
            views: HashMap::new(),
            rows: Vec::new(),
            suspended: false,
        }
    }
}
