//! Percusión en el reloj y la salida comunes. Las notas usan offsets del scheduler.
use super::{
    controls::DrumBlock,
    kit::PreparedKit,
    live::{LiveDrums, VoiceHandoff},
};
use elevate::{
    StereoFrame,
    musical::{
        drums::DrumKit,
        model::{InstrumentId, Pattern},
        scheduler::EventKind,
        time::FrameWindow,
    },
};
use rtrb::{Consumer, Producer, RingBuffer};

pub(crate) struct DrumInput {
    pub pattern: Option<Pattern>,
    pub kit: DrumKit,
}

pub(crate) struct DrumPreview {
    pub piece: InstrumentId,
    pub pitch: u8,
    pub velocity: u8,
}

pub(crate) struct PreparedDrums {
    kit: PreparedKit,
    handoff: VoiceHandoff,
    previews: Consumer<DrumPreview>,
    last_end: Option<u64>,
    was_playing: bool,
}

impl PreparedDrums {
    pub fn prepare(
        input: DrumInput,
        rate: u32,
        bpm: f64,
    ) -> Result<(Self, LiveDrums, Producer<DrumPreview>), String> {
        let kit = PreparedKit::prepare(&input.kit, &Default::default(), rate)?;
        let (live, handoff) =
            LiveDrums::prepare(input.kit, input.pattern, kit.assets.clone(), rate, bpm)?;
        let (preview_tx, previews) = RingBuffer::new(32);
        Ok((
            Self {
                kit,
                handoff,
                previews,
                last_end: None,
                was_playing: false,
            },
            live,
            preview_tx,
        ))
    }

    pub fn render(&mut self, at: u64, output: &mut [StereoFrame], state: &DrumBlock) {
        self.handoff.install(&mut self.kit, state.voice_revision);
        if self.last_end.is_some_and(|end| end != at) || (self.was_playing && !state.loops()) {
            self.kit.cut();
        }
        self.kit.apply_controls(state);
        for _ in 0..32 {
            let Ok(note) = self.previews.pop() else {
                break;
            };
            self.kit
                .trigger(state, note.piece, note.pitch, note.velocity, 0);
        }
        if let Some(plan) = &state.plan {
            let length = plan.length_frames();
            let mut written = 0;
            while written < output.len() {
                let frame = at.saturating_add(written as u64);
                let local = frame % length;
                let count = (output.len() - written) as u64;
                let count = count.min(length - local) as usize;
                let window =
                    FrameWindow::new(local, count as u32).expect("tramo de batería positivo");
                let mut cursor = written;
                // Un NoteOff en el final de la vuelta precede a los NoteOn de la siguiente.
                if local == 0
                    && frame > 0
                    && let Ok(boundary) = FrameWindow::new(length, 1)
                {
                    for event in plan.events_in(boundary) {
                        if event.kind == EventKind::NoteOff {
                            self.kit.note_off(
                                event.instrument,
                                event.pitch,
                                event.expression.release_velocity,
                            );
                        }
                    }
                }
                for event in plan.events_in(window) {
                    let end = written + event.sample_offset as usize;
                    match event.kind {
                        EventKind::NoteOn { velocity }
                            if event.expression.plays(event.note, frame / length) =>
                        {
                            self.kit.render_segment(cursor, end);
                            cursor = end;
                            self.kit.trigger(
                                state,
                                event.instrument,
                                event.pitch,
                                velocity,
                                event.note.0,
                            );
                        }
                        EventKind::NoteOff => {
                            self.kit.render_segment(cursor, end);
                            cursor = end;
                            self.kit.note_off(
                                event.instrument,
                                event.pitch,
                                event.expression.release_velocity,
                            );
                        }
                        _ => {}
                    }
                }
                self.kit.render_segment(cursor, written + count);
                written += count;
            }
        } else {
            self.kit.render_segment(0, output.len());
        }
        self.kit.sum(state, output);
        self.last_end = Some(at.saturating_add(output.len() as u64));
        self.was_playing = state.loops();
    }
}
