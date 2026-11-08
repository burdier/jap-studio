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
