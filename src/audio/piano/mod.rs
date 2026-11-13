//! Adaptador SoundFont (`libfluidsynth` C por `dlopen` con respaldo `RustySynth`) → Volume → CPAL.
mod backend;
mod filter;
mod fluidsynth;
mod offline;
mod plan;
mod prepare;
mod render;
mod soundfont;
mod voice;

use super::output::choose_output_config;
use super::stereo_pan::StereoPan;
pub(crate) use backend::SynthBackend;
use cpal::{SampleFormat, traits::StreamTrait};
use elevate::musical::{
    model::Pattern,
    scheduler::PreparedPattern,
    time::{PPQ, Tick, TimeBase},
};
pub(crate) use fluidsynth::is_available as fluidsynth_available;
pub(crate) use offline::{OfflinePiano, PianoSnapshot};
pub(crate) use prepare::PreparedPiano;
use rtrb::{Consumer, Producer, RingBuffer};
pub(crate) use soundfont::{
    PresetEntry, SoundFontBank, discover as discover_soundfonts, load_path as load_soundfont,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

const COMMAND_QUEUE_CAP: usize = 512;

#[derive(Clone)]
enum NoteCommand {
    Filter(elevate::devices::filter::FilterSettings),
    SamplerControl(u8, f32),
    On {
        note: u8,
        velocity: u8,
    },
    Off {
        note: u8,
    },
    AllOff,
    /// Mensaje MIDI de canal.
    Midi {
        command: u8,
        data1: u8,
        data2: u8,
    },
    Channel(u8),
    MasterVolume(f32),
    Polyphony(usize),
    Effects(bool),
    PlayPlan {
        plan: Arc<PreparedPattern>,
        start_frame: u64,
        looped: bool,
    },
    StopPlan,
}

/// Ajustes que RustySynth solo lee al crear el sintetizador.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PianoSettings {
    pub maximum_polyphony: usize,
    pub reverb_and_chorus: bool,
}

impl Default for PianoSettings {
    fn default() -> Self {
        Self {
            maximum_polyphony: 64,
            reverb_and_chorus: true,
        }
    }
}

struct InstrumentState {
    gain: AtomicU32,
    pan: StereoPan,
    fx: super::fx_gain::FxGain,
    utility: elevate::devices::utility_slot::UtilitySlot,
    failed: AtomicBool,
    plan_frame: AtomicU64,
    plan_playing: AtomicBool,
    plan_finished: AtomicBool,
    spectrum_enabled: AtomicBool,
}

pub(crate) struct PianoPlayer {
    _stream: cpal::Stream,
    commands: Mutex<Producer<NoteCommand>>,
    spectrum: Mutex<Consumer<[f32; 2]>>,
    state: Arc<InstrumentState>,
    pub sample_rate: u32,
}

impl PianoPlayer {
    pub fn start_sampler(
        mut engine: crate::audio::sampler::SfizzEngine,
        gain: f32,
        utility: elevate::devices::utility_slot::UtilitySlot,
        pan: f32,
        fx_gain: f32,
        audio: &super::device::AudioConfig,
    ) -> Result<Self, String> {
        let device = audio.device(false)?;
        let setup = choose_output_config(&device, audio)?;
        engine.set_sample_rate(setup.config.sample_rate);
        Self::start_backend(
            device,
            setup,
            SynthBackend::Sampler(engine),
            gain,
            utility,
            pan,
            fx_gain,
        )
    }
    fn start_backend(
        device: cpal::Device,
        setup: super::output::StreamSetup,
        backend: SynthBackend,
        gain: f32,
        utility: elevate::devices::utility_slot::UtilitySlot,
        pan: f32,
        fx_gain: f32,
    ) -> Result<Self, String> {
        let sample_rate = setup.config.sample_rate;
        let state = Arc::new(InstrumentState {
            gain: AtomicU32::new(gain.clamp(0.0, 2.0).to_bits()),
            pan: StereoPan::new(pan),
            fx: super::fx_gain::FxGain::new(fx_gain),
            utility,
            failed: AtomicBool::new(false),
            plan_frame: AtomicU64::new(0),
            plan_playing: AtomicBool::new(false),
            plan_finished: AtomicBool::new(false),
            spectrum_enabled: AtomicBool::new(false),
        });
        let (producer, consumer) = RingBuffer::new(COMMAND_QUEUE_CAP);
        let (spectrum_tx, spectrum_rx) = RingBuffer::new(8192);
        let stream = match setup.format {
            SampleFormat::F32 => render::build_stream::<f32>(
                &device,
                setup.config,
                setup.first_channel,
                backend,
                consumer,
                spectrum_tx,
                state.clone(),
            ),
            SampleFormat::I16 => render::build_stream::<i16>(
                &device,
                setup.config,
                setup.first_channel,
                backend,
                consumer,
                spectrum_tx,
                state.clone(),
            ),
            SampleFormat::I32 => render::build_stream::<i32>(
                &device,
                setup.config,
                setup.first_channel,
                backend,
                consumer,
                spectrum_tx,
                state.clone(),
            ),
            SampleFormat::U16 => render::build_stream::<u16>(
                &device,
                setup.config,
                setup.first_channel,
                backend,
                consumer,
                spectrum_tx,
                state.clone(),
            ),
            format => return Err(format!("Formato de salida no compatible: {format:?}")),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Self {
            _stream: stream,
            commands: Mutex::new(producer),
            spectrum: Mutex::new(spectrum_rx),
            state,
            sample_rate,
        })
    }

    pub fn drain_spectrum(&self, analyzer: &mut super::analyzer::SpectrumAnalyzer) {
        if let Ok(mut samples) = self.spectrum.try_lock() {
            for _ in 0..8192 {
                let Ok(sample) = samples.pop() else {
                    break;
                };
                analyzer.push_stereo(sample);
            }
        }
    }

    pub fn set_spectrum_enabled(&self, enabled: bool) {
        self.state
            .spectrum_enabled
            .store(enabled, Ordering::Relaxed);
    }
    fn push(&self, cmd: NoteCommand) {
        if let Ok(mut guard) = self.commands.lock() {
            let _ = guard.push(cmd);
        }
    }
    pub fn sampler_control(&self, cc: u8, value: f32) {
        self.push(NoteCommand::SamplerControl(cc, value));
    }
    pub fn note_on(&self, note: u8, velocity: u8) {
        self.push(NoteCommand::On { note, velocity });
    }
    pub fn note_off(&self, note: u8) {
        self.push(NoteCommand::Off { note });
    }
    pub fn all_notes_off(&self) {
        self.push(NoteCommand::AllOff);
    }
    pub fn set_channel(&self, channel: u8) {
        self.push(NoteCommand::Channel(channel.min(15)));
    }
    pub fn midi(&self, command: u8, data1: u8, data2: u8) {
        self.push(NoteCommand::Midi {
            command,
            data1,
            data2,
        });
    }
    pub fn set_master_volume(&self, volume: f32) {
        self.push(NoteCommand::MasterVolume(volume));
    }
    pub fn set_polyphony(&self, polyphony: usize) {
        self.push(NoteCommand::Polyphony(polyphony));
    }
    pub fn set_effects(&self, enabled: bool) {
        self.push(NoteCommand::Effects(enabled));
    }
    pub fn set_filter(&self, settings: elevate::devices::filter::FilterSettings) {
        self.push(NoteCommand::Filter(settings));
    }
    /// Compila `pattern` fuera del callback y ordena su reproducción sample-accurate.
    pub fn play_pattern(
        &self,
        pattern: &Pattern,
        offset_beats: f64,
        bpm: f64,
        looped: bool,
    ) -> Result<(), String> {
        let time_base = TimeBase::new(self.sample_rate, bpm).map_err(|e| e.to_string())?;
        let plan =
            Arc::new(PreparedPattern::compile(pattern, time_base).map_err(|e| e.to_string())?);
        let offset_ticks = Tick((offset_beats.max(0.0) * PPQ as f64).round() as i64);
        let start_frame = time_base
            .frame_at(offset_ticks)
            .map_err(|e| e.to_string())?;
        let start_frame = if looped {
            start_frame % plan.length_frames().max(1)
        } else {
            start_frame
        };
        let start_tick =
            (start_frame as f64 / self.sample_rate as f64 * bpm / 60.0 * PPQ as f64).round() as i64;
        let mut chased = std::collections::BTreeMap::new();
        for point in pattern
            .controls()
            .points()
            .iter()
            .filter(|point| point.at.0 < start_tick)
        {
            chased.insert(point.lane, point.value);
        }
        self.state.plan_frame.store(start_frame, Ordering::Relaxed);
        self.state.plan_finished.store(false, Ordering::Relaxed);
        self.state.plan_playing.store(true, Ordering::Relaxed);
        self.push(NoteCommand::PlayPlan {
            plan,
            start_frame,
            looped,
        });
        for (lane, value) in chased {
            let (command, a, b) = lane.midi(value);
            self.midi(command, a, b);
        }
        Ok(())
    }
    pub fn stop_pattern(&self) {
        self.state.plan_playing.store(false, Ordering::Relaxed);
        self.state.plan_finished.store(false, Ordering::Relaxed);
        self.push(NoteCommand::StopPlan);
    }
    pub fn plan_playing(&self) -> bool {
        self.state.plan_playing.load(Ordering::Relaxed)
    }
    pub fn plan_finished(&self) -> bool {
        self.state.plan_finished.load(Ordering::Relaxed)
    }
    pub fn plan_frame(&self) -> u64 {
        self.state.plan_frame.load(Ordering::Relaxed)
    }
    pub fn set_fx_rack(&self, rack: Option<std::sync::Arc<crate::audio::effects::Rack>>) {
        self.state.fx.set_rack(rack, self.sample_rate);
    }
    pub fn set_fx_gain(&self, gain: f32) {
        self.state.fx.set(gain);
    }
    pub fn set_gain(&self, gain: f32) {
        self.state
            .gain
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }
    pub fn set_pan(&self, pan: f32) {
        self.state.pan.set(pan);
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}
