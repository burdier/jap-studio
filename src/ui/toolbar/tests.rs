use super::{ToolbarAction as A, ToolbarScope as S};
use crate::ui::icons::{ToolbarBindings, ToolbarIcon};
use elevate::catalog::{self, CatalogAction, EntryKind};

fn bindings() -> ToolbarBindings {
    ToolbarBindings {
        icons: vec![ToolbarIcon {
            id: "test".into(),
            label: "Test".into(),
            svg_bytes: b"",
        }],
        transport_buttons: vec![],
        piano_roll_buttons: vec![],
        picker_open: true,
        picker_scope: S::Transport,
        selected_icon: 0,
        selected_action: A::InsertMidiClip,
        search_query: "test".into(),
        action_query: String::new(),
        searching_actions: true,
        custom_command: String::new(),
        editing_custom_command: false,
    }
}

#[test]
fn every_executable_catalog_action_is_bindable_without_duplicate_entries() {
    let actions = A::available(S::Transport, "");
    for entry in catalog::entries() {
        if entry.kind == EntryKind::Action && matches!(entry.action, CatalogAction::Invoke(_)) {
            assert_eq!(
                actions
                    .iter()
                    .filter(|a| **a == A::from_catalog(entry.id))
                    .count(),
                1,
                "{}",
                entry.id
            );
        }
    }
    assert!(actions.contains(&A::Record));
    assert!(actions.contains(&A::Catalog("action.clip.materialize")));
    let export = A::available(S::Transport, "exportar");
    assert_eq!(actions.first(), Some(&A::Catalog("action.export")));
    assert!(export.contains(&A::Catalog("action.export")));
    assert!(export.contains(&A::Catalog("action.export.settings")));
    assert!(matches!(
        catalog::find("action.export").unwrap().action,
        CatalogAction::Invoke("export")
    ));
}

#[test]
fn action_search_is_independent_of_icons_and_finds_catalog_keywords_and_accents() {
    assert!(A::available(S::Transport, "unir").contains(&A::Catalog("action.clip.materialize")));
    assert!(A::available(S::Transport, "grabacion").contains(&A::Record));
    assert!(A::available(S::PianoRoll, "altura").contains(&A::RollTaller));
    assert!(A::available(S::Transport, "no-existe-esta-accion").is_empty());
}

#[test]
fn scopes_and_metadata_stay_consistent_for_all_available_actions() {
    for scope in [S::Transport, S::PianoRoll] {
        let actions = A::available(scope, "");
        for (index, action) in actions.iter().enumerate() {
            assert_eq!(action.scope(), scope);
            assert!(!action.label().is_empty());
            assert!(!action.short_tag().is_empty());
            assert!(!actions[..index].contains(action));
        }
    }
    assert!(!A::available(S::PianoRoll, "").contains(&A::Catalog("action.clip.delete")));
    assert!(A::available(S::PianoRoll, "").contains(&A::RollResetView));
}

#[test]
fn searching_actions_preserves_icon_and_prevents_binding_a_hidden_action() {
    let mut picker = bindings();
    picker.edit_search("materialize");
    assert_eq!(
        picker.selected_action,
        A::Catalog("action.clip.materialize")
    );
    assert_eq!(picker.selected_icon, 0);
    assert_eq!(picker.search_query, "test");
    assert!(picker.can_bind_selected());
    picker.edit_search("xyz");
    assert!(!picker.can_bind_selected());
    for _ in 0..3 {
        picker.edit_search("backspace");
    }
    assert!(picker.can_bind_selected());
}

#[test]
fn reassigning_an_action_replaces_its_icon_and_keeps_bars_independent() {
    let mut picker = bindings();
    picker.selected_action = A::Catalog("action.clip.materialize");
    assert_eq!(picker.bind_selected().0, S::Transport);
    picker.icons.push(ToolbarIcon {
        id: "second".into(),
        label: "Second".into(),
        svg_bytes: b"",
    });
    picker.selected_icon = 1;
    picker.bind_selected();
    assert_eq!(picker.transport_buttons.len(), 1);
    assert_eq!(picker.transport_buttons[0].icon_id, "second");

    picker.open_picker(S::PianoRoll);
    picker.selected_action = A::RollResetView;
    assert_eq!(picker.bind_selected().0, S::PianoRoll);
    assert_eq!(picker.piano_roll_buttons.len(), 1);
    assert_eq!(
        picker.transport_buttons[0].action,
        A::Catalog("action.clip.materialize")
    );
}
