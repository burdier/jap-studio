use super::{Explorer, ExplorerEvent};
use elevate::explorer::EntryKind;
use gpui::{Context, KeyDownEvent, ScrollStrategy, Window};

impl Explorer {
    pub(super) fn activate_row(&mut self, index: usize, double: bool, cx: &mut Context<Self>) {
        let Some(row) = self.visible_rows().get(index).cloned() else {
            return;
        };
        self.selected = Some(row.entry.path.clone());
        if row.entry.kind == EntryKind::Directory {
            // La primera pulsación ya expandió: el segundo down de un doble clic no la cierra.
            if !double {
                self.tree.toggle(&row.entry.path);
                self.scan_pending(cx);
            }
        } else if double && row.entry.kind.importable() {
            self.stop_preview();
            cx.emit(ExplorerEvent::Insert(row.entry.path.clone()));
        }
        if !double {
            self.preview_selection(
                (row.entry.kind == EntryKind::Audio).then_some(row.entry.path.clone()),
                cx,
            );
        }
        cx.notify();
    }

    pub(super) fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        let modifiers = event.keystroke.modifiers;
        let unmodified = !modifiers.control && !modifiers.alt && !modifiers.platform;
        if modifiers.control && !modifiers.alt && !modifiers.platform && event.keystroke.key == "f"
        {
            self.focus_search(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.search_has_focus(window, cx) {
            if key == "escape" {
                window.focus(&self.focus, cx);
                cx.stop_propagation();
            }
            return;
        }
        if unmodified && key == "escape" {
            cx.stop_propagation();
            if !event.is_held {
                cx.emit(ExplorerEvent::Close);
            }
            return;
        }
        // El transporte del workspace recibe Espacio aunque el árbol conserve el foco.
        if matches!(key, " " | "space") {
            return;
        }
        cx.stop_propagation();
        if !unmodified {
            return;
        }
        let current = self
            .visible_rows()
            .iter()
            .position(|row| Some(&row.entry.path) == self.selected.as_ref());
        match key {
            "down" | "up" => {
                if self.visible_rows().is_empty() {
                    return;
                }
                let last = self.visible_rows().len() - 1;
                let index = match current {
                    Some(index) if key == "up" => index.saturating_sub(1),
                    Some(index) => (index + 1).min(last),
                    None => 0,
                };
                self.select_and_scroll(index, cx);
            }
            "enter" if !event.is_held => {
                if let Some(index) = current {
                    let file = self.visible_rows()[index].entry.kind != EntryKind::Directory;
                    self.activate_row(index, file, cx);
                }
            }
            "right" => {
                if let Some(index) = current {
                    let row = &self.visible_rows()[index];
                    if row.entry.kind == EntryKind::Audio {
                        if !event.is_held {
                            self.restart_sample_preview(row.entry.path.clone(), cx);
                        }
                    } else if row.entry.kind == EntryKind::Directory && !row.expanded {
                        self.activate_row(index, false, cx);
                    } else if row.expanded
                        && self
                            .visible_rows()
                            .get(index + 1)
                            .is_some_and(|next| next.depth > row.depth)
                    {
                        self.select_and_scroll(index + 1, cx);
                    }
                }
            }
            "left" => {
                if let Some(index) = current {
                    let row = &self.visible_rows()[index];
                    if row.expanded {
                        self.activate_row(index, false, cx);
                    } else if let Some(parent) = (0..index)
                        .rev()
                        .find(|&i| self.visible_rows()[i].depth < row.depth)
                    {
                        self.select_and_scroll(parent, cx);
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }
    fn select_and_scroll(&mut self, index: usize, cx: &mut Context<Self>) {
        let entry = &self.visible_rows()[index].entry;
        let path = entry.path.clone();
        let audio = (entry.kind == EntryKind::Audio).then_some(path.clone());
        self.selected = Some(path);
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        self.preview_selection(audio, cx);
    }
}
