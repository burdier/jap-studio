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
        scheduler::{EventKind, PreparedPattern},
        time::FrameWindow,
    },
};
use rtrb::{Consumer, Producer, RingBuffer};

pub(crate) struct DrumInput {
    pub score: Option<DrumScore>,
    pub kit: DrumKit,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DrumScore {
    pub pattern: Pattern,
    pub repeat: bool,
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
    previous_mode: Option<bool>,
}

impl PreparedDrums {
    pub fn prepare(
        input: DrumInput,
        rate: u32,
        bpm: f64,
    ) -> Result<(Self, LiveDrums, Producer<DrumPreview>), String> {
        let kit = PreparedKit::prepare(&input.kit, &Default::default(), rate)?;
        let (live, handoff) =
            LiveDrums::prepare(input.kit, input.score, kit.assets.clone(), rate, bpm)?;
        let (preview_tx, previews) = RingBuffer::new(32);
        Ok((
            Self {
                kit,
                handoff,
                previews,
                last_end: None,
                previous_mode: None,
            },
            live,
            preview_tx,
        ))
    }

    pub fn has_one_shot_tail(&self) -> bool {
        self.kit.has_one_shot_tail()
    }

    pub fn render(&mut self, at: u64, output: &mut [StereoFrame], state: &DrumBlock) {
        self.handoff.install(&mut self.kit, state.voice_revision);
        let mode = state.plan.as_ref().map(|plan| plan.repeat);
        if self.last_end.is_some_and(|end| end != at) || self.previous_mode != mode {
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
        match &state.plan {
            Some(plan) if plan.repeat => {
                self.render_cycle(&plan.pattern, at, output.len(), state);
            }
            Some(plan) => self.render_window(&plan.pattern, at, output.len(), 0, 0, state),
            None => self.kit.render_segment(0, output.len()),
        }
        self.kit.sum(state, output);
        self.last_end = Some(at.saturating_add(output.len() as u64));
        self.previous_mode = mode;
    }

    fn render_cycle(&mut self, plan: &PreparedPattern, at: u64, frames: usize, state: &DrumBlock) {
        let length = plan.length_frames();
        let mut written = 0;
        while written < frames {
            let frame = at.saturating_add(written as u64);
            let local = frame % length;
            let count = ((frames - written) as u64).min(length - local) as usize;
            // Entrega NoteOff al cerrar una vuelta antes de los NoteOn de la siguiente.
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
            self.render_window(plan, local, count, written, frame / length, state);
            written += count;
        }
    }

    fn render_window(
        &mut self,
        plan: &PreparedPattern,
        start: u64,
        count: usize,
        offset: usize,
        cycle: u64,
        state: &DrumBlock,
    ) {
        let mut cursor = offset;
        if let Ok(window) = FrameWindow::new(start, count as u32) {
            for event in plan.events_in(window) {
                let end = offset + event.sample_offset as usize;
                match event.kind {
                    EventKind::NoteOn { velocity } if event.expression.plays(event.note, cycle) => {
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
        }
        self.kit.render_segment(cursor, offset + count);
    }
}
