use super::{Explorer, ExplorerEvent};
use elevate::explorer::EntryKind;
use gpui::{Context, KeyDownEvent, ScrollStrategy};

impl Explorer {
    pub(super) fn activate_row(&mut self, index: usize, double: bool, cx: &mut Context<Self>) {
        let Some(row) = self.tree.rows().get(index).cloned() else {
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
            cx.emit(ExplorerEvent::Insert(row.entry.path));
        }
        cx.notify();
    }

    pub(super) fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        let modifiers = event.keystroke.modifiers;
        if !modifiers.control && !modifiers.alt && !modifiers.platform {
            if key == "backspace" {
                self.search.pop();
                self.selected = None;
                cx.notify();
                return;
            }
            if let Some(character) = event.keystroke.key_char.as_deref()
                && !character.chars().any(char::is_control)
                && !matches!(character, " " | "space")
            {
                self.search.push_str(character);
                self.selected = None;
                cx.notify();
                return;
            }
        }
        // El transporte del workspace recibe Espacio aunque el árbol conserve el foco.
        if matches!(key, " " | "space") {
            return;
        }
        cx.stop_propagation();
        if !self.vim_enabled && matches!(key, "h" | "j" | "k" | "l" | "e") { return; }
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        if matches!(key, "escape" | "e") {
            if !event.is_held {
                cx.emit(ExplorerEvent::Close);
            }
            return;
        }
        let current = self
            .tree
            .rows()
            .iter()
            .position(|row| Some(&row.entry.path) == self.selected.as_ref());
        match key {
            "j" | "down" | "k" | "up" => {
                if self.tree.rows().is_empty() {
                    return;
                }
                let last = self.tree.rows().len() - 1;
                let index = match current {
                    Some(index) if matches!(key, "k" | "up") => index.saturating_sub(1),
                    Some(index) => (index + 1).min(last),
                    None => 0,
                };
                self.selected = Some(self.tree.rows()[index].entry.path.clone());
                self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            }
            "enter" if !event.is_held => {
                if let Some(index) = current {
                    let file = self.tree.rows()[index].entry.kind != EntryKind::Directory;
                    self.activate_row(index, file, cx);
                }
            }
            "l" | "right" => {
                if let Some(index) = current {
                    let row = &self.tree.rows()[index];
                    if row.entry.kind == EntryKind::Directory && !row.expanded {
                        self.activate_row(index, false, cx);
                    } else if row.expanded
                        && self
                            .tree
                            .rows()
                            .get(index + 1)
                            .is_some_and(|next| next.depth > row.depth)
                    {
                        self.selected = Some(self.tree.rows()[index + 1].entry.path.clone());
                        self.scroll
                            .scroll_to_item(index + 1, ScrollStrategy::Nearest);
                    }
                }
            }
            "h" | "left" => {
                if let Some(index) = current {
                    let row = &self.tree.rows()[index];
                    if row.expanded {
                        self.activate_row(index, false, cx);
                    } else if let Some(parent) = (0..index)
                        .rev()
                        .find(|&i| self.tree.rows()[i].depth < row.depth)
                    {
                        self.selected = Some(self.tree.rows()[parent].entry.path.clone());
                        self.scroll.scroll_to_item(parent, ScrollStrategy::Nearest);
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }
}
