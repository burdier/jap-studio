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
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    AllNotesOff,
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
}
