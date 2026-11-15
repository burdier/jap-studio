//! Desactivación de la instancia y drenaje de notas por sus destinos originales.
use super::{LiveEvent, LiveNoteProcessor};
use crate::musical::midi::MidiMessage;

pub struct SafeBlock<'a> {
    pub events: &'a [LiveEvent],
    pub error: Option<&'a str>,
}

impl LiveNoteProcessor {
    /// Ante fallo devuelve solo liberaciones de notas ya publicadas. Los nuevos
    /// NoteOn se descartan hasta preparar otra instancia; el error queda para UI.
    pub fn process_or_disable(&mut self, input: &[LiveEvent], frames: u32) -> SafeBlock<'_> {
        if self.disabled {
            return SafeBlock {
                events: &[],
                error: None,
            };
        }
        let result = self.process_block(input, frames).map(|_| ());
        match result {
            Ok(()) => SafeBlock {
                events: &self.output,
                error: None,
            },
            Err(error) => {
                self.disable_with(error);
                SafeBlock {
                    events: &self.output,
                    error: self.last_error.as_deref(),
                }
            }
        }
    }

    /// Stop, seek o retiro de una ruta: liberar antes de descartar la instancia.
    pub fn stop_and_release(&mut self) -> &[LiveEvent] {
        self.disable_with("Lua live: instancia detenida".into());
        &self.output
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn disable_with(&mut self, error: String) {
        self.output.clear();
        for owner in self.active.drain(..) {
            self.output.push(LiveEvent {
                offset: 0,
                channel: owner.output_channel,
                message: MidiMessage::NoteOff {
                    note: owner.output_pitch,
                },
                note_id: owner.id,
            });
        }
        self.pending.clear();
        self.disabled = true;
        self.last_error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musical::model::NoteId;

    fn on(id: u64, note: u8) -> LiveEvent {
        LiveEvent {
            offset: 1,
            channel: 0,
            message: MidiMessage::NoteOn { note, velocity: 90 },
            note_id: Some(NoteId(id)),
        }
    }

    #[test]
    fn failure_releases_previous_destinations_and_drops_later_notes() {
        let source = "local calls = 0; return function(kind, ch, a, b, offset) \
            calls = calls + 1; if calls == 2 then error('fallo deliberado') end \
            if kind == 1 then a = a + 7 end; return kind, ch, a, b, offset end";
        let mut processor = LiveNoteProcessor::prepare(source).unwrap();
        let first = processor.process_or_disable(&[on(1, 60)], 64);
        assert_eq!(
            first.events[0].message,
            MidiMessage::NoteOn {
                note: 67,
                velocity: 90
            }
        );
        assert!(first.error.is_none());
        let failed = processor.process_or_disable(&[on(2, 62)], 64);
        assert_eq!(failed.events.len(), 1);
        assert_eq!(failed.events[0].message, MidiMessage::NoteOff { note: 67 });
        assert_eq!(failed.events[0].note_id, Some(NoteId(1)));
        assert!(failed.error.unwrap().contains("fallo deliberado"));
        assert!(processor.is_disabled());
        assert!(
            processor
                .process_or_disable(&[on(3, 64)], 64)
                .events
                .is_empty()
        );
    }

    #[test]
    fn stop_releases_each_held_note_once() {
        let mut processor = LiveNoteProcessor::prepare(
            "return function(kind, ch, a, b, offset) return kind, ch, a, b, offset end",
        )
        .unwrap();
        processor.process_or_disable(&[on(1, 60), on(2, 62)], 64);
        assert_eq!(processor.stop_and_release().len(), 2);
        assert_eq!(processor.active_count(), 0);
        assert!(processor.stop_and_release().is_empty());
    }
}
