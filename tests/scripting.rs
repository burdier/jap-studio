use elevate::{
    musical::{clip::MidiClip, grid::NoteGrid, model::{InstrumentId, NoteId, Pattern}, time::Tick},
    scripting::{actions::{MidiOptions, MidiTool}, edit_midi},
};

fn fixture() -> (Pattern, NoteId, NoteId) {
    let mut pattern = Pattern::new(Tick(3840)).unwrap();
    let first = pattern.add_note(InstrumentId(7), Tick(100), Tick(1000), 60, 90).unwrap();
    let second = pattern.add_note(InstrumentId(8), Tick(1200), Tick(480), 64, 110).unwrap();
    (pattern, first, second)
}

fn options(denominator: u16) -> MidiOptions {
    MidiOptions { grid: NoteGrid::new(denominator).unwrap(), amount: 50, ..MidiOptions::default() }
}

#[test]
fn subdivide_preserves_coverage_off_grid_tails_mute_and_selection() {
    let (pattern, first, second) = fixture();
    let original_next = pattern.next_id();
    let untouched = *pattern.notes().iter().find(|n| n.id == second).unwrap();
    let edit = edit_midi(MidiTool::Subdivide.source(), &pattern, &[first], options(16)).unwrap();
    let mut clip = MidiClip::new("roll", 2, 8.0, pattern);
    clip.muted_notes.insert(first);
    clip.trim_start = 0.5;
    clip.trim_len = Some(2.0);
    let mut selected = vec![first];
    edit.apply(&mut clip, &mut selected);
    let roll: Vec<_> = clip.notes().iter().filter(|n| n.instrument == InstrumentId(7)).collect();
    assert_eq!(roll.iter().map(|n| (n.start.0, n.duration.0)).collect::<Vec<_>>(),
        vec![(100, 140), (240, 240), (480, 240), (720, 240), (960, 140)]);
    assert_eq!(roll[0].id, first);
    assert!(roll[1..].iter().all(|n| n.id.0 >= original_next));
    assert_eq!(clip.pattern.next_id(), original_next + 4);
    assert!(roll.iter().all(|n| n.pitch == 60 && n.velocity == 90 && selected.contains(&n.id) && clip.muted_notes.contains(&n.id)));
    assert_eq!(clip.notes().iter().find(|n| n.id == second), Some(&untouched));
    assert_eq!((clip.start_beat, clip.trim_start, clip.trim_len), (8.0, 0.5, Some(2.0)));

    // Más eventos del mismo gesto no producen cortes ni IDs adicionales.
    let repeated = edit_midi(MidiTool::Subdivide.source(), &clip.pattern, &selected, options(16)).unwrap();
    assert_eq!(repeated.pattern.notes(), clip.pattern.notes());
    assert_eq!(repeated.pattern.next_id(), clip.pattern.next_id());
}

#[test]
fn grid_change_alone_is_non_destructive_and_short_notes_are_unchanged() {
    let (pattern, first, _) = fixture();
    let before = pattern.notes().to_vec();
    let coarse = NoteGrid::new(8).unwrap();
    assert_eq!(coarse.finer().denominator(), 16);
    assert_eq!(coarse.finer().ticks(), Tick(240));
    assert_eq!(coarse.finer().coarser(), coarse);
    assert_eq!(NoteGrid::new(1).unwrap().coarser().denominator(), 1);
    assert_eq!(NoteGrid::new(128).unwrap().finer().denominator(), 128);
    assert!(NoteGrid::new(3).is_err());
    let edit = edit_midi(MidiTool::Subdivide.source(), &pattern, &[first], options(1)).unwrap();
    assert_eq!(edit.pattern.notes(), before);
    assert_eq!(pattern.notes(), before);
}

#[test]
fn quantize_uses_captured_resolution_and_preserves_unselected_notes() {
    let (pattern, first, second) = fixture();
    let edit = edit_midi(MidiTool::Quantize.source(), &pattern, &[second], options(8)).unwrap();
    assert_eq!(edit.pattern.notes().iter().find(|n| n.id == first).unwrap().start, Tick(100));
    let moved = edit.pattern.notes().iter().find(|n| n.id == second).unwrap();
    assert_eq!((moved.start, moved.duration), (Tick(1440), Tick(480)));
    assert_eq!(edit.pattern.next_id(), pattern.next_id());
    let all = edit_midi(MidiTool::Quantize.source(), &pattern, &[], options(16)).unwrap();
    assert_eq!(all.pattern.notes()[0].start, Tick(0));
}

#[test]
fn humanize_is_repeatable_bounded_and_zero_amount_is_identity() {
    let (pattern, first, _) = fixture();
    let tool = MidiTool::Humanize;
    let a = edit_midi(tool.source(), &pattern, &[first], options(16)).unwrap();
    let b = edit_midi(tool.source(), &pattern, &[first], options(16)).unwrap();
    assert_eq!(a.pattern.notes(), b.pattern.notes());
    assert_ne!(a.pattern.notes()[0], pattern.notes()[0]);
    assert!((a.pattern.notes()[0].start.0 - 100).abs() <= 6);
    assert!((i16::from(a.pattern.notes()[0].velocity) - 90).abs() <= 4);
    assert_eq!(a.pattern.notes()[1], pattern.notes()[1]);
    let zero = MidiOptions { amount: 0, ..options(16) };
    assert_eq!(edit_midi(tool.source(), &pattern, &[], zero).unwrap().pattern.notes(), pattern.notes());
}

#[test]
fn invalid_lua_results_leave_original_intact_and_expansion_is_bounded() {
    let (pattern, first, _) = fixture();
    let before = pattern.notes().to_vec();
    for source in [
        "notes[1].cuts = {100}",
        "notes[1].cuts = {1100}",
        "notes[1].cuts = {300, 200}",
        "notes[1].cuts = {200, 200}",
        "notes[1].cuts = {'invalid'}",
        "notes[1].id = 999",
        "notes[1].velocity = 0",
        "table.remove(notes, 1)",
        "notes[1].duration = -1",
        "notes[1].cuts = {}; for i=1,16384 do notes[1].cuts[i]=i end",
    ] {
        assert!(edit_midi(source, &pattern, &[first], options(16)).is_err(), "{source}");
        assert_eq!(pattern.notes(), before);
    }
}

#[test]
fn lua_budget_errors_and_missing_fields_cannot_bypass_validation() {
    let (pattern, _, _) = fixture();
    assert!(edit_midi("while true do end", &pattern, &[], options(16)).is_err());
    assert!(edit_midi("local a = string.rep('x', 16*1024*1024)", &pattern, &[], options(16)).is_err());
    assert!(edit_midi(&" ".repeat(256 * 1024 + 1), &pattern, &[], options(16)).is_err());
    // Los resultados se leen en crudo: un __index no puede fabricar una nota válida.
    assert!(edit_midi("notes[1].pitch=nil; setmetatable(notes[1], {__index=function() return 60 end})", &pattern, &[], options(16)).is_err());
}

#[test]
fn named_actions_share_parameter_contracts_and_reject_invalid_grid() {
    for (command, argument) in [("midi.quantize", "8"), ("midi.subdivide", "32"), ("midi.humanize", "75")] {
        let tool = MidiTool::from_command(command).unwrap();
        assert!(tool.options(&[argument.into()], options(16)).is_ok());
        assert_eq!(elevate::actions::find(command).unwrap().command, command);
        assert!(elevate::catalog::entries().iter().any(|entry|
            matches!(entry.action, elevate::catalog::CatalogAction::Invoke(value) if value == command)));
    }
    assert!(MidiTool::Subdivide.options(&["3".into()], options(16)).is_err());
    assert!(MidiTool::Humanize.options(&["101".into()], options(16)).is_err());
}
