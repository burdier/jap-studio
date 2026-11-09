//! Contratos de consolidación MIDI: sin UI ni dispositivo.
use elevate::musical::{
    clip::MidiClip,
    materialize::materialize,
    model::{InstrumentId, Pattern},
    time::{PPQ, Tick},
};

fn clip(start: f64) -> MidiClip {
    let mut pattern = Pattern::new(Tick(4 * PPQ)).unwrap();
    pattern
        .add_note(InstrumentId(1), Tick(0), Tick(2 * PPQ), 60, 96)
        .unwrap();
    pattern
        .add_note(InstrumentId(1), Tick(3 * PPQ), Tick(PPQ), 67, 80)
        .unwrap();
    MidiClip::new("MIDI", 0, start, pattern)
}

#[test]
fn single_clip_commits_visible_window_and_leaves_original_untouched() {
    let mut source = clip(8.0);
    source.trim_start = 1.0;
    source.trim_len = Some(2.0);
    source.props.gain_db = -3.0;
    let result = materialize(&[&source]).unwrap();
    assert_ne!(source.id, result.id);
    assert_eq!(result.start_beat, 8.0);
    assert_eq!(result.pattern.length(), Tick(2 * PPQ));
    assert_eq!(result.trim_start, 0.0);
    assert_eq!(result.trim_len, None);
    assert_eq!(result.props, source.props);
    assert_eq!(result.notes().len(), 1);
    assert_eq!(result.notes()[0].start, Tick(0));
    assert_eq!(result.notes()[0].duration, Tick(PPQ));
    assert_eq!(source.notes().len(), 2);
    assert_eq!(source.pattern.length(), Tick(4 * PPQ));
}

#[test]
fn joins_in_time_order_with_gaps_adjacency_or_overlaps_and_fresh_note_ids() {
    for second_start in [6.0, 8.0, 12.0] {
        let first = clip(4.0);
        let second = clip(second_start);
        let result = materialize(&[&second, &first]).unwrap();
        assert_eq!(result.start_beat, 4.0);
        assert_eq!(result.duration_beats(), second_start);
        assert_eq!(result.notes().len(), 4);
        let mut expected: Vec<_> = [first.clone(), second.clone()]
            .iter()
            .flat_map(|c| {
                c.notes()
                    .iter()
                    .map(|n| {
                        (
                            ((c.start_beat - 4.0) * PPQ as f64) as i64 + n.start.0,
                            n.duration.0,
                            n.pitch,
                            n.velocity,
                            n.instrument,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        expected.sort();
        let mut actual: Vec<_> = result
            .notes()
            .iter()
            .map(|n| (n.start.0, n.duration.0, n.pitch, n.velocity, n.instrument))
            .collect();
        actual.sort();
        assert_eq!(actual, expected);
        let ids: std::collections::HashSet<_> = result.notes().iter().map(|n| n.id).collect();
        assert_eq!(ids.len(), 4);
    }
}

#[test]
fn incompatible_locked_or_invalid_sources_are_rejected() {
    let first = clip(0.0);
    let mut second = clip(4.0);
    second.track = 1;
    assert!(materialize(&[&first, &second]).is_err());
    second.track = 0;
    second.props.locked = true;
    assert!(materialize(&[&first, &second]).is_err());
    second.props.locked = false;
    second.props.gain_db = -6.0;
    assert!(materialize(&[&first, &second]).is_err());
    second.props = first.props;
    second.start_beat = f64::INFINITY;
    assert!(materialize(&[&first, &second]).is_err());
    assert!(materialize(&[]).is_err());
}

#[test]
fn snapshots_keep_identity_but_editable_duplicates_do_not() {
    let original = clip(0.0);
    assert_eq!(original.id, original.clone().id);
    assert_ne!(original.id, original.duplicate().id);
}
