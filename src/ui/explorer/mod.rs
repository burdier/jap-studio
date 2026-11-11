//! Vista autónoma: modelo headless, lecturas asíncronas y lista virtualizada.
mod input;
mod io;
mod view;

use super::components::tree::TreeStyle as ExplorerStyle;
use elevate::explorer::LibraryTree;
use gpui::{Context, EventEmitter, FocusHandle, Task, UniformListScrollHandle};
use std::{collections::HashMap, path::PathBuf};

pub(super) enum ExplorerEvent {
    Insert(PathBuf),
    Close,
    Focused,
}

pub(super) struct Explorer {
    pub(super) style: ExplorerStyle,
    pub(super) focus: FocusHandle,
    pub(super) vim_enabled: bool,
    tree: LibraryTree,
    selected: Option<PathBuf>,
    search: String,
    category: Option<String>,
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
            vim_enabled: true,
            tree: LibraryTree::default(),
            selected: None,
            search: String::new(),
            category: None,
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
        self.tree
            .rows()
            .iter()
            .find(|row| {
                Some(&row.entry.path) == self.selected.as_ref() && row.entry.kind.importable()
            })
            .map(|row| row.entry.path.clone())
    }
}
