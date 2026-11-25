//! Índice de nombres por biblioteca. Busca también en carpetas cerradas.
use super::{Explorer, ExplorerEvent};
use elevate::explorer::{Entry, EntryKind, Row, read_directory};
use gpui::{
    AppContext, Context, Entity, Focusable, ScrollStrategy, Subscription, Task, Window,
    component::input::{InputEvent, InputState},
};
use std::{collections::HashSet, path::PathBuf};

const RESULT_LIMIT: usize = 500;

#[derive(Default)]
pub(super) struct Search {
    pub input: Option<Entity<InputState>>,
    subscription: Option<Subscription>,
    pub query: String,
    rows: Vec<Row>,
    index: Option<Vec<(String, Entry)>>,
    task: Option<Task<()>>,
    truncated: bool,
}

impl Explorer {
    pub(super) fn visible_rows(&self) -> &[Row] {
        if self.search.query.is_empty() {
            self.tree.rows()
        } else {
            &self.search.rows
        }
    }

    pub(super) fn search_message(&self) -> Option<&'static str> {
        if self.search.query.is_empty() {
            None
        } else if self.search.task.is_some() {
            Some("Buscando en bibliotecas…")
        } else if self.search.rows.is_empty() {
            Some("Sin coincidencias")
        } else if self.search.truncated {
            Some("Primeras 500 coincidencias")
        } else {
            None
        }
    }

    pub(super) fn init_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Buscar archivos…"));
        self.search.subscription = Some(cx.subscribe_in(
            &input,
            window,
            |this, input, event, window, cx| match event {
                InputEvent::Change => {
                    let query = input.read(cx).value().trim().to_lowercase();
                    if this.search.query != query {
                        this.search.query = query;
                        this.stop_preview();
                        this.update_search(cx);
                    }
                }
                InputEvent::PressEnter { .. } => window.focus(&this.focus, cx),
                InputEvent::Focus => cx.emit(ExplorerEvent::Focused),
                InputEvent::Blur => {}
            },
        ));
        self.search.input = Some(input);
    }

    pub(in crate::ui) fn search_has_focus(&self, window: &Window, cx: &gpui::App) -> bool {
        self.search
            .input
            .as_ref()
            .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window))
    }

    pub(super) fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.search.input {
            let focus = input.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        }
    }

    pub(super) fn invalidate_search(&mut self, cx: &mut Context<Self>) {
        self.search.task = None;
        self.search.index = None;
        self.update_search(cx);
    }

    fn update_search(&mut self, cx: &mut Context<Self>) {
        self.search.rows.clear();
        self.search.truncated = false;
        if !self.search.query.is_empty() {
            if let Some(index) = &self.search.index {
                let terms: Vec<_> = self.search.query.split_whitespace().collect();
                let matches = index
                    .iter()
                    .filter(|(path, _)| terms.iter().all(|term| path.contains(term)));
                for (_, entry) in matches.take(RESULT_LIMIT + 1) {
                    if self.search.rows.len() == RESULT_LIMIT {
                        self.search.truncated = true;
                        break;
                    }
                    self.search.rows.push(Row {
                        entry: entry.clone(),
                        depth: 0,
                        root: false,
                        expanded: false,
                        status: None,
                    });
                }
            } else if self.search.task.is_none() && !self.tree.roots().is_empty() {
                let roots = self.tree.roots().to_vec();
                let work = cx.background_spawn(async move { build_index(roots) });
                self.search.task = Some(cx.spawn(async move |this, cx| {
                    let index = work.await;
                    let _ = this.update(cx, |this, cx| {
                        this.search.task = None;
                        this.search.index = Some(index);
                        this.update_search(cx);
                    });
                }));
            }
        }
        let current = self
            .visible_rows()
            .iter()
            .position(|row| Some(&row.entry.path) == self.selected.as_ref());
        if let Some(index) = current {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        } else {
            self.selected = (!self.search.query.is_empty())
                .then(|| self.search.rows.first())
                .flatten()
                .map(|row| row.entry.path.clone());
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        cx.notify();
    }
}

fn build_index(roots: Vec<PathBuf>) -> Vec<(String, Entry)> {
    let mut seen = HashSet::new();
    let mut index = Vec::new();
    for root in roots {
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            if !seen.insert(directory.clone()) {
                continue;
            }
            let Ok(entries) = read_directory(&directory) else {
                continue;
            };
            for entry in entries {
                if entry.kind == EntryKind::Directory {
                    pending.push(entry.path);
                } else if entry.kind.importable() {
                    let key = entry
                        .path
                        .strip_prefix(&root)
                        .unwrap_or(&entry.path)
                        .to_string_lossy()
                        .to_lowercase();
                    index.push((key, entry));
                }
            }
        }
    }
    index.sort_by_cached_key(|(_, entry)| entry.name.to_lowercase());
    index
}
