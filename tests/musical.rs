//! Contratos temporales del prototipo; no requieren GPUI ni dispositivo de audio.
use elevate::musical::{
    model::{InstrumentId, Pattern},
    scheduler::{EventKind, PreparedPattern},
    time::{FrameWindow, PPQ, Tick, TimeBase, Transport},
};

#[test]
fn arbitrary_length_and_simultaneous_notes_survive_edits_with_stable_ids() {
    let mut pattern = Pattern::new(Tick(PPQ * 5)).unwrap();
    let kick = pattern
        .add_note(InstrumentId(1), Tick(0), Tick(37), 36, 100)
        .unwrap();
    let hat = pattern
        .add_note(InstrumentId(2), Tick(0), Tick(19), 42, 72)
        .unwrap();
    pattern
        .add_note(InstrumentId(1), Tick(4733), Tick(67), 36, 90)
        .unwrap();
    pattern.remove_note(kick).unwrap();
    pattern.set_velocity(hat, 120).unwrap();
    assert_eq!(pattern.notes()[0].id, hat);
    assert_eq!(pattern.notes()[0].velocity, 120);
    assert_eq!(pattern.notes()[1].start, Tick(4733));
    let replacement = pattern
        .add_note(InstrumentId(1), Tick(0), Tick(20), 36, 100)
        .unwrap();
    assert_ne!(kick, replacement);
}

#[test]
fn invalid_edits_do_not_mutate_the_pattern() {
    let mut pattern = Pattern::new(Tick(PPQ)).unwrap();
    let id = pattern
        .add_note(InstrumentId(1), Tick(0), Tick(PPQ), 36, 100)
        .unwrap();
    let original = pattern.notes().to_vec();
    for (start, duration, pitch, velocity) in [
        (-1, 1, 36, 100),
        (0, 0, 36, 100),
        (PPQ, 1, 36, 100),
        (0, 1, 128, 100),
        (0, 1, 36, 0),
        (i64::MAX, 1, 36, 100),
    ] {
        assert!(
            pattern
                .add_note(
                    InstrumentId(1),
                    Tick(start),
                    Tick(duration),
                    pitch,
                    velocity
                )
                .is_err()
        );
    }
    assert!(pattern.set_velocity(id, 0).is_err());
    assert_eq!(pattern.notes(), original);
    assert!(Pattern::new(Tick(0)).is_err());
    for bpm in [0.0, f64::NAN, f64::INFINITY, -120.0] {
        assert!(TimeBase::new(48000, bpm).is_err());
    }
    assert!(TimeBase::new(0, 120.0).is_err());
    assert!(
        TimeBase::new(48000, 120.0)
            .unwrap()
            .frame_at(Tick(-1))
            .is_err()
    );
}

#[test]
fn windows_do_not_duplicate_boundaries_and_release_before_retriggering() {
    let mut pattern = Pattern::new(Tick(PPQ * 2)).unwrap();
    let first = pattern
        .add_note(InstrumentId(1), Tick(0), Tick(PPQ), 36, 100)
        .unwrap();
    let second = pattern
        .add_note(InstrumentId(1), Tick(PPQ), Tick(PPQ), 36, 110)
        .unwrap();
    let plan = PreparedPattern::compile(&pattern, TimeBase::new(48000, 120.0).unwrap()).unwrap();
    let before: Vec<_> = plan
        .events_in(FrameWindow::new(0, 24000).unwrap())
        .collect();
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].note, first);
    let boundary: Vec<_> = plan
        .events_in(FrameWindow::new(24000, 1).unwrap())
        .collect();
    assert_eq!(boundary.len(), 2);
    assert_eq!(boundary[0].kind, EventKind::NoteOff);
    assert_eq!(boundary[1].note, second);
    assert!(boundary.iter().all(|event| event.sample_offset == 0));
    assert_eq!(
        plan.events_in(FrameWindow::new(24000, 0).unwrap()).count(),
        0
    );
    assert_eq!(
        plan.events_in(FrameWindow::new(48000, 1).unwrap())
            .next()
            .unwrap()
            .kind,
        EventKind::NoteOff
    );
}

#[test]
fn musical_output_is_independent_of_audio_buffer_partition() {
    for rate in [44100, 48000] {
        let clock = TimeBase::new(rate, 137.0).unwrap();
        let mut pattern = Pattern::new(Tick(PPQ * 17)).unwrap();
        for index in 0..41 {
            pattern
                .add_note(
                    InstrumentId(index % 4),
                    Tick(i64::from(index) * 379),
                    Tick(137),
                    36 + (index % 4) as u8,
                    100,
                )
                .unwrap();
        }
        let plan = PreparedPattern::compile(&pattern, clock).unwrap();
        let total = clock.frame_at(pattern.length()).unwrap() as u32 + 1;
        let expected: Vec<_> = plan
            .events_in(FrameWindow::new(0, total).unwrap())
            .map(|event| (u64::from(event.sample_offset), event.note, event.kind))
            .collect();
        for block_size in [1, 64, 257, 512, 4096] {
            let mut transport = Transport::default();
            transport.play();
            let mut actual = Vec::new();
            while transport.frame() < u64::from(total) {
                let frames = block_size.min((u64::from(total) - transport.frame()) as u32);
                let window = transport.advance(frames).unwrap().unwrap();
                actual.extend(plan.events_in(window).map(|event| {
                    (
                        window.start() + u64::from(event.sample_offset),
                        event.note,
                        event.kind,
                    )
                }));
            }
            assert_eq!(actual, expected, "rate={rate}, buffer={block_size}");
            transport.stop();
            let frame = transport.frame();
            assert!(transport.advance(512).unwrap().is_none());
            assert_eq!(transport.frame(), frame);
        }
    }
}

#[test]
fn transport_overflow_keeps_its_previous_position() {
    let mut transport = Transport::default();
    transport.seek(u64::MAX - 10);
    transport.play();
    assert!(transport.advance(11).is_err());
    assert_eq!(transport.frame(), u64::MAX - 10);
}
