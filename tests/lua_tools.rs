#[path = "lua_tools/arguments.rs"]
mod arguments;
#[path = "lua_tools/execution.rs"]
mod execution;
#[path = "lua_packages/support.rs"]
mod fixtures;
#[path = "lua_tools/reload.rs"]
mod reload;

use elevate::{
    actions::ParameterValues,
    musical::{
        model::{InstrumentId, Pattern},
        time::Tick,
    },
    scripting::{
        actions::MidiOptions,
        packages::{PackageRegistry, ToolInvocation, ToolRequest},
    },
};
use fixtures::Fixture;

fn source(body: &str, parameters: &str) -> String {
    fixtures::source("user.midi.accent", parameters).replace(
        "function() error('run no se ejecuta al registrar') end",
        &format!("function(context, params) {body} end"),
    )
}

fn registry(body: &str, parameters: &str) -> (Fixture, PackageRegistry) {
    let fixture = Fixture::new();
    fixture.write("tool.lua", source(body, parameters));
    let registry = fixture.load().unwrap();
    (fixture, registry)
}

fn pattern() -> Pattern {
    let mut pattern = Pattern::new(Tick(3840)).unwrap();
    pattern
        .add_note(InstrumentId(7), Tick(0), Tick(960), 60, 90)
        .unwrap();
    pattern
        .add_note(InstrumentId(8), Tick(960), Tick(480), 64, 80)
        .unwrap();
    elevate::scripting::edit_midi(
        "controls[1]={start=0,lane='cc1',value=40}",
        &pattern,
        &[],
        MidiOptions::default(),
    )
    .unwrap()
    .pattern
}

fn invoke(registry: &PackageRegistry, command: &str) -> ToolInvocation {
    match registry.request(command).expect("comando Lua").unwrap() {
        ToolRequest::Invoke(invocation) => invocation,
        _ => panic!("se esperaba invocación"),
    }
}

fn defaults(registry: &PackageRegistry) -> ParameterValues<String> {
    registry
        .get("user.midi.accent")
        .unwrap()
        .descriptor()
        .parameters
        .iter()
        .map(|parameter| {
            (
                parameter.key.clone(),
                parameter.parse(&parameter.default).unwrap(),
            )
        })
        .collect()
}

#[test]
fn catalog_uses_the_same_invocation_and_preserves_native_entries() {
    let (_fixture, registry) = registry("context.notes[1].velocity=100", "");
    let entries = elevate::catalog::tools::entries(&registry);
    let entry = &entries[0].entry;
    assert_eq!(entry.kind, elevate::catalog::EntryKind::Action);
    assert!(entry.matches("acentuar"));
    assert!(entry.matches("Lua"));
    assert!(entry.matches("user.midi.accent"));
    assert!(entry.matches("Edita notas"));
    let elevate::catalog::CatalogAction::Invoke(command) = &entry.action else {
        panic!()
    };
    let edit = invoke(&registry, command)
        .edit_midi(&pattern(), &[], MidiOptions::default())
        .unwrap();
    assert_eq!(edit.pattern.notes()[0].velocity, 100);
    assert!(elevate::catalog::find("action.midi.tools.reload").is_some());
    assert!(elevate::catalog::find("action.export").is_some());
    assert!(elevate::actions::find("midi.script").is_some());
}

#[test]
fn bundled_chord_utility_builds_voicings_inversions_bass_and_strum() {
    let dirs = elevate::scripting::packages::PackageDirectories {
        bundled: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/scripts/tools/midi"),
        user: None,
    };
    let mut registry = PackageRegistry::default();
    registry.reload(&dirs).unwrap();
    assert!(registry.get("hay.midi.chord_utility").is_some());

    let mut pat = Pattern::new(Tick(3840)).unwrap();
    let root_id = pat
        .add_note(InstrumentId(1), Tick(0), Tick(960), 60, 100)
        .unwrap();

    // Maj7 en 1ª inversión con bajo (-1 octava) y rasgueo de 20 ticks por voz.
    let edit = invoke(
        &registry,
        "midi.tool hay.midi.chord_utility maj7 1 close true 20",
    )
    .edit_midi(&pat, &[root_id], MidiOptions::default())
    .unwrap();

    let mut pitches: Vec<u8> = edit.pattern.notes().iter().map(|n| n.pitch).collect();
    pitches.sort_unstable();
    // C4 maj7 (60,64,67,71) -> 1ª inv (64,67,71,72) + bajo C3 (48).
    assert_eq!(pitches, vec![48, 64, 67, 71, 72]);

    let mut starts: Vec<i64> = edit.pattern.notes().iter().map(|n| n.start.0).collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![0, 20, 40, 60, 80]);
}
