use super::{FxEvent, FxTarget, FxTree};
use elevate::musical::fx::FxEdit;
use gpui::{Context, KeyDownEvent, ScrollStrategy};
impl FxTree {
    pub(super) fn activate(&mut self, index: usize, double: bool, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(index).cloned() else {
            return;
        };
        let Some(track) = self.track else {
            return;
        };
        let view = self.views.entry(track).or_default();
        view.selected = row.id;
        view.instrument_selected = row.instrument;
        if row.instrument {
            cx.emit(FxEvent::Instrument(track));
            cx.emit(FxEvent::Selection(FxTarget { track, node: None }));
            cx.notify();
            return;
        }
        if row.container && !double {
            if let Some(id) = row.id {
                if !view.collapsed.remove(&id) {
                    view.collapsed.insert(id);
                }
            } else {
                view.root_closed = !view.root_closed;
            }
            self.rebuild();
        } else if double && !row.container {
            if let Some(target) = self.target() {
                cx.emit(FxEvent::Action(target, "fx.parameters"));
            }
        }
        if let Some(target) = self.target() {
            cx.emit(FxEvent::Selection(target));
        }
        cx.notify();
    }
    pub(super) fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.suspended {
            return;
        }
        let key = event.keystroke.key.as_str();
        if matches!(key, "space" | " ") {
            return;
        }
        cx.stop_propagation();
        let Some(target) = self.target() else {
            return;
        };
        let modifiers = event.keystroke.modifiers;
        if modifiers.control && key == "d" {
            if !event.is_held {
                if let Some(id) = target.node {
                    cx.emit(FxEvent::Edit(target.track, FxEdit::Duplicate(id)));
                }
            }
            return;
        }
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        let current = self
            .rows
            .iter()
            .position(|row| {
                row.id == target.node
                    && row.instrument == self.views[&target.track].instrument_selected
            })
            .unwrap_or(0);
        match key {
            "escape" => cx.emit(FxEvent::Close),
            "insert" if !event.is_held => cx.emit(FxEvent::Add(target)),
            "delete" if !event.is_held => {
                if let Some(id) = target.node {
                    cx.emit(FxEvent::Edit(target.track, FxEdit::Remove(id)));
                }
            }
            "f2" if !event.is_held => cx.emit(FxEvent::Action(target, "fx.rename")),
            "enter" if !event.is_held => self.activate(current, !self.rows[current].container, cx),
            "up" | "down" => {
                let index = if key == "up" {
                    current.saturating_sub(1)
                } else {
                    (current + 1).min(self.rows.len().saturating_sub(1))
                };
                let view = self.views.get_mut(&target.track).unwrap();
                view.selected = self.rows.get(index).and_then(|row| row.id);
                view.instrument_selected = self.rows.get(index).is_some_and(|row| row.instrument);
                view.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            }
            "right" | "left" => {
                let view = self.views.get(&target.track).unwrap();
                let closed = target
                    .node
                    .map_or(view.root_closed, |id| view.collapsed.contains(&id));
                let opening = key == "right";
                if self.rows[current].container && closed == opening {
                    self.activate(current, false, cx);
                } else {
                    let index = if opening {
                        (current + 1).min(self.rows.len() - 1)
                    } else {
                        (0..current)
                            .rev()
                            .find(|i| self.rows[*i].depth < self.rows[current].depth)
                            .unwrap_or(0)
                    };
                    let view = self.views.get_mut(&target.track).unwrap();
                    view.selected = self.rows[index].id;
                    view.instrument_selected = self.rows[index].instrument;
                    view.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
                }
            }
            _ => {}
        }
        if let Some(target) = self.target() {
            cx.emit(FxEvent::Selection(target));
        }
        cx.notify();
    }
}
