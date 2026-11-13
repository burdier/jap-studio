//! Voz de instrumento preparada: estado MIDI y patrón sin poseer un stream CPAL.
use super::{InstrumentState, NoteCommand, SynthBackend, plan::render_plan_block};
use elevate::musical::model::InstrumentId;
use elevate::musical::scheduler::PreparedPattern;
use std::sync::{Arc, atomic::Ordering};

pub(super) struct InstrumentVoice {
    synth: SynthBackend,
    plan: Option<(Arc<PreparedPattern>, u64, bool, u64)>,
    channel: u8,
}

impl InstrumentVoice {
    pub fn new(synth: SynthBackend) -> Self {
        Self {
            synth,
            plan: None,
            channel: 0,
        }
    }

    pub fn command(&mut self, command: NoteCommand) {
        match command {
            NoteCommand::Channel(value) => {
                self.synth.panic();
                self.channel = value;
            }
            NoteCommand::SamplerControl(cc, value) => {
                if let SynthBackend::Sampler(sampler) = &mut self.synth {
                    sampler.control(cc, value);
                }
            }
            NoteCommand::On { note, velocity } => {
                self.synth.midi(0x90 | self.channel, note, velocity);
            }
            NoteCommand::Off { note } => self.synth.midi(0x80 | self.channel, note, 0),
            NoteCommand::AllOff => self.synth.panic(),
            NoteCommand::Midi {
                command,
                data1,
                data2,
            } => {
                self.synth
                    .midi((command & 0xf0) | self.channel, data1, data2);
            }
            NoteCommand::MasterVolume(volume) => self.synth.set_master_volume(volume),
            NoteCommand::Polyphony(polyphony) => self.synth.set_polyphony(polyphony),
            NoteCommand::Effects(enabled) => self.synth.set_effects(enabled),
            NoteCommand::PlayPlan {
                plan,
                start_frame,
                looped,
            } => {
                self.synth.panic();
                self.plan = Some((plan, start_frame, looped, 0));
            }
            NoteCommand::StopPlan => {
                if self.plan.take().is_some() {
                    self.synth.panic();
                }
            }
            NoteCommand::Filter(_) => {}
        }
    }

    pub fn render(&mut self, left: &mut [f32], right: &mut [f32], state: &InstrumentState) {
        if let Some((plan, position, looped, cycle)) = self.plan.as_mut() {
            let finished = render_plan_block(
                &mut self.synth,
                plan,
                position,
                *looped,
                self.channel,
                cycle,
                left,
                right,
            );
            state.plan_frame.store(*position, Ordering::Relaxed);
            if finished {
                self.plan = None;
                state.plan_playing.store(false, Ordering::Relaxed);
                state.plan_finished.store(true, Ordering::Relaxed);
            }
        } else {
            self.synth.render(left, right);
        }
    }

    pub fn failed(&self) -> bool {
        self.synth.failed()
    }
}

/// Voces preparadas fuera del callback, identificadas por instancia.
/// No abre dispositivos; el motor común podrá renderizarlas en su propio bloque.
pub(super) struct PreparedVoices {
    voices: Vec<(InstrumentId, InstrumentVoice, Arc<InstrumentState>)>,
}

impl PreparedVoices {
    pub fn new() -> Self {
        Self { voices: Vec::new() }
    }

    /// Transferencia de un backend ya preparado. Rechaza IDs duplicados sin alterar el conjunto.
    pub fn insert(
        &mut self,
        id: InstrumentId,
        synth: SynthBackend,
        state: Arc<InstrumentState>,
    ) -> Result<(), SynthBackend> {
        if self.voices.iter().any(|(existing, _, _)| *existing == id) {
            return Err(synth);
        }
        self.voices.push((id, InstrumentVoice::new(synth), state));
        Ok(())
    }

    pub fn command(&mut self, id: InstrumentId, command: NoteCommand) -> bool {
        let Some((_, voice, _)) = self.voices.iter_mut().find(|(key, _, _)| *key == id) else {
            return false;
        };
        voice.command(command);
        true
    }

    /// Suma en flotante; los scratch se crean en el stack, nunca por bloque en el heap.
    pub fn render(&mut self, output: &mut [[f32; 2]]) {
        for chunk in output.chunks_mut(512) {
            chunk.fill([0.0; 2]);
            for (_, voice, state) in &mut self.voices {
                let mut left = [0.0; 512];
                let mut right = [0.0; 512];
                voice.render(&mut left[..chunk.len()], &mut right[..chunk.len()], state);
                if voice.failed() {
                    state.failed.store(true, Ordering::Relaxed);
                }
                for (frame, (l, r)) in chunk.iter_mut().zip(left.iter().zip(right.iter())) {
                    frame[0] += l;
                    frame[1] += r;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustysynth::{Synthesizer, SynthesizerSettings};

    fn synth(font: &Arc<rustysynth::SoundFont>) -> SynthBackend {
        let settings = SynthesizerSettings::new(48_000);
        SynthBackend::Rusty(Synthesizer::new(font, &settings).unwrap())
    }

    fn add(set: &mut PreparedVoices, id: InstrumentId, font: &Arc<rustysynth::SoundFont>) {
        assert!(
            set.insert(
                id,
                synth(font),
                Arc::new(InstrumentState::new(1.0, Default::default(), 0.0, 1.0)),
            )
            .is_ok()
        );
    }

    #[test]
    fn two_instances_keep_notes_separate_and_mix_as_their_sum() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/soundfonts/UprightPianoKW-20220221.sf2");
        let font = super::super::load_soundfont(&path).unwrap();
        let a = InstrumentId(41);
        let b = InstrumentId(42);
        let mut together = PreparedVoices::new();
        let mut only_a = PreparedVoices::new();
        let mut only_b = PreparedVoices::new();
        add(&mut together, a, &font);
        add(&mut together, b, &font);
        add(&mut only_a, a, &font);
        add(&mut only_b, b, &font);
        assert!(
            together
                .insert(
                    a,
                    synth(&font),
                    Arc::new(InstrumentState::new(1.0, Default::default(), 0.0, 1.0))
                )
                .is_err()
        );
        assert!(!together.command(InstrumentId(99), NoteCommand::AllOff));
        for set in [&mut together, &mut only_a] {
            assert!(set.command(
                a,
                NoteCommand::On {
                    note: 60,
                    velocity: 100
                }
            ));
        }
        for set in [&mut together, &mut only_b] {
            assert!(set.command(b, NoteCommand::MasterVolume(0.35)));
            assert!(set.command(
                b,
                NoteCommand::On {
                    note: 67,
                    velocity: 88
                }
            ));
        }
        for pass in 0..2 {
            if pass == 1 {
                assert!(together.command(a, NoteCommand::Off { note: 60 }));
                assert!(only_a.command(a, NoteCommand::Off { note: 60 }));
            }
            let mut mixed = [[0.0; 2]; 256];
            let mut left = [[0.0; 2]; 256];
            let mut right = [[0.0; 2]; 256];
            together.render(&mut mixed);
            only_a.render(&mut left);
            only_b.render(&mut right);
            assert!(right.iter().any(|frame| frame[0].abs() > 0.00001));
            for ((both, a), b) in mixed.iter().zip(&left).zip(&right) {
                assert!((both[0] - a[0] - b[0]).abs() < 0.00001);
                assert!((both[1] - a[1] - b[1]).abs() < 0.00001);
            }
        }
    }
}
