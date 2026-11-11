use super::{DragKind, Timeline};
use crate::audio::{AudioClip, AudioFile};
use elevate::musical::{
    clip::MidiClip,
    model::{InstrumentId, Pattern},
    time::{PPQ, Tick},
};
use std::sync::Arc;

fn timeline_with_clips() -> Timeline {
    let mut timeline = Timeline::new(32.0);
    let mut pattern = Pattern::new(Tick(8 * PPQ)).unwrap();
    pattern
        .add_note(InstrumentId(1), Tick(2 * PPQ), Tick(4 * PPQ), 60, 100)
        .unwrap();
    let mut midi = MidiClip::new("MIDI", 0, 4.0, pattern);
    midi.trim_start = 1.0;
    midi.trim_len = Some(6.0);
    timeline.insert_midi_clip(midi);
    timeline.insert_audio_clip(AudioClip {
        name: "Audio".into(),
        file: Arc::new(AudioFile {
            name: "source.wav".into(),
            sample_rate: 100,
            frames: vec![[0.25, -0.25]; 400],
            peaks: vec![[0.0, 0.25]],
            max_peak: 0.25,
        }),
        track: 0,
        start_beat: 4.0,
        props: Default::default(),
        trim_start: 0.5,
        trim_len: Some(3.0),
    });
    timeline
}

#[test]
fn split_trimmed_audio_and_midi_preserves_source_alignment_and_total_length() {
    let mut timeline = timeline_with_clips();
    let source = timeline.clip.as_ref().unwrap().file.clone();
    let notes = timeline.midi_clip.as_ref().unwrap().notes().to_vec();
    for kind in [DragKind::Midi, DragKind::Audio] {
        assert!(timeline.split_clip(kind, 7.0));
        assert_eq!(timeline.clip_extent(kind), Some((7.0, 10.0, 4.0, true)));
    }
    let left = &timeline.other_midi_clips[0];
    let right = timeline.midi_clip.as_ref().unwrap();
    assert_eq!(left.end_beat(), right.start_beat);
    assert_eq!(left.duration_beats() + right.duration_beats(), 6.0);
    assert_eq!(left.notes(), notes);
    assert_eq!(right.notes(), notes);
    // La nota que cruza el corte se conserva en las dos ventanas de reproducción.
    assert_eq!(left.playback_pattern().notes()[0].duration, Tick(2 * PPQ));
    assert_eq!(right.playback_pattern().notes()[0].start, Tick(0));
    assert_eq!(right.playback_pattern().notes()[0].duration, Tick(2 * PPQ));
    let left = &timeline.other_audio_clips[0];
    let right = timeline.clip.as_ref().unwrap();
    assert_eq!(left.end_beat(elevate::musical::time::DEFAULT_BPM), right.start_beat);
    assert_eq!(left.length_seconds() + right.length_seconds(), 3.0);
    assert!(Arc::ptr_eq(&left.file, &source));
    assert!(Arc::ptr_eq(&right.file, &source));
}

#[test]
fn split_rejects_boundaries_invalid_positions_and_locked_clips() {
    for kind in [DragKind::Midi, DragKind::Audio] {
        let mut timeline = timeline_with_clips();
        for beat in [f64::NAN, f64::INFINITY, -1.0, 4.0, 4.1, 9.9, 10.0] {
            assert!(!timeline.split_clip(kind, beat));
        }
        timeline.midi_clip.as_mut().unwrap().props.locked = true;
        timeline.clip.as_mut().unwrap().props.locked = true;
        assert!(!timeline.split_clip(kind, 7.0));
        timeline.restore_clip_trim(kind);
        assert_eq!(timeline.clip_extent(kind), Some((4.0, 10.0, 1.0, true)));
        assert!(timeline.other_audio_clips.is_empty());
        assert!(timeline.other_midi_clips.is_empty());
    }
}

#[test]
fn restoring_trim_preserves_alignment_and_never_moves_before_zero() {
    for kind in [DragKind::Midi, DragKind::Audio] {
        let mut timeline = timeline_with_clips();
        timeline.restore_clip_trim(kind);
        assert_eq!(timeline.clip_extent(kind), Some((3.0, 11.0, 0.0, false)));

        let mut timeline = timeline_with_clips();
        timeline.midi_clip.as_mut().unwrap().start_beat = 0.5;
        timeline.clip.as_mut().unwrap().start_beat = 0.5;
        timeline.restore_clip_trim(kind);
        assert_eq!(timeline.clip_extent(kind), Some((0.0, 7.5, 0.5, true)));
    }
}
