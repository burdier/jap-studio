//! Contratos del catálogo headless; no requieren GPUI ni dispositivo de audio.
use elevate::catalog::{self, CatalogAction, EntryKind};
use std::collections::HashSet;

#[test]
fn search_is_accent_insensitive_and_finds_every_category() {
    for (query, expected) in [
        ("PIANO", EntryKind::Instrument),
        ("volumen", EntryKind::Primitive),
        ("componente serial", EntryKind::Component),
        ("reproduccion", EntryKind::Action),
        ("diseno", EntryKind::Action),
    ] {
        let results = catalog::search(query);
        assert!(
            results.iter().any(|entry| entry.kind == expected),
            "«{query}» debería encontrar {expected:?}"
        );
    }
    assert!(catalog::search("zzzz").is_empty());
    assert_eq!(catalog::search("").len(), catalog::entries().len());
}

#[test]
fn ids_are_unique_and_invoked_actions_are_named() {
    let mut ids = HashSet::new();
    for entry in catalog::entries() {
        assert!(ids.insert(entry.id), "ID duplicado: {}", entry.id);
        assert!(!entry.name.is_empty());
        assert!(!entry.summary.is_empty());
        match entry.action {
            CatalogAction::Invoke(command) => assert!(!command.is_empty()),
            CatalogAction::Unavailable(reason) => assert!(!reason.is_empty()),
        }
    }
    let piano = catalog::find("instrument.piano").expect("piano registrado");
    assert_eq!(piano.kind, EntryKind::Instrument);
    assert_eq!(piano.action, CatalogAction::Invoke("piano"));
}

#[test]
fn clip_edit_actions_are_searchable_and_dispatch_to_named_commands() {
    for (id, command) in [
        ("action.clip.materialize", "clip.materialize"),
        ("action.clip.duplicate", "clip.duplicate"),
        ("action.clip.split", "clip.split"),
        ("action.clip.restore_trim", "clip.restore_trim"),
        ("action.clip.mute", "clip.mute"),
        ("action.clip.lock", "clip.lock"),
        ("action.clip.rename", "clip.rename"),
        ("action.clip.gain", "clip.gain"),
        ("action.clip.gain.down", "clip.gain.down"),
        ("action.clip.gain.reset", "clip.gain.reset"),
        ("action.clip.gain.up", "clip.gain.up"),
        ("action.clip.color", "clip.color"),
        ("action.clip.track.previous", "clip.track.previous"),
        ("action.clip.track.next", "clip.track.next"),
        ("action.clip.delete", "clip.delete"),
    ] {
        let entry = catalog::find(id).unwrap_or_else(|| panic!("falta {id}"));
        assert_eq!(entry.kind, EntryKind::Action);
        assert_eq!(entry.action, CatalogAction::Invoke(command));
        assert!(catalog::search("clip").iter().any(|found| found.id == id));
    }
}
