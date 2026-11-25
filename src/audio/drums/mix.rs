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
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering},
};

/// Control lock-free para MPC Note Repeat gobernado por frames del motor RT (cero timers UI).
#[derive(Default)]
pub(crate) struct NoteRepeatControl {
    active: AtomicBool,
    piece: AtomicU32,
    pitch: AtomicU8,
    velocity: AtomicU8,
    interval_frames: AtomicU64,
    generation: AtomicU64,
    hit_counter: AtomicU64,
}

impl NoteRepeatControl {
    pub fn start(&self, piece: InstrumentId, pitch: u8, velocity: u8, interval_frames: u64) {
        self.piece.store(piece.0, Ordering::Relaxed);
        self.pitch.store(pitch, Ordering::Relaxed);
        self.velocity.store(velocity, Ordering::Relaxed);
        self.interval_frames
            .store(interval_frames.max(64), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.active.store(true, Ordering::Release);
    }

    pub fn update_interval(&self, interval_frames: u64) {
        self.interval_frames
            .store(interval_frames.max(64), Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn hits(&self) -> u64 {
        self.hit_counter.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(super) struct NoteRepeatClock {
    last_gen: u64,
    until_next: u64,
}

impl NoteRepeatClock {
    /// Avanza el reloj de Note Repeat `frames` muestras y devuelve los offsets dentro del bloque
    /// donde debe dispararse un golpe repetido (máximo 8 por bloque de 256).
    pub fn collect(
        &mut self,
        ctrl: &NoteRepeatControl,
        frames: usize,
        out: &mut [(usize, DrumPreview); 8],
    ) -> usize {
        if !ctrl.active.load(Ordering::Acquire) {
            self.until_next = 0;
            return 0;
        }
        let generation = ctrl.generation.load(Ordering::Acquire);
        let interval = ctrl.interval_frames.load(Ordering::Relaxed).max(64);
        if generation != self.last_gen {
            self.last_gen = generation;
            // El primer golpe ya se disparó inmediatamente al pulsar; el siguiente llega tras `interval`.
            self.until_next = interval;
        }
        let piece = InstrumentId(ctrl.piece.load(Ordering::Relaxed));
        let pitch = ctrl.pitch.load(Ordering::Relaxed);
        let velocity = ctrl.velocity.load(Ordering::Relaxed);
        let mut count = 0usize;
        let mut remaining = frames as u64;
        let mut cursor = 0usize;
        while remaining > 0 {
            if self.until_next == 0 {
                self.until_next = interval;
            }
            if self.until_next > remaining {
                self.until_next -= remaining;
                break;
            }
            cursor += self.until_next as usize;
            remaining -= self.until_next;
            self.until_next = interval;
            if count < out.len() && cursor < frames {
                out[count] = (
                    cursor,
                    DrumPreview {
                        piece,
                        pitch,
                        velocity,
                    },
                );
                count += 1;
                ctrl.hit_counter.fetch_add(1, Ordering::Relaxed);
            }
        }
        count
    }
}

#[derive(Clone)]
pub(crate) struct DrumInput {
    pub score: Option<DrumScore>,
    pub kit: DrumKit,
    pub note_repeat: Option<Arc<NoteRepeatControl>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DrumScore {
    pub pattern: Pattern,
    pub repeat: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct DrumPreview {
    pub piece: InstrumentId,
    pub pitch: u8,
    pub velocity: u8,
}

pub(crate) struct PreparedDrums {
    kit: PreparedKit,
    handoff: VoiceHandoff,
    previews: Consumer<DrumPreview>,
    note_repeat: Arc<NoteRepeatControl>,
    repeat_clock: NoteRepeatClock,
    last_end: Option<u64>,
    previous_mode: Option<bool>,
}

impl PreparedDrums {
    pub fn prepare(
        input: DrumInput,
        rate: u32,
        bpm: f64,
    ) -> Result<(Self, LiveDrums, Producer<DrumPreview>), String> {
        let note_repeat = input.note_repeat.unwrap_or_default();
        let kit = PreparedKit::prepare(&input.kit, &Default::default(), rate)?;
        let (live, handoff) =
            LiveDrums::prepare(input.kit, input.score, kit.assets.clone(), rate, bpm)?;
        let (preview_tx, previews) = RingBuffer::new(32);
        Ok((
            Self {
                kit,
                handoff,
                previews,
                note_repeat,
                repeat_clock: NoteRepeatClock::default(),
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

    pub fn finish_input(&mut self) {
        self.kit.finish_input();
    }

    pub fn render_block(
        &mut self,
        at: u64,
        output: &mut [StereoFrame],
        state: &DrumBlock,
        tail: bool,
        taps: Option<&mut crate::audio::playback::StemBuffers>,
    ) {
        self.handoff.install(&mut self.kit, state.voice_revision);
        let mode = state.plan.as_ref().map(|plan| plan.repeat);
        if !tail && (self.last_end.is_some_and(|end| end != at) || self.previous_mode != mode) {
            self.kit.cut();
        }
        self.kit.apply_controls(state);
        for _ in 0..if tail { 0 } else { 32 } {
            let Ok(note) = self.previews.pop() else {
                break;
            };
            self.kit
                .trigger(state, note.piece, note.pitch, note.velocity, 0);
        }
        let mut repeat_hits = [(
            0usize,
            DrumPreview {
                piece: InstrumentId(0),
                pitch: 36,
                velocity: 100,
            },
        ); 8];
        let repeat_count = if tail {
            0
        } else {
            self.repeat_clock
                .collect(&self.note_repeat, output.len(), &mut repeat_hits)
        };
        if repeat_count > 0 && state.plan.is_none() {
            let mut cursor = 0usize;
            for &(pos, note) in &repeat_hits[..repeat_count] {
                self.kit.render_segment(cursor, pos);
                cursor = pos;
                self.kit
                    .trigger(state, note.piece, note.pitch, note.velocity, 0);
            }
            self.kit.render_segment(cursor, output.len());
        } else {
            for &(_, note) in &repeat_hits[..repeat_count] {
                self.kit
                    .trigger(state, note.piece, note.pitch, note.velocity, 0);
            }
            match state.plan.as_ref().filter(|_| !tail) {
                Some(plan) if plan.repeat => {
                    self.render_cycle(&plan.pattern, at, output.len(), state);
                }
                Some(plan) => self.render_window(&plan.pattern, at, output.len(), 0, 0, state),
                None => self.kit.render_segment(0, output.len()),
            }
        }
        self.kit.sum(state, output, taps);
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
