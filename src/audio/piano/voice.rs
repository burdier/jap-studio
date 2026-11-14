//! Voz de instrumento preparada: estado MIDI y patrón sin poseer un stream CPAL.
use super::{InstrumentState, NoteCommand, SynthBackend, plan::render_plan_block};
use crate::audio::vst3::{SharedInstrument, TransportContext};
use elevate::musical::model::InstrumentId;
use elevate::musical::scheduler::PreparedPattern;
use elevate::musical::{
    arrangement_midi::PreparedMidiArrangement,
    midi::MidiMessage,
    scheduler::{EventKind, ScheduledEvent},
    time::FrameWindow,
};
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

    pub fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        state: &InstrumentState,
        time: Option<TransportContext>,
    ) {
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
            self.synth.render_timed(left, right, time);
        }
    }

    pub fn failed(&self) -> bool {
        self.synth.failed()
    }

    fn scheduled(&mut self, channel: u8, event: ScheduledEvent) {
        match event.kind {
            EventKind::NoteOn { velocity } if event.expression.plays(event.note, 0) => {
                self.synth
                    .note_on_expression(channel, event.pitch, velocity, event.expression);
            }
            EventKind::NoteOff if event.expression.plays(event.note, 0) => {
                self.synth.midi(
                    0x80 | channel,
                    event.pitch,
                    event.expression.release_velocity,
                );
            }
            EventKind::Control { lane, value } => {
                let (command, a, b) = lane.midi(value);
                self.synth.midi(command | channel, a, b);
            }
            _ => {}
        }
    }
}

/// Voces preparadas fuera del callback, identificadas por instancia.
/// No abre dispositivos; el motor común podrá renderizarlas en su propio bloque.
pub(super) struct PreparedVoices {
    voices: Vec<(InstrumentId, InstrumentVoice, Arc<InstrumentState>)>,
    last_end: Option<u64>,
}

#[derive(Clone)]
pub(crate) enum VoiceSource {
    Piano(super::PianoSnapshot),
    Sampler(crate::audio::sampler::SamplerSnapshot),
    Vst(Arc<SharedInstrument>),
}

/// Una instancia por pista, preparada por el worker antes de publicar el Mix.
pub(crate) struct PreparedArrangementVoice {
    id: InstrumentId,
    voices: PreparedVoices,
    filter: Option<super::filter::StereoFilter>,
}

impl PreparedArrangementVoice {
    pub fn prepare(id: InstrumentId, source: VoiceSource, rate: u32) -> Result<Self, String> {
        let (backend, filter) = match source {
            VoiceSource::Piano(snapshot) => {
                let filter = super::filter::StereoFilter::new(rate, snapshot.filter);
                let font = super::load_soundfont(&snapshot.path)?;
                let mut synth = SynthBackend::new(&snapshot.path, &font, rate, snapshot.settings)?;
                synth.set_master_volume(snapshot.master_volume);
                for (command, a, b) in snapshot.midi {
                    synth.midi(command, a, b);
                }
                (synth, Some(filter))
            }
            VoiceSource::Sampler(snapshot) => (
                SynthBackend::Sampler(crate::audio::sampler::SfizzEngine::prepare(snapshot, rate)?),
                None,
            ),
            VoiceSource::Vst(shared) => {
                if shared.rate != rate {
                    return Err(
                        "La salida cambió de frecuencia; recarga el instrumento VST3".into(),
                    );
                }
                (
                    SynthBackend::Vst(crate::audio::vst3::InstrumentVoice::arrangement(shared)),
                    None,
                )
            }
        };
        let mut voices = PreparedVoices::new();
        voices
            .insert(
                id,
                backend,
                Arc::new(InstrumentState::new(1.0, Default::default(), 0.0, 1.0)),
            )
            .map_err(|_| "ID de instrumento duplicado")?;
        Ok(Self { id, voices, filter })
    }

    pub fn id(&self) -> InstrumentId {
        self.id
    }

    pub fn live_midi(&mut self, message: MidiMessage) {
        let command = match message {
            MidiMessage::NoteOn { note, velocity } => NoteCommand::On { note, velocity },
            MidiMessage::NoteOff { note } => NoteCommand::Off { note },
            MidiMessage::ControlChange { cc, value } => NoteCommand::Midi {
                command: 0xb0,
                data1: cc,
                data2: value,
            },
            MidiMessage::PitchBend { bend } => {
                let value = (i32::from(bend) + 8192).clamp(0, 16383) as u16;
                NoteCommand::Midi {
                    command: 0xe0,
                    data1: (value & 127) as u8,
                    data2: (value >> 7) as u8,
                }
            }
            MidiMessage::ProgramChange { program } => NoteCommand::Midi {
                command: 0xc0,
                data1: program,
                data2: 0,
            },
            MidiMessage::AllNotesOff => NoteCommand::AllOff,
        };
        self.voices.command(self.id, command);
    }

    pub fn render_timed(
        &mut self,
        plan: &PreparedMidiArrangement,
        at: u64,
        output: &mut [[f32; 2]],
        time: Option<TransportContext>,
    ) {
        self.voices.render_arrangement_timed(plan, at, output, time);
        if let Some(filter) = &mut self.filter {
            filter.process(output);
        }
    }
}

impl PreparedVoices {
    pub fn new() -> Self {
        Self {
            voices: Vec::new(),
            last_end: None,
        }
    }

    /// Transferencia de un backend ya preparado. Rechaza IDs duplicados sin alterar el conjunto.
    pub fn insert(
        &mut self,
        id: InstrumentId,
        synth: SynthBackend,
        state: Arc<InstrumentState>,
    ) -> Result<(), SynthBackend> {
        if self.voices.len() >= 256 || self.voices.iter().any(|(existing, _, _)| *existing == id) {
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
        self.render_timed(output, None);
    }
    fn render_timed(&mut self, output: &mut [[f32; 2]], time: Option<TransportContext>) {
        for (index, chunk) in output.chunks_mut(512).enumerate() {
            chunk.fill([0.0; 2]);
            for (_, voice, state) in &mut self.voices {
                let mut left = [0.0; 512];
                let mut right = [0.0; 512];
                voice.render(
                    &mut left[..chunk.len()],
                    &mut right[..chunk.len()],
                    state,
                    time.map(|time| time.offset(index * 512)),
                );
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

    /// Renderiza eventos absolutos de todos los clips. Los segmentos entre eventos
    /// comparten el frame del transporte; ningún evento se desplaza con el tamaño de bloque.
    #[cfg(test)]
    pub fn render_arrangement(
        &mut self,
        plan: &PreparedMidiArrangement,
        position: u64,
        output: &mut [[f32; 2]],
    ) {
        self.render_arrangement_timed(plan, position, output, None);
    }
    pub fn render_arrangement_timed(
        &mut self,
        plan: &PreparedMidiArrangement,
        position: u64,
        output: &mut [[f32; 2]],
        time: Option<TransportContext>,
    ) {
        if self.last_end.is_some_and(|end| end != position) {
            for (_, voice, _) in &mut self.voices {
                voice.command(NoteCommand::AllOff);
            }
        }
        for (index, block) in output.chunks_mut(512).enumerate() {
            let at = position.saturating_add(index as u64 * 512);
            let Ok(window) = FrameWindow::new(at, block.len() as u32) else {
                block.fill([0.0; 2]);
                break;
            };
            let mut written = 0;
            for routed in plan.events_in(window) {
                let offset = routed.event.sample_offset as usize;
                if offset > written {
                    self.render_timed(
                        &mut block[written..offset],
                        time.map(|time| time.offset(index * 512 + written)),
                    );
                    written = offset;
                }
                if let Some((_, voice, _)) = self
                    .voices
                    .iter_mut()
                    .find(|(id, _, _)| *id == routed.instrument)
                {
                    voice.scheduled(routed.channel, routed.event);
                }
            }
            if written < block.len() {
                self.render_timed(
                    &mut block[written..],
                    time.map(|time| time.offset(index * 512 + written)),
                );
            }
        }
        self.last_end = position.checked_add(output.len() as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::musical::{
        clip::MidiClip,
        model::Pattern,
        time::{PPQ, Tick},
        track::DEMO_TRACKS,
    };
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

    #[test]
    fn arrangement_events_drive_two_voices_independently_of_block_size() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/soundfonts/UprightPianoKW-20220221.sf2");
        let font = super::super::load_soundfont(&path).unwrap();
        let mut tracks = DEMO_TRACKS;
        tracks[0].instrument = Some(InstrumentId(41));
        tracks[1].instrument = Some(InstrumentId(42));
        let mut first = Pattern::new(Tick(PPQ)).unwrap();
        first
            .add_note(InstrumentId(41), Tick(0), Tick(4), 60, 100)
            .unwrap();
        let mut second = Pattern::new(Tick(PPQ)).unwrap();
        second
            .add_note(InstrumentId(42), Tick(1), Tick(4), 67, 90)
            .unwrap();
        let clips = [
            MidiClip::new("A", 0, 0.0, first),
            MidiClip::new("B", 1, 0.0, second),
        ];
        let plan = PreparedMidiArrangement::compile(&clips, &tracks, 48_000, 120.0).unwrap();
        let make = || {
            let mut set = PreparedVoices::new();
            add(&mut set, InstrumentId(41), &font);
            add(&mut set, InstrumentId(42), &font);
            set
        };
        let mut whole = make();
        let mut split = make();
        let mut a = [[0.0; 2]; 256];
        let mut b = a;
        whole.render_arrangement(&plan, 0, &mut a);
        for (index, block) in b.chunks_mut(13).enumerate() {
            split.render_arrangement(&plan, (index * 13) as u64, block);
        }
        assert!(a.iter().any(|frame| frame[0].abs() > 0.00001));
        for (x, y) in a.iter().zip(b) {
            assert!((x[0] - y[0]).abs() < 0.0001);
            assert!((x[1] - y[1]).abs() < 0.0001);
        }
    }
}
