use super::support::{Fixture, altered, source};
use elevate::scripting::packages::PackagePhase;
use std::fs;

#[test]
fn instruction_and_memory_budgets_abort_evaluation() {
    let fixture = Fixture::new();
    for body in [
        "while true do end",
        "local huge=string.rep('x',16*1024*1024)",
    ] {
        fixture.write("bad.lua", body);
        let error = fixture.load().unwrap_err();
        assert_eq!(error.phase, PackagePhase::Load);
        assert!(
            error.message.contains("250 ms") || error.message.contains("memory"),
            "{error}"
        );
    }
}

#[test]
fn files_are_utf8_text_and_at_most_256_kib() {
    let fixture = Fixture::new();
    fixture.write("bad.lua", [0xff]);
    assert_eq!(fixture.load().unwrap_err().phase, PackagePhase::Load);
    // Bytecode real de Lua: el descriptor nunca puede activar el modo binario.
    let lua = mlua::Lua::new();
    let bytecode = lua
        .load(source("user.bytecode", ""))
        .into_function()
        .unwrap()
        .dump(false);
    fixture.write("bad.lua", bytecode);
    assert!(fixture.load().is_err());
    let mut valid = source("user.limit", "");
    valid.push_str(&" ".repeat(256 * 1024 - valid.len()));
    fixture.write("bad.lua", &valid);
    assert!(fixture.load().is_ok());
    valid.push(' ');
    fixture.write("bad.lua", valid);
    assert!(fixture.load().unwrap_err().message.contains("256 KiB"));
}

#[test]
fn parameter_limit_is_32_and_empty_arrays_are_valid() {
    let fixture = Fixture::new();
    fixture.write("tool.lua", source("user.empty", ""));
    assert!(fixture.load().is_ok());
    let body = "p.parameters={}; for i=1,32 do p.parameters[i]={key='p'..i,label='P',kind='bool',default=false} end";
    fixture.write("tool.lua", altered(body));
    assert_eq!(
        fixture
            .load()
            .unwrap()
            .get("user.midi.accent")
            .unwrap()
            .descriptor()
            .parameters
            .len(),
        32
    );
    fixture.write(
        "tool.lua",
        altered(&format!("{body}; p.parameters[33]=p.parameters[1]")),
    );
    assert!(fixture.load().is_err());
}

#[test]
fn package_count_limit_applies_across_both_roots() {
    let fixture = Fixture::new();
    for i in 0..128 {
        fixture.write(&format!("{i:03}.lua"), source(&format!("user.p{i}"), ""));
    }
    let mut registry = fixture.load().unwrap();
    assert_eq!(registry.packages().len(), 128);
    fs::write(
        fixture.directories.bundled.join("extra.lua"),
        source("hay.extra", ""),
    )
    .unwrap();
    assert!(
        registry
            .reload(&fixture.directories)
            .unwrap_err()
            .message
            .contains("128 paquetes")
    );
    assert_eq!(registry.generation(), 1);
    assert_eq!(registry.packages().len(), 128);
}

#[test]
fn entry_limit_counts_ignored_files_too() {
    let fixture = Fixture::new();
    for i in 0..1024 {
        fixture.write(&format!("{i}.txt"), "");
    }
    assert!(fixture.load().is_ok());
    fixture.write("overflow.txt", "");
    assert!(
        fixture
            .load()
            .unwrap_err()
            .message
            .contains("1024 entradas")
    );
}

#[test]
fn aggregate_source_limit_applies_across_both_roots() {
    let fixture = Fixture::new();
    for i in 0..32 {
        let mut text = source(&format!("user.p{i}"), "");
        text.push_str(&" ".repeat(256 * 1024 - text.len()));
        fixture.write(&format!("{i:03}.lua"), text);
    }
    let mut registry = fixture.load().unwrap();
    assert_eq!(registry.packages().len(), 32);
    fs::write(
        fixture.directories.bundled.join("extra.lua"),
        source("hay.extra", ""),
    )
    .unwrap();
    assert!(
        registry
            .reload(&fixture.directories)
            .unwrap_err()
            .message
            .contains("8 MiB")
    );
    assert_eq!(registry.generation(), 1);
    assert_eq!(registry.packages().len(), 32);
}
