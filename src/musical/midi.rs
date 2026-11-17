//! Mensajes y servidor de enrutamiento MIDI (arquitectura servidor/cliente).
//! Los controladores (teclado del ordenador, futuro MIDI físico, pad XY) publican
//! `MidiMessage` en `MidiServer` sin saber quién escucha. El servidor mantiene el
//! cliente activo (`MidiTarget`) y el registro de notas activas por cliente:
//! si el cliente activo cambia mientras hay notas pulsadas, el servidor emite
//! automáticamente los `NoteOff` al cliente anterior para que ningún instrumento
//! o secuenciador se quede colgado con el MIDI ni con notas fantasma.
use super::{model::InstrumentId, track::HiloMode, weave::WeaveRule};
use std::collections::{HashMap, HashSet};

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

    /// Conserva el canal de origen para emparejar notas MIDI físicas repetidas.
    pub fn input_from_bytes(bytes: &[u8]) -> Option<MidiInputEvent> {
        let message = Self::from_bytes(bytes)?;
        let channel = bytes
            .first()
            .copied()
            .filter(|status| (0x80..=0xef).contains(status))
            .map_or(0, |status| status & 0x0f);
        Some(MidiInputEvent { channel, message })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiInputEvent {
    pub channel: u8,
    pub message: MidiMessage,
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
    active_targets: Vec<MidiTarget>,
    held: HashSet<(MidiTarget, u8)>,
    mode: HiloMode,
    next_branch: usize,
    weave_rule: Option<WeaveRule>,
    round_robin_owners: HashMap<(u8, u8), Vec<Option<MidiTarget>>>,
    /// NoteOff físico pendiente después de liberar una nota al cambiar de foco.
    released_on_focus: HashSet<u8>,
}

impl MidiServer {
    pub fn new(initial: Option<MidiTarget>) -> Self {
        Self {
            active_targets: initial.into_iter().collect(),
            held: HashSet::new(),
            mode: HiloMode::Layer,
            next_branch: 0,
            weave_rule: None,
            round_robin_owners: HashMap::new(),
            released_on_focus: HashSet::new(),
        }
    }

    pub fn active_target(&self) -> Option<MidiTarget> {
        self.active_targets.first().copied()
    }

    pub fn active_targets(&self) -> &[MidiTarget] {
        &self.active_targets
    }

    /// Publica una regla preparada fuera de audio; cambiarla libera notas
    /// anteriores antes de volver al primer paso.
    pub fn set_weave_rule(&mut self, rule: Option<WeaveRule>) -> Result<Vec<RoutedMidi>, String> {
        if rule
            .as_ref()
            .is_some_and(|rule| !rule.valid_for(self.active_targets.len()))
        {
            return Err("Tejer: la regla menciona una rama inexistente".into());
        }
        if self.weave_rule == rule {
            return Ok(Vec::new());
        }
        let released = self.release_all();
        self.weave_rule = rule;
        self.next_branch = 0;
        Ok(released)
    }

    /// Cambia el cliente suscrito a la entrada MIDI. Si el cliente anterior tenía
    /// notas sostenidas, devuelve los `NoteOff` dirigidos a ese cliente anterior.
    pub fn focus(&mut self, target: Option<MidiTarget>) -> Vec<RoutedMidi> {
        self.focus_targets_mode(target, HiloMode::Layer)
    }

    /// Cambia el reparto de un Hilo. Una edición del modo libera las notas activas
    /// antes de publicar la nueva ruta, evitando NoteOff en otra rama.
    pub fn focus_targets_mode(
        &mut self,
        targets: impl IntoIterator<Item = MidiTarget>,
        mode: HiloMode,
    ) -> Vec<RoutedMidi> {
        let changed = self.mode != mode;
        let mut released = if changed {
            self.release_all()
        } else {
            Vec::new()
        };
        self.mode = mode;
        if changed {
            self.next_branch = 0;
        }
        released.extend(self.focus_targets(targets));
        released
    }

    /// Suscribe uno o varios clientes simultáneamente (ej. dos pistas armadas con MIDI).
    /// Libera únicamente las notas sostenidas por clientes que dejan de estar activos.
    pub fn focus_targets(
        &mut self,
        targets: impl IntoIterator<Item = MidiTarget>,
    ) -> Vec<RoutedMidi> {
        let mut next = Vec::new();
        for target in targets {
            if !next.contains(&target) {
                next.push(target);
            }
        }
        if self.active_targets == next {
            return Vec::new();
        }
        self.next_branch = 0;
        let mut released = Vec::new();
        for &previous in &self.active_targets.clone() {
            if !next.contains(&previous) {
                released.extend(self.release_target(previous));
            }
        }
        for event in &released {
            if let MidiMessage::NoteOff { note } = event.message
                && !self.held.iter().any(|&(_, held_note)| held_note == note)
            {
                self.released_on_focus.insert(note);
            }
        }
        self.active_targets = next;
        if self
            .weave_rule
            .as_ref()
            .is_some_and(|rule| !rule.valid_for(self.active_targets.len()))
        {
            self.weave_rule = None;
        }
        released
    }

    /// Publica mensajes en el servidor.
    /// - `NoteOn` va a todos los clientes activos y queda registrado a su nombre.
    /// - `NoteOff` va a los clientes que iniciaron esa nota (aunque el foco haya cambiado),
    ///   evitando que el instrumento anterior se quede con la nota pegada.
    /// - `AllNotesOff` libera todos los clientes con notas activas.
    pub fn publish(&mut self, messages: impl IntoIterator<Item = MidiMessage>) -> Vec<RoutedMidi> {
        self.publish_inputs(messages.into_iter().map(|message| MidiInputEvent {
            channel: 0,
            message,
        }))
    }

    pub fn publish_inputs(
        &mut self,
        inputs: impl IntoIterator<Item = MidiInputEvent>,
    ) -> Vec<RoutedMidi> {
        let mut routed = Vec::new();
        for MidiInputEvent { channel, message } in inputs {
            match message {
                MidiMessage::NoteOn { note, velocity } => {
                    if !self.active_targets.is_empty() {
                        self.released_on_focus.remove(&note);
                        let targets: Vec<_> = if self.mode == HiloMode::RoundRobin {
                            let index = self
                                .weave_rule
                                .as_ref()
                                .map(|rule| rule.branch_at(self.next_branch))
                                .unwrap_or(self.next_branch % self.active_targets.len());
                            let target = self.active_targets[index];
                            self.next_branch = self.next_branch.wrapping_add(1);
                            self.round_robin_owners
                                .entry((channel.min(15), note))
                                .or_default()
                                .push(Some(target));
                            vec![target]
                        } else {
                            self.active_targets.clone()
                        };
                        for target in targets {
                            self.held.insert((target, note));
                            routed.push(RoutedMidi {
                                target,
                                message: MidiMessage::NoteOn { note, velocity },
                            });
                        }
                    }
                }
                MidiMessage::NoteOff { note } => {
                    if self.mode == HiloMode::RoundRobin {
                        let key = (channel.min(15), note);
                        let slot = self.round_robin_owners.get_mut(&key).and_then(Vec::pop);
                        if let Some(slot) = slot {
                            if self.round_robin_owners[&key].is_empty() {
                                self.round_robin_owners.remove(&key);
                            }
                            if let Some(target) = slot
                                && !self.round_robin_owners.iter().any(
                                    |(&(_, owner_note), owners)| {
                                        owner_note == note && owners.contains(&Some(target))
                                    },
                                )
                            {
                                self.held.remove(&(target, note));
                                routed.push(RoutedMidi {
                                    target,
                                    message: MidiMessage::NoteOff { note },
                                });
                            }
                            continue;
                        }
                        continue;
                    }
                    let mut owners: Vec<MidiTarget> = self
                        .held
                        .iter()
                        .filter_map(|&(target, held_note)| (held_note == note).then_some(target))
                        .collect();
                    owners.sort_by_key(|target| {
                        self.active_targets
                            .iter()
                            .position(|t| t == target)
                            .unwrap_or(usize::MAX)
                    });
                    if owners.is_empty() {
                        if self.released_on_focus.remove(&note) {
                            continue;
                        }
                        for &target in &self.active_targets {
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
                    for &target in &self.active_targets {
                        routed.push(RoutedMidi {
                            target,
                            message: MidiMessage::ControlChange { cc, value },
                        });
                    }
                }
                MidiMessage::PitchBend { bend } => {
                    for &target in &self.active_targets {
                        routed.push(RoutedMidi {
                            target,
                            message: MidiMessage::PitchBend { bend },
                        });
                    }
                }
                MidiMessage::ProgramChange { program } => {
                    for &target in &self.active_targets {
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
        for owners in self.round_robin_owners.values_mut() {
            for owner in owners {
                if *owner == Some(target) {
                    *owner = None;
                }
            }
        }
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
        self.next_branch = 0;
        self.round_robin_owners.clear();
        let mut entries: Vec<(MidiTarget, u8)> = self.held.drain().collect();
        entries.sort_by_key(|&(_, note)| note);
        self.released_on_focus
            .extend(entries.iter().map(|(_, note)| *note));
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
    fn prepared_weave_routes_three_branches_and_releases_original_owners() {
        let targets = [1, 2, 3].map(|n| MidiTarget::Instrument(InstrumentId(n)));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode(targets, HiloMode::RoundRobin);
        server
            .set_weave_rule(Some(WeaveRule::new(vec![0, 2, 1]).unwrap()))
            .unwrap();
        let owners: Vec<_> = (60..66)
            .map(|note| {
                server.publish([MidiMessage::NoteOn {
                    note,
                    velocity: 100,
                }])[0]
                    .target
            })
            .collect();
        assert_eq!(
            owners,
            [
                targets[0], targets[2], targets[1], targets[0], targets[2], targets[1]
            ]
        );
        for note in (60..66).rev() {
            assert_eq!(
                server.publish([MidiMessage::NoteOff { note }])[0].target,
                owners[(note - 60) as usize]
            );
        }
        assert!(
            server
                .set_weave_rule(Some(WeaveRule::new(vec![3]).unwrap()))
                .is_err()
        );
    }

    #[test]
    fn round_robin_releases_the_branch_that_received_each_press() {
        let a = MidiTarget::Instrument(InstrumentId(41));
        let b = MidiTarget::Instrument(InstrumentId(42));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode([a, b], HiloMode::RoundRobin);
        assert_eq!(
            server.publish([MidiMessage::NoteOn {
                note: 60,
                velocity: 100
            }])[0]
                .target,
            a
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOn {
                note: 60,
                velocity: 100
            }])[0]
                .target,
            b
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }])[0].target,
            b
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }])[0].target,
            a
        );
        assert!(server.release_all().is_empty());
    }

    #[test]
    fn round_robin_repeated_pitch_waits_for_last_press_on_the_same_branch() {
        let a = MidiTarget::Instrument(InstrumentId(41));
        let b = MidiTarget::Instrument(InstrumentId(42));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode([a, b], HiloMode::RoundRobin);
        for _ in 0..3 {
            server.publish([MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            }]);
        }
        assert!(
            server
                .publish([MidiMessage::NoteOff { note: 60 }])
                .is_empty()
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }])[0].target,
            b
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }])[0].target,
            a
        );
        assert!(server.release_all().is_empty());
    }

    #[test]
    fn round_robin_pairs_repeated_pitch_with_its_input_channel() {
        let a = MidiTarget::Instrument(InstrumentId(41));
        let b = MidiTarget::Instrument(InstrumentId(42));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode([a, b], HiloMode::RoundRobin);
        let input = |channel, message| MidiInputEvent { channel, message };
        assert_eq!(
            server.publish_inputs([input(
                0,
                MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100
                }
            )])[0]
                .target,
            a
        );
        assert_eq!(
            server.publish_inputs([input(
                1,
                MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100
                }
            )])[0]
                .target,
            b
        );
        assert_eq!(
            server.publish_inputs([input(0, MidiMessage::NoteOff { note: 60 })]),
            vec![RoutedMidi {
                target: a,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
        assert_eq!(
            server.publish_inputs([input(1, MidiMessage::NoteOff { note: 60 })]),
            vec![RoutedMidi {
                target: b,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
        assert!(server.release_all().is_empty());
    }

    #[test]
    fn removed_branch_discards_its_late_release_on_the_original_channel() {
        let a = MidiTarget::Instrument(InstrumentId(41));
        let b = MidiTarget::Instrument(InstrumentId(42));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode([a, b], HiloMode::RoundRobin);
        let input = |channel, message| MidiInputEvent { channel, message };
        server.publish_inputs([input(
            0,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        )]);
        server.publish_inputs([input(
            1,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            },
        )]);
        assert_eq!(
            server.focus_targets_mode([b], HiloMode::RoundRobin),
            vec![RoutedMidi {
                target: a,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
        assert!(
            server
                .publish_inputs([input(0, MidiMessage::NoteOff { note: 60 })])
                .is_empty()
        );
        assert_eq!(
            server.publish_inputs([input(1, MidiMessage::NoteOff { note: 60 })]),
            vec![RoutedMidi {
                target: b,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
    }

    #[test]
    fn removing_one_round_robin_branch_keeps_the_other_note_held() {
        let a = MidiTarget::Instrument(InstrumentId(41));
        let b = MidiTarget::Instrument(InstrumentId(42));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode([a, b], HiloMode::RoundRobin);
        server.publish([MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        }]);
        server.publish([MidiMessage::NoteOn {
            note: 62,
            velocity: 100,
        }]);
        assert_eq!(
            server.focus_targets_mode([b], HiloMode::RoundRobin),
            vec![RoutedMidi {
                target: a,
                message: MidiMessage::NoteOff { note: 60 },
            }]
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 62 }])[0].target,
            b
        );
        assert!(
            server
                .publish([MidiMessage::NoteOff { note: 60 }])
                .is_empty()
        );
    }

    #[test]
    fn late_release_of_removed_branch_does_not_release_same_pitch_on_another() {
        let a = MidiTarget::Instrument(InstrumentId(41));
        let b = MidiTarget::Instrument(InstrumentId(42));
        let mut server = MidiServer::new(None);
        server.focus_targets_mode([a, b], HiloMode::RoundRobin);
        for _ in 0..2 {
            server.publish([MidiMessage::NoteOn {
                note: 60,
                velocity: 100,
            }]);
        }
        assert_eq!(
            server.focus_targets_mode([a], HiloMode::RoundRobin),
            vec![RoutedMidi {
                target: b,
                message: MidiMessage::NoteOff { note: 60 },
            }]
        );
        assert!(
            server
                .publish([MidiMessage::NoteOff { note: 60 }])
                .is_empty()
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }]),
            vec![RoutedMidi {
                target: a,
                message: MidiMessage::NoteOff { note: 60 },
            }]
        );
    }

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
        assert!(
            server
                .publish([MidiMessage::NoteOff { note: 60 }])
                .is_empty()
        );
    }

    #[test]
    fn retrigger_after_focus_change_keeps_the_new_owner() {
        let piano = MidiTarget::Instrument(InstrumentId(1));
        let sampler = MidiTarget::Instrument(InstrumentId(2));
        let mut server = MidiServer::new(Some(piano));
        server.publish([MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        }]);
        assert_eq!(server.focus(Some(sampler))[0].target, piano);
        assert_eq!(
            server.publish([MidiMessage::NoteOn {
                note: 60,
                velocity: 80
            }])[0]
                .target,
            sampler
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }]),
            vec![RoutedMidi {
                target: sampler,
                message: MidiMessage::NoteOff { note: 60 },
            }]
        );
        assert!(server.release_all().is_empty());
    }

    #[test]
    fn stop_discards_the_late_physical_note_off() {
        let target = MidiTarget::Instrument(InstrumentId(2));
        let mut server = MidiServer::new(Some(target));
        server.publish([MidiMessage::NoteOn {
            note: 64,
            velocity: 90,
        }]);
        assert_eq!(server.release_all().len(), 1);
        assert!(
            server
                .publish([MidiMessage::NoteOff { note: 64 }])
                .is_empty()
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOn {
                note: 64,
                velocity: 90
            }])[0]
                .target,
            target
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 64 }])[0].target,
            target
        );
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
        assert_eq!(
            MidiMessage::input_from_bytes(&[0x91, 60, 100]),
            Some(MidiInputEvent {
                channel: 1,
                message: MidiMessage::NoteOn {
                    note: 60,
                    velocity: 100
                },
            })
        );
        assert_eq!(
            MidiMessage::input_from_bytes(&[0x81, 60, 0]),
            Some(MidiInputEvent {
                channel: 1,
                message: MidiMessage::NoteOff { note: 60 }
            })
        );
    }

    #[test]
    fn two_active_targets_receive_notes_and_partial_unfocus_releases_only_removed() {
        let a = MidiTarget::Instrument(InstrumentId(1));
        let b = MidiTarget::Instrument(InstrumentId(2));
        let mut server = MidiServer::new(None);
        assert!(server.focus_targets([a, b]).is_empty());
        assert_eq!(server.active_targets(), &[a, b]);

        let on = server.publish([MidiMessage::NoteOn {
            note: 60,
            velocity: 100,
        }]);
        assert_eq!(
            on,
            vec![
                RoutedMidi {
                    target: a,
                    message: MidiMessage::NoteOn {
                        note: 60,
                        velocity: 100
                    }
                },
                RoutedMidi {
                    target: b,
                    message: MidiMessage::NoteOn {
                        note: 60,
                        velocity: 100
                    }
                },
            ]
        );

        let handoff = server.focus_targets([b]);
        assert_eq!(
            handoff,
            vec![RoutedMidi {
                target: a,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
        assert_eq!(
            server.publish([MidiMessage::NoteOff { note: 60 }]),
            vec![RoutedMidi {
                target: b,
                message: MidiMessage::NoteOff { note: 60 }
            }]
        );
    }
}
