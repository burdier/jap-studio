//! Plan inmutable preparado fuera del callback; consulta sin asignaciones por bloque.
//! Primera etapa: una pasada a tempo constante. Loop/tempo map requieren segmentación futura.
use super::{
    model::{InstrumentId, NoteId, Pattern},
    time::{FrameWindow, Tick, TimeBase, TimeError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    NoteOff,
    NoteOn { velocity: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledEvent {
    pub sample_offset: u32,
    pub note: NoteId,
    pub instrument: InstrumentId,
    pub pitch: u8,
    pub kind: EventKind,
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
        }
        // Off antes de On en un retrigger. IDs estabilizan notas simultáneas.
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
