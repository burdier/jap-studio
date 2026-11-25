//! Plan inmutable preparado fuera del callback; consulta sin asignaciones por bloque.
//! Una vuelta a tempo constante; el reproductor segmenta bloques en los límites del loop.
use super::{
    clip::ClipId,
    model::{InstrumentId, NoteId, Pattern},
    time::{FrameWindow, Tick, TimeBase, TimeError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    NoteOff,
    Control {
        lane: super::automation::ControlLane,
        value: i16,
    },
    NoteOn {
        velocity: u8,
    },
    PitchRamp {
        from_cents: i16,
        to_cents: i16,
        frames: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub sample_offset: u32,
    pub note: NoteId,
    pub instrument: InstrumentId,
    pub pitch: u8,
    pub clip: Option<ClipId>,
    pub initial_pitch_cents: Option<i16>,
    pub kind: EventKind,
    pub expression: super::note_expression::NoteExpression,
}

#[derive(Debug, Clone, Copy)]
struct PreparedEvent {
    frame: u64,
    event: ScheduledEvent,
}

pub struct PreparedPattern {
    events: Vec<PreparedEvent>,
    time_base: TimeBase,
    length_frames: u64,
}

impl PreparedPattern {
    /// Los eventos se cuantizan una vez a frames. Duración mínima: un frame.
    /// Recompilar fuera del hilo de audio si cambian notas, BPM o sample rate.
    pub fn compile(pattern: &Pattern, time_base: TimeBase) -> Result<Self, TimeError> {
        let length_frames = time_base.frame_at(pattern.length())?.max(1);
        let mut events = Vec::with_capacity(pattern.notes().len() * 2);
        for note in pattern.notes() {
            let start = time_base.frame_at(note.start)?;
            let end = time_base
                .frame_at(Tick(note.start.0 + note.duration.0))?
                .max(start + 1);
            let event = ScheduledEvent {
                sample_offset: 0,
                note: note.id,
                instrument: note.instrument,
                pitch: note.pitch,
                clip: None,
                initial_pitch_cents: note.pitch_curve.points().first().map(|point| point.cents),
                expression: note.expression,
                kind: EventKind::NoteOn {
                    velocity: note.velocity,
                },
            };
            events.push(PreparedEvent {
                frame: start,
                event,
            });
            events.push(PreparedEvent {
                frame: end,
                event: ScheduledEvent {
                    kind: EventKind::NoteOff,
                    ..event
                },
            });
            let mut anchors = Vec::<(u64, i16)>::new();
            for point in note.pitch_curve.points() {
                let fraction = u128::from(point.at);
                let offset = (u128::from(end - start) * fraction + u128::from(u16::MAX / 2))
                    / u128::from(u16::MAX);
                let frame = start + offset as u64;
                if let Some(last) = anchors.last_mut().filter(|last| last.0 == frame) {
                    if frame != start {
                        last.1 = point.cents;
                    }
                } else {
                    anchors.push((frame, point.cents));
                }
            }
            for pair in anchors.windows(2) {
                let (frame, from_cents) = pair[0];
                let (target, to_cents) = pair[1];
                if from_cents != to_cents {
                    events.push(PreparedEvent {
                        frame,
                        event: ScheduledEvent {
                            kind: EventKind::PitchRamp {
                                from_cents,
                                to_cents,
                                frames: target - frame,
                            },
                            ..event
                        },
                    });
                }
            }
        }
        for point in pattern.controls().points() {
            events.push(PreparedEvent {
                frame: time_base.frame_at(point.at)?,
                event: ScheduledEvent {
                    sample_offset: 0,
                    note: NoteId(0),
                    instrument: InstrumentId(0),
                    pitch: 0,
                    clip: None,
                    initial_pitch_cents: None,
                    expression: Default::default(),
                    kind: EventKind::Control {
                        lane: point.lane,
                        value: point.value,
                    },
                },
            });
        }
        // Off antes de Control y On en un retrigger. Las rampas empiezan tras su On.
        events.sort_by_key(|item| (item.frame, item.event.kind, item.event.note));
        Ok(Self {
            events,
            time_base,
            length_frames,
        })
    }
    pub fn time_base(&self) -> TimeBase {
        self.time_base
    }
    /// Duración de una vuelta. Nunca es cero, incluso con patrones de un tick.
    pub fn length_frames(&self) -> u64 {
        self.length_frames
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Iteración durante la preparación, antes de publicar el plan al audio.
    pub fn all_events(&self) -> impl Iterator<Item = (u64, ScheduledEvent)> + '_ {
        self.events.iter().map(|item| (item.frame, item.event))
    }

    /// Consulta O(log N + eventos del bloque), sin mutar el plan ni reservar memoria.
    /// Seek no reconstruye notas sostenidas; el host debe liberar voces antes de saltar.
    pub fn events_in(&self, window: FrameWindow) -> impl Iterator<Item = ScheduledEvent> + '_ {
        let start = self
            .events
            .partition_point(|item| item.frame < window.start());
        self.events[start..]
            .iter()
            .take_while(move |item| item.frame < window.end())
            .map(move |item| ScheduledEvent {
                sample_offset: (item.frame - window.start()) as u32,
                ..item.event
            })
    }
}
