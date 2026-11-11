#[path = "lua_packages/descriptor.rs"]
mod descriptor;
#[path = "lua_packages/limits.rs"]
mod limits;
#[path = "lua_packages/support.rs"]
mod support;

use elevate::{
    actions::{self, ParameterKind, ParameterValue},
    scripting::packages::{PackagePhase, PackageRegistry},
};
use std::{fs, sync::Arc};
use support::{Fixture, NUMBER, altered, source};

#[test]
fn discovery_copies_owned_metadata_and_never_calls_run() {
    let fixture = Fixture::new();
    fs::write(
        fixture.directories.bundled.join("a.lua"),
        source("hay.midi.accent", ""),
    )
    .unwrap();
    let parameters = format!(
        "{NUMBER}, {{key='name',label='Nombre',kind='text',default='Nota'}}, {{key='enabled',label='Activo',kind='bool',default=false}}"
    );
    let source = source("user.midi.accent", &parameters);
    let path = fixture.write("tool.lua", &source);
    let registry = fixture.load().unwrap();
    assert_eq!(registry.generation(), 1);
    assert_eq!(
        registry
            .packages()
            .map(|p| p.descriptor().id.as_str())
            .collect::<Vec<_>>(),
        ["hay.midi.accent", "user.midi.accent"]
    );
    let package = registry.get("user.midi.accent").unwrap().clone();
    assert_eq!(package.path(), path);
    assert_eq!(package.source(), source);
    let descriptor = package.descriptor();
    assert_eq!(descriptor.api_version, 1);
    assert_eq!(descriptor.name, "Acentuar");
    assert_eq!(descriptor.description, "Edita notas");
    assert_eq!(descriptor.parameters.len(), 3);
    assert!(
        matches!(&descriptor.parameters[0].kind, ParameterKind::Number { unit, .. } if unit == "velocity")
    );
    assert_eq!(descriptor.parameters[0].hint, "");
    assert!(matches!(
        descriptor.parameters[1].parse("Otra nota"),
        Ok(ParameterValue::Text(_))
    ));
    assert!(matches!(
        descriptor.parameters[2].parse("false"),
        Ok(ParameterValue::Bool(false))
    ));
    drop(registry);
    drop(fixture);
    assert_eq!(package.source(), source);
    // Los snapshots pueden viajar al worker sin ningún handle Lua.
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PackageRegistry>();
}

#[test]
fn reload_is_transactional_and_retains_old_sources_for_pending_work() {
    let fixture = Fixture::new();
    let path = fixture.write("a.lua", source("user.midi.accent", NUMBER));
    let mut registry = fixture.load().unwrap();
    let snapshot = registry.get("user.midi.accent").unwrap().clone();
    fs::write(&path, altered("p.name='Nuevo'")).unwrap();
    let bad = fixture.write("z.lua", "return false");
    assert!(registry.reload(&fixture.directories).is_err());
    assert_eq!(registry.generation(), 1);
    assert!(Arc::ptr_eq(
        registry.get("user.midi.accent").unwrap(),
        &snapshot
    ));
    fs::remove_file(bad).unwrap();
    registry.reload(&fixture.directories).unwrap();
    assert_eq!(registry.generation(), 2);
    assert_eq!(
        registry.get("user.midi.accent").unwrap().descriptor().name,
        "Nuevo"
    );
    assert_eq!(snapshot.descriptor().name, "Acentuar");
    fs::remove_file(path).unwrap();
    registry.reload(&fixture.directories).unwrap();
    assert_eq!(registry.generation(), 3);
    assert_eq!(registry.packages().len(), 0);
    assert!(snapshot.source().contains("run=function()"));
}

#[test]
fn duplicate_ids_report_both_sorted_paths_and_preserve_native_actions() {
    let fixture = Fixture::new();
    let second = fixture.write("z.lua", source("user.duplicate", ""));
    let first = fixture.write("a.lua", source("user.duplicate", ""));
    let mut registry = PackageRegistry::default();
    let error = registry.reload(&fixture.directories).unwrap_err();
    assert_eq!(error.phase, PackagePhase::Descriptor);
    assert_eq!(error.path, second);
    assert_eq!(error.id.as_deref(), Some("user.duplicate"));
    assert_eq!(error.field.as_deref(), Some("id"));
    assert!(error.message.contains(first.to_str().unwrap()));
    assert!(error.to_string().starts_with("descriptor · "));
    assert_eq!(registry.generation(), 0);
    assert_eq!(registry.packages().len(), 0);
    assert!(actions::find("midi.script").is_some());
    fs::remove_file(first).unwrap();
    fs::write(
        fixture.directories.bundled.join("b.lua"),
        source("user.duplicate", ""),
    )
    .unwrap();
    assert!(
        registry
            .reload(&fixture.directories)
            .unwrap_err()
            .message
            .contains("ID duplicado")
    );
}

#[test]
fn discovery_is_shallow_and_missing_directories_are_empty() {
    let fixture = Fixture::new();
    fixture.write("ignored.txt", "invalid Lua");
    let sub = fixture.directories.user.as_ref().unwrap().join("sub.lua");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("invalid.lua"), "invalid Lua").unwrap();
    assert_eq!(fixture.load().unwrap().packages().len(), 0);
    let mut directories = fixture.directories.clone();
    directories.bundled = directories.bundled.join("missing");
    directories.user = Some(sub.join("missing"));
    let mut registry = PackageRegistry::default();
    registry.reload(&directories).unwrap();
    directories.user = None;
    registry.reload(&directories).unwrap();
    assert_eq!(registry.generation(), 2);
    directories.bundled = fixture.write("not-a-directory", "");
    let error = registry.reload(&directories).unwrap_err();
    assert_eq!(error.phase, PackagePhase::Load);
    assert_eq!(registry.generation(), 2);
}

#[cfg(unix)]
#[test]
fn symlinks_and_special_files_are_not_read() {
    use std::os::{unix::fs::symlink, unix::net::UnixListener};
    let fixture = Fixture::new();
    let target = fixture.write("ignored.txt", "while true do end");
    let root = fixture.directories.user.as_ref().unwrap();
    symlink(&target, root.join("link.lua")).unwrap();
    symlink(root.join("missing"), root.join("dangling.lua")).unwrap();
    let _socket = UnixListener::bind(root.join("socket.lua")).unwrap();
    assert_eq!(fixture.load().unwrap().packages().len(), 0);
    let mut directories = fixture.directories.clone();
    let link = fixture.directories.bundled.join("linked-directory");
    symlink(root, &link).unwrap();
    directories.user = Some(link);
    assert!(PackageRegistry::default().reload(&directories).is_err());
}
