//! Ratón y teclado despachan las mismas acciones; ningún evento llega al piano al escribir.
use super::{ClipAction as A, MenuPage};
use crate::ui::DawApp;
use gpui::{Context, KeyDownEvent, Window};

impl DawApp {
    pub(in crate::ui) fn handle_clip_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        // El selector es una fila de muestras, no una lista vertical de nombres.
        if matches!(key, "left" | "right") {
            let colors: Vec<_> = self
                .clip_menu_items()
                .iter()
                .enumerate()
                .filter_map(|(index, item)| matches!(item.action, A::Color(_)).then_some(index))
                .collect();
            if let Some(menu) = &mut self.clip_menu {
                if let Some(current) = colors.iter().position(|index| *index == menu.selected) {
                    let next = if key == "right" {
                        (current + 1) % colors.len()
                    } else {
                        (current + colors.len() - 1) % colors.len()
                    };
                    menu.selected = colors[next];
                    cx.notify();
                    return;
                }
            }
        }
        let Some(menu) = &mut self.clip_menu else {
            return;
        };
        if menu.page == MenuPage::Rename {
            match key {
                "escape" => {
                    menu.page = MenuPage::Main;
                    menu.selected = 0;
                }
                "enter" => self.run_clip_menu_action(A::Rename, window, cx),
                "a" if event.keystroke.modifiers.control || event.keystroke.modifiers.platform => {
                    menu.select_name = true
                }
                "backspace" | "delete" => {
                    if menu.select_name {
                        menu.rename.clear();
                    } else {
                        menu.rename.pop();
                    }
                    menu.select_name = false;
                }
                _ if !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.alt =>
                {
                    if let Some(text) = &event.keystroke.key_char {
                        if menu.select_name {
                            menu.rename.clear();
                        }
                        menu.select_name = false;
                        for ch in text.chars().filter(|ch| !ch.is_control()) {
                            if menu.rename.chars().count() < 120 {
                                menu.rename.push(ch);
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            match key {
                "escape" | "left" if menu.page != MenuPage::Main => {
                    menu.page = MenuPage::Main;
                    menu.selected = 0;
                }
                "escape" => self.clip_menu = None,
                "up" | "down" => {
                    let current = menu.selected;
                    let items = self.clip_menu_items();
                    let count = items.len();
                    for step in 1..=count {
                        let next = if key == "down" {
                            (current + step) % count
                        } else {
                            (current + count - step) % count
                        };
                        if items[next].enabled {
                            self.clip_menu.as_mut().unwrap().selected = next;
                            break;
                        }
                    }
                }
                "enter" | "right" => {
                    let index = menu.selected;
                    if let Some(item) = self
                        .clip_menu_items()
                        .get(index)
                        .filter(|item| item.enabled)
                    {
                        if key == "enter" || matches!(item.action, A::Page(_)) {
                            self.run_clip_menu_action(item.action, window, cx);
                        }
                    }
                }
                _ => {}
            }
        }
        cx.notify();
    }

    pub(in crate::ui) fn run_clip_menu_action(
        &mut self,
        action: A,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = &self.clip_menu else { return };
        if let A::Page(page) = action {
            let name = menu
                .clip_kind
                .map(|kind| self.active_clip_name(kind).to_string())
                .unwrap_or_default();
            let menu = self.clip_menu.as_mut().unwrap();
            menu.page = page;
            menu.selected = 0;
            if page == MenuPage::Rename {
                menu.rename = name;
                menu.select_name = true;
            }
            return;
        }
        if matches!(action, A::InsertMidi | A::InsertCapture) {
            if menu.clip_kind.is_some() {
                return;
            }
            match action {
                A::InsertMidi => {
                    self.insert_midi_clip_from_menu();
                    self.focus_piano_roll(window, cx);
                }
                A::InsertCapture => {
                    self.clip_menu = None;
                    self.capture_midi_to_track();
                }
                _ => unreachable!(),
            }
            return;
        }
        let Some(kind) = menu.clip_kind else { return };
        let beat = menu.beat;
        match action {
            A::EditMidi => {
                if kind == crate::ui::timeline::DragKind::Midi {
                    self.open_piano_roll();
                    self.focus_piano_roll(window, cx);
                }
            }
            A::Rename => {
                let name = menu.rename.clone();
                if name.trim().is_empty() {
                    return;
                }
                self.rename_active_clip(kind, &name);
            }
            A::Duplicate => self.duplicate_active_clip(kind),
            A::Materialize => self.materialize_midi_selection(),
            A::Split => {
                self.stop_audio();
                self.timeline.split_clip(kind, beat);
            }
            A::RestoreTrim => {
                self.stop_audio();
                self.timeline.restore_clip_trim(kind);
            }
            A::Mute => self.toggle_clip_mute(kind),
            A::Lock => self.toggle_clip_lock(kind),
            A::Color(color) => {
                if let Some(props) = self.active_clip_props_mut(kind) {
                    props.color_index = color;
                }
            }
            A::Gain(delta) => {
                self.stop_audio();
                if let Some(props) = self.active_clip_props_mut(kind) {
                    props.gain_db =
                        delta.map_or(0.0, |delta| (props.gain_db + delta).clamp(-24.0, 12.0));
                }
                return;
            }
            A::Move(delta) => self.move_active_clip_track(kind, delta),
            A::RouteMidi(target) => self.route_active_midi_clip(target),
            A::Delete => self.delete_active_clip(kind),
            A::Page(_) | A::InsertMidi | A::InsertCapture => unreachable!(),
        }
        self.clip_menu = None;
    }
}
