//! Adaptador SoundFont (`libfluidsynth` C por `dlopen` con respaldo `RustySynth`) → Volume → CPAL.
mod backend;
mod filter;
mod fluidsynth;
mod offline;
mod render;
mod soundfont;

use super::output::choose_output_config;
use super::stereo_pan::StereoPan;
pub(crate) use backend::SynthBackend;
use cpal::{
    SampleFormat,
    traits::{HostTrait, StreamTrait},
};
use elevate::musical::{
    model::Pattern,
    scheduler::PreparedPattern,
    time::{PPQ, Tick, TimeBase},
};
pub(crate) use fluidsynth::is_available as fluidsynth_available;
pub(crate) use offline::{OfflinePiano, PianoSnapshot};
use rtrb::{Producer, RingBuffer};
pub(crate) use soundfont::{
    PresetEntry, SoundFontBank, discover as discover_soundfonts, load_path as load_soundfont,
    preload, presets_of,
};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
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
    MasterVolume(f32),
    Polyphony(usize),
    Effects(bool),
    PlayPlan {
        plan: Arc<PreparedPattern>,
        start_frame: u64,
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
}

pub(crate) struct PianoPlayer {
    _stream: cpal::Stream,
    commands: Mutex<Producer<NoteCommand>>,
    state: Arc<InstrumentState>,
    pub sample_rate: u32,
}

impl PianoPlayer {
    pub fn start(
        soundfont_path: &Path,
        gain: f32,
        utility: elevate::devices::utility_slot::UtilitySlot,
        pan: f32,
        fx_gain: f32,
        settings: PianoSettings,
    ) -> Result<Self, String> {
        let sound_font = soundfont::load_path(soundfont_path)?;
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No hay dispositivo de salida predeterminado")?;
        let config = choose_output_config(&device, None)?;
        let sample_rate = config.sample_rate();
        let backend = SynthBackend::new(soundfont_path, &sound_font, sample_rate, settings)?;
        Self::start_backend(device, config, backend, gain, utility, pan, fx_gain)
    }
    pub fn start_sampler(
        mut engine: crate::audio::sampler::SfizzEngine,
        gain: f32,
        utility: elevate::devices::utility_slot::UtilitySlot,
        pan: f32,
        fx_gain: f32,
    ) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No hay salida de audio")?;
        let config = choose_output_config(&device, None)?;
        engine.set_sample_rate(config.sample_rate());
        Self::start_backend(
            device,
            config,
            SynthBackend::Sampler(engine),
            gain,
            utility,
            pan,
            fx_gain,
        )
    }
    fn start_backend(
        device: cpal::Device,
        config: cpal::SupportedStreamConfig,
        backend: SynthBackend,
        gain: f32,
        utility: elevate::devices::utility_slot::UtilitySlot,
        pan: f32,
        fx_gain: f32,
    ) -> Result<Self, String> {
        let sample_rate = config.sample_rate();
        let state = Arc::new(InstrumentState {
            gain: AtomicU32::new(gain.clamp(0.0, 2.0).to_bits()),
            pan: StereoPan::new(pan),
            fx: super::fx_gain::FxGain::new(fx_gain),
            utility,
            failed: AtomicBool::new(false),
            plan_frame: AtomicU64::new(0),
            plan_playing: AtomicBool::new(false),
            plan_finished: AtomicBool::new(false),
        });
        let (producer, consumer) = RingBuffer::new(COMMAND_QUEUE_CAP);
        let stream = match config.sample_format() {
            SampleFormat::F32 => render::build_stream::<f32>(
                &device,
                config.into(),
                backend,
                consumer,
                state.clone(),
            ),
            SampleFormat::I16 => render::build_stream::<i16>(
                &device,
                config.into(),
                backend,
                consumer,
                state.clone(),
            ),
            SampleFormat::I32 => render::build_stream::<i32>(
                &device,
                config.into(),
                backend,
                consumer,
                state.clone(),
            ),
            SampleFormat::U16 => render::build_stream::<u16>(
                &device,
                config.into(),
                backend,
                consumer,
                state.clone(),
            ),
            format => return Err(format!("Formato de salida no compatible: {format:?}")),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Self {
            _stream: stream,
            commands: Mutex::new(producer),
            state,
            sample_rate,
        })
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
    ) -> Result<(), String> {
        let time_base = TimeBase::new(self.sample_rate, bpm).map_err(|e| e.to_string())?;
        let plan =
            Arc::new(PreparedPattern::compile(pattern, time_base).map_err(|e| e.to_string())?);
        let offset_ticks = Tick((offset_beats.max(0.0) * PPQ as f64).round() as i64);
        let start_frame = time_base
            .frame_at(offset_ticks)
            .map_err(|e| e.to_string())?;
        self.state.plan_frame.store(start_frame, Ordering::Relaxed);
        self.state.plan_finished.store(false, Ordering::Relaxed);
        self.state.plan_playing.store(true, Ordering::Relaxed);
        self.push(NoteCommand::PlayPlan { plan, start_frame });
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
