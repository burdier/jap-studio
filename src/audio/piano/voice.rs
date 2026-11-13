//! Voz de instrumento preparada: estado MIDI y patrón sin poseer un stream CPAL.
use super::{InstrumentState, NoteCommand, SynthBackend, plan::render_plan_block};
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
