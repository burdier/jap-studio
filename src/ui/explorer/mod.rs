//! Vista autónoma: modelo headless, lecturas asíncronas y lista virtualizada.
mod input;
mod io;
mod preview;
mod search;
mod view;

pub(super) use view::ExplorerSampleDrag;

use super::components::tree::TreeStyle as ExplorerStyle;
use elevate::explorer::LibraryTree;
use gpui::{Context, EventEmitter, FocusHandle, Task, UniformListScrollHandle};
use std::{collections::HashMap, path::PathBuf};

pub(super) enum ExplorerEvent {
    Insert(PathBuf),
    PreviewPhrase(PathBuf, u8, i32),
    LoadRoot(PathBuf, u8, i32),
    AddToHilo(PathBuf, u8, i32),
    Close,
    Focused,
}

pub(super) struct Explorer {
    pub(super) style: ExplorerStyle,
    pub(super) focus: FocusHandle,
    tree: LibraryTree,
    selected: Option<PathBuf>,
    search: search::Search,
    preview: preview::Preview,
    pub(super) has_phrase: bool,
    scroll: UniformListScrollHandle,
    config: Option<PathBuf>,
    ready: bool,
    status: Option<String>,
    save_error: Option<String>,
    revision: u64,
    saving: bool,
    picking: bool,
    generation: u64,
    scans: HashMap<PathBuf, Task<()>>,
    restore_task: Option<Task<()>>,
    picker_task: Option<Task<()>>,
    save_task: Option<Task<()>>,
}

impl EventEmitter<ExplorerEvent> for Explorer {}

impl Explorer {
    pub(super) fn new(style: ExplorerStyle, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            style,
            focus: cx.focus_handle(),
            tree: LibraryTree::default(),
            selected: None,
            search: search::Search::default(),
            preview: preview::Preview::default(),
            has_phrase: false,
            scroll: UniformListScrollHandle::new(),
            config: dirs::config_dir().map(|path| path.join("hay-daw/libraries")),
            ready: false,
            status: None,
            save_error: None,
            revision: 0,
            saving: false,
            picking: false,
            generation: 0,
            scans: HashMap::new(),
            restore_task: None,
            picker_task: None,
            save_task: None,
        };
        this.restore(cx);
        this
    }

    pub(super) fn selected_file(&self) -> Option<PathBuf> {
        self.visible_rows()
            .iter()
            .find(|row| {
                Some(&row.entry.path) == self.selected.as_ref() && row.entry.kind.importable()
            })
            .map(|row| row.entry.path.clone())
    }
}
