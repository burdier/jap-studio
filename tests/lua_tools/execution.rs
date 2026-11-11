use super::{invoke, pattern, registry};
use elevate::scripting::{actions::MidiOptions, packages::PackagePhase};

#[test]
fn context_units_selection_and_midi_controls_use_the_existing_contract() {
    let body = "assert(context.length==3840 and context.ppq==960 and context.grid_ticks==240); for _,n in ipairs(context.notes) do assert(n.selected); n.release_velocity=50; n.pan=-20; n.probability=75 end; context.controls[1].value=70; context.controls[2]={start=480,lane='pitch',value=123}; context.length=1; context.ppq=1; context.grid_ticks=1";
    let (_fixture, registry) = registry(body, "");
    let original = pattern();
    let options = MidiOptions {
        grid: elevate::musical::grid::NoteGrid::new(16).unwrap(),
        ..Default::default()
    };
    let edit = invoke(&registry, "midi.tool user.midi.accent")
        .edit_midi(&original, &[], options)
        .unwrap();
    assert_eq!(edit.pattern.length(), original.length());
    assert_eq!(edit.pattern.notes()[0].expression.pan, -20);
    assert_eq!(edit.pattern.notes()[1].expression.probability, 75);
    assert_eq!(
        edit.pattern.controls().points()[0].id,
        original.controls().points()[0].id
    );
    assert_eq!(edit.pattern.controls().points()[0].value, 70);
    assert_eq!(edit.pattern.controls().points().len(), 2);
    assert!(edit.pattern.controls().points()[1].id >= original.controls().next_id());
}

#[test]
fn invalid_results_reject_the_whole_edit_without_mutating_the_pattern() {
    let original = pattern();
    let before = original.clone();
    for body in [
        "context.notes[1].velocity=100; context.controls[1].value=999",
        "context.notes={}",
        "context.controls={}",
        "context.notes=nil",
        "return true",
        "return {}",
        "return nil, 1",
        "context.notes[1].id=999",
        "context.notes[1].velocity=0",
        "table.remove(context.notes,1)",
        "context.notes[3]=context.notes[1]",
        "context.notes[1],context.notes[2]=context.notes[2],context.notes[1]",
        "context.notes[4]=context.notes[1]",
        "context.notes[0]=context.notes[1]",
        "context.controls[3]=context.controls[1]",
        "context.controls[1].id=999",
        "context.notes[1].cuts={[2]=100}",
        "context.notes[1].cuts={200,100}",
        "for i=2,65537 do context.controls[i]=context.controls[1] end",
    ] {
        let (_fixture, registry) = registry(body, "");
        let invocation = invoke(&registry, "midi.tool user.midi.accent");
        let error = invocation
            .edit_midi(&original, &[], MidiOptions::default())
            .err()
            .unwrap();
        assert_eq!(error.phase, PackagePhase::Result, "{body}: {error}");
        assert_eq!(error.id.as_deref(), Some("user.midi.accent"));
        assert!(error.field.is_some());
        assert_eq!(error.path, invocation.package().path());
        assert_eq!(original, before);
    }
}

#[test]
fn execution_errors_sandbox_and_budgets_leave_the_original_intact() {
    let original = pattern();
    let before = original.clone();
    for body in [
        "context.notes[1].velocity=100; error('fallo deliberado')",
        "require('io')",
        "io.open('file')",
        "load('return true')",
        "pcall(function() end)",
        "while true do end",
        "local huge=string.rep('x',16*1024*1024)",
    ] {
        let (_fixture, registry) = registry(body, "");
        let error = invoke(&registry, "midi.tool user.midi.accent")
            .edit_midi(&original, &[], MidiOptions::default())
            .err()
            .unwrap();
        assert_eq!(error.phase, PackagePhase::Execute, "{body}: {error}");
        assert_eq!(original, before);
    }
}

#[test]
fn every_invocation_gets_a_fresh_vm_and_uses_captured_source() {
    let (fixture, registry) = registry(
        "calls=(calls or 0)+1; context.notes[1].velocity=90+calls",
        "",
    );
    let invocation = invoke(&registry, "midi.tool user.midi.accent");
    fixture.write("tool.lua", "invalid changed file");
    for _ in 0..2 {
        let edit = invocation
            .edit_midi(&pattern(), &[], MidiOptions::default())
            .unwrap();
        assert_eq!(edit.pattern.notes()[0].velocity, 91);
    }
}
