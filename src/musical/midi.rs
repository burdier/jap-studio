//! Mensajes y servidor de enrutamiento MIDI (arquitectura servidor/cliente).
//! Los controladores (teclado del ordenador, futuro MIDI físico, pad XY) publican
//! `MidiMessage` en `MidiServer` sin saber quién escucha. El servidor mantiene el
//! cliente activo (`MidiTarget`) y el registro de notas activas por cliente:
//! si el cliente activo cambia mientras hay notas pulsadas, el servidor emite
//! automáticamente los `NoteOff` al cliente anterior para que ningún instrumento
//! o secuenciador se quede colgado con el MIDI ni con notas fantasma.
use super::model::InstrumentId;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage {
    NoteOn {
        note: u8,
        velocity: u8,
    },
    NoteOff {
        note: u8,
    },
    ControlChange {
        cc: u8,
        value: u8,
    },
    /// Pitch bend de 14 bits centrado en 0 (`-8192..=8191`).
    PitchBend {
        bend: i16,
    },
    ProgramChange {
        program: u8,
    },
    AllNotesOff,
}

impl MidiMessage {
    /// Decodifica bytes MIDI 1.0 en vivo usando `wmidi`.
    /// Trata `NoteOn` con velocity 0 como `NoteOff`, y CC 120/123 como `AllNotesOff`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let parsed = wmidi::MidiMessage::try_from(bytes).ok()?;
        match parsed {
            wmidi::MidiMessage::NoteOn(_, note, velocity) => {
                let note = u8::from(note);
                let velocity = u8::from(velocity);
                if velocity == 0 {
                    Some(Self::NoteOff { note })
                } else {
                    Some(Self::NoteOn { note, velocity })
                }
            }
            wmidi::MidiMessage::NoteOff(_, note, _) => Some(Self::NoteOff {
                note: u8::from(note),
            }),
            wmidi::MidiMessage::ControlChange(_, cc, value) => {
                let cc = u8::from(cc);
                let value = u8::from(value);
                if cc == 120 || cc == 123 {
                    Some(Self::AllNotesOff)
                } else {
                    Some(Self::ControlChange { cc, value })
                }
            }
            wmidi::MidiMessage::PitchBendChange(_, bend) => {
                let raw = u16::from(bend);
                Some(Self::PitchBend {
                    bend: (raw as i32 - 8192).clamp(-8192, 8191) as i16,
                })
            }
            wmidi::MidiMessage::ProgramChange(_, program) => Some(Self::ProgramChange {
                program: u8::from(program),
            }),
            wmidi::MidiMessage::Reset => Some(Self::AllNotesOff),
            _ => None,
        }
    }
}

/// Identidad de cualquier cliente capaz de recibir MIDI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MidiTarget {
    Instrument(InstrumentId),
    Sequencer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutedMidi {
    pub target: MidiTarget,
    pub message: MidiMessage,
}

/// Servidor MIDI central: desacopla los emisores (controladores) de los clientes
/// (instrumentos, secuenciador).
#[derive(Debug, Default)]
pub struct MidiServer {
    active_target: Option<MidiTarget>,
    held: HashSet<(MidiTarget, u8)>,
}

impl MidiServer {
    pub fn new(initial: Option<MidiTarget>) -> Self {
        Self {
            active_target: initial,
            held: HashSet::new(),
        }
    }

    pub fn active_target(&self) -> Option<MidiTarget> {
        self.active_target
    }

    /// Cambia el cliente suscrito a la entrada MIDI. Si el cliente anterior tenía
    /// notas sostenidas, devuelve los `NoteOff` dirigidos a ese cliente anterior.
    pub fn focus(&mut self, target: Option<MidiTarget>) -> Vec<RoutedMidi> {
        if self.active_target == target {
            return Vec::new();
        }
        let released = match self.active_target {
            Some(previous) => self.release_target(previous),
            None => Vec::new(),
        };
        self.active_target = target;
        released
    }

    /// Publica mensajes en el servidor.
    /// - `NoteOn` va al cliente actualmente enfocado y queda registrado a su nombre.
    /// - `NoteOff` va al cliente que inició esa nota (aunque el foco haya cambiado),
    ///   evitando que el instrumento anterior se quede con la nota pegada.
    /// - `AllNotesOff` libera todos los clientes con notas activas.
    pub fn publish(&mut self, messages: impl IntoIterator<Item = MidiMessage>) -> Vec<RoutedMidi> {
        let mut routed = Vec::new();
        for message in messages {
            match message {
                MidiMessage::NoteOn { note, velocity } => {
                    if let Some(target) = self.active_target {
                        self.held.insert((target, note));
                        routed.push(RoutedMidi {
                            target,
                            message: MidiMessage::NoteOn { note, velocity },
                        });
                    }
                }
                MidiMessage::NoteOff { note } => {
                    let owners: Vec<MidiTarget> = self
                        .held
                        .iter()
                        .filter_map(|&(target, held_note)| (held_note == note).then_some(target))
                        .collect();
                    if owners.is_empty() {
                        if let Some(target) = self.active_target {
                            routed.push(RoutedMidi {
                                target,
                                message: MidiMessage::NoteOff { note },
                            });
                        }
                    } else {
                        for target in owners {
                            self.held.remove(&(target, note));
                            routed.push(RoutedMidi {
                                target,
                                message: MidiMessage::NoteOff { note },
                            });
                        }
                    }
                }
                MidiMessage::ControlChange { cc, value } => {
                    if let Some(target) = self.active_target {
                        routed.push(RoutedMidi {
                            target,
                            message: MidiMessage::ControlChange { cc, value },
                        });
                    }
                }
                MidiMessage::PitchBend { bend } => {
                    if let Some(target) = self.active_target {
                        routed.push(RoutedMidi {
                            target,
                            message: MidiMessage::PitchBend { bend },
                        });
                    }
                }
                MidiMessage::ProgramChange { program } => {
                    if let Some(target) = self.active_target {
                        routed.push(RoutedMidi {
                            target,
                            message: MidiMessage::ProgramChange { program },
                        });
                    }
                }
                MidiMessage::AllNotesOff => {
                    routed.extend(self.release_all());
                }
            }
        }
        routed
    }

    /// Libera todas las notas sostenidas de un cliente específico.
    pub fn release_target(&mut self, target: MidiTarget) -> Vec<RoutedMidi> {
        let mut notes: Vec<u8> = self
            .held
            .iter()
            .filter_map(|&(owner, note)| (owner == target).then_some(note))
            .collect();
        notes.sort_unstable();
        for &note in &notes {
            self.held.remove(&(target, note));
        }
        notes
            .into_iter()
            .map(|note| RoutedMidi {
                target,
                message: MidiMessage::NoteOff { note },
            })
            .collect()
    }

    /// Libera todas las notas sostenidas de todos los clientes registrados.
    pub fn release_all(&mut self) -> Vec<RoutedMidi> {
        let mut entries: Vec<(MidiTarget, u8)> = self.held.drain().collect();
        entries.sort_by_key(|&(_, note)| note);
        entries
            .into_iter()
            .map(|(target, note)| RoutedMidi {
                target,
                message: MidiMessage::NoteOff { note },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_focus_releases_notes_on_the_previous_client() {
        let piano = MidiTarget::Instrument(InstrumentId(1));
        let seq = MidiTarget::Sequencer;
        let mut server = MidiServer::new(Some(piano));

        let routed = server.publish([MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        }]);
        assert_eq!(
            routed,
            vec![RoutedMidi {
                target: piano,
                message: MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100
                }
            }]
        );

        let handoff = server.focus(Some(seq));
        assert_eq!(
            handoff,
            vec![RoutedMidi {
                target: piano,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
        assert_eq!(server.active_target(), Some(seq));
    }

    #[test]
    fn without_active_client_messages_are_dropped_cleanly() {
        let mut server = MidiServer::new(None);
        let routed = server.publish([MidiMessage::NoteOn {
            note: 64,
            velocity: 90,
        }]);
        assert!(routed.is_empty());
    }

    #[test]
    fn raw_midi_bytes_parse_notes_cc_and_pitch_bend() {
        assert_eq!(
            MidiMessage::from_bytes(&[0x90, 60, 100]),
            Some(MidiMessage::NoteOn {
                note: 60,
                velocity: 100
            })
        );
        assert_eq!(
            MidiMessage::from_bytes(&[0x90, 60, 0]),
            Some(MidiMessage::NoteOff { note: 60 })
        );
        assert_eq!(
            MidiMessage::from_bytes(&[0xB0, 64, 127]),
            Some(MidiMessage::ControlChange { cc: 64, value: 127 })
        );
        assert_eq!(
            MidiMessage::from_bytes(&[0xE0, 0x00, 0x40]),
            Some(MidiMessage::PitchBend { bend: 0 })
        );
    }
}
