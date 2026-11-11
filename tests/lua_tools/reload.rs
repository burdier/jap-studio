use super::{
    defaults,
    fixtures::{NUMBER, altered},
    invoke, pattern, registry, source,
};
use elevate::{
    musical::clip::MidiClip,
    scripting::{MidiScriptTarget, actions::MidiOptions, packages::PackagePhase},
};
use std::{fs, sync::mpsc, thread};

#[test]
fn failed_reload_keeps_forms_valid_and_successful_reload_invalidates_them() {
    let (fixture, mut registry) = registry("context.notes[1].velocity=params.amount", NUMBER);
    let values = defaults(&registry);
    let form = registry.form("user.midi.accent").unwrap();
    fixture.write("tool.lua", altered("p.api_version=2"));
    assert!(registry.reload(&fixture.directories).is_err());
    assert!(form.submit(&registry, &values).is_ok());
    fixture.write("tool.lua", source("context.notes[1].velocity=120", NUMBER));
    registry.reload(&fixture.directories).unwrap();
    assert_eq!(
        form.submit(&registry, &values).unwrap_err().phase,
        PackagePhase::Stale
    );
    let current = registry.form("user.midi.accent").unwrap();
    let edit = current
        .submit(&registry, &values)
        .unwrap()
        .edit_midi(&pattern(), &[], MidiOptions::default())
        .unwrap();
    assert_eq!(edit.pattern.notes()[0].velocity, 120);
    fs::remove_file(fixture.directories.user.as_ref().unwrap().join("tool.lua")).unwrap();
    registry.reload(&fixture.directories).unwrap();
    assert_eq!(
        current.submit(&registry, &values).unwrap_err().phase,
        PackagePhase::Stale
    );
}

#[test]
fn submitted_work_finishes_with_its_version_after_the_package_is_removed() {
    let (fixture, mut registry) = registry("context.notes[1].velocity=params.amount", NUMBER);
    let invocation = invoke(&registry, "midi.tool user.midi.accent 111");
    let (release, wait) = mpsc::channel();
    let work = thread::spawn(move || {
        wait.recv().unwrap();
        invocation
            .edit_midi(&pattern(), &[], MidiOptions::default())
            .unwrap()
            .pattern
    });
    fs::remove_file(fixture.directories.user.as_ref().unwrap().join("tool.lua")).unwrap();
    registry.reload(&fixture.directories).unwrap();
    assert_eq!(registry.packages().len(), 0);
    assert!(
        registry
            .request("midi.tool user.midi.accent 111")
            .unwrap()
            .is_err()
    );
    release.send(()).unwrap();
    assert_eq!(work.join().unwrap().notes()[0].velocity, 111);
}

#[test]
fn concurrent_clip_changes_lock_and_drag_reject_the_pending_result() {
    let original = MidiClip::new("Original", 0, 0.0, pattern());
    let target = MidiScriptTarget::capture(&original);
    assert!(target.validate(Some(&original), false).is_ok());
    assert!(target.validate(None, false).is_err());
    assert!(target.validate(Some(&original), true).is_err());
    let other = MidiClip::new("Otro", 0, 0.0, original.pattern.clone());
    assert!(target.validate(Some(&other), false).is_err());
    let mut locked = original.clone();
    locked.props.locked = true;
    assert!(target.validate(Some(&locked), false).is_err());
    let mut changed = original.clone();
    changed.pattern = elevate::scripting::edit_midi(
        "notes[1].velocity=110",
        &changed.pattern,
        &[],
        MidiOptions::default(),
    )
    .unwrap()
    .pattern;
    assert!(target.validate(Some(&changed), false).is_err());
    assert_eq!(original.pattern, pattern());
}

#[test]
fn validated_cuts_preserve_current_selection_mutes_and_clip_properties() {
    let (_fixture, registry) = registry("context.notes[1].cuts={240,480}", "");
    let mut clip = MidiClip::new("Toma", 2, 8.0, pattern());
    clip.trim_start = 0.25;
    clip.trim_len = Some(2.0);
    let target = MidiScriptTarget::capture(&clip);
    let id = clip.notes()[0].id;
    let edit = invoke(&registry, "midi.tool user.midi.accent")
        .edit_midi(&clip.pattern, &[id], MidiOptions::default())
        .unwrap();
    // Mute y selección pueden cambiar mientras corre el worker, sin invalidar el patrón.
    clip.muted_notes.insert(id);
    let mut selected = vec![id];
    target.validate(Some(&clip), false).unwrap();
    edit.apply(&mut clip, &mut selected);
    assert_eq!(selected.len(), 3);
    assert!(selected.iter().all(|id| clip.muted_notes.contains(id)));
    assert_eq!(clip.notes().len(), 4);
    assert_eq!(clip.notes()[0].id, id);
    assert_eq!(clip.pattern.controls(), pattern().controls());
    assert_eq!(
        (clip.start_beat, clip.trim_start, clip.trim_len),
        (8.0, 0.25, Some(2.0))
    );
}
