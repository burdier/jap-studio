//! Adaptador provisional RustySynth → Volume → CPAL. La UI solo usa esta fachada.
mod render;
mod soundfont;

use super::output::choose_output_config;
use cpal::{
    SampleFormat,
    traits::{HostTrait, StreamTrait},
};
use elevate::musical::{
    model::Pattern,
    scheduler::PreparedPattern,
    time::{PPQ, Tick, TimeBase},
};
pub(crate) use soundfont::{
    PresetEntry, SoundFontBank, discover as discover_soundfonts, load_path as load_soundfont,
    preload, presets_of,
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Sender},
    },
};

#[derive(Clone)]
enum NoteCommand {
    On {
        note: u8,
        velocity: u8,
    },
    Off {
        note: u8,
    },
    AllOff,
    /// Mensaje MIDI de canal tal cual lo entiende RustySynth.
    Midi {
        command: u8,
        data1: u8,
        data2: u8,
    },
    MasterVolume(f32),
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
    bypass: AtomicBool,
    failed: AtomicBool,
    plan_frame: AtomicU64,
    plan_playing: AtomicBool,
    plan_finished: AtomicBool,
}

pub(crate) struct PianoPlayer {
    _stream: cpal::Stream,
    commands: Sender<NoteCommand>,
    state: Arc<InstrumentState>,
    pub sample_rate: u32,
}

impl PianoPlayer {
    pub fn start(
        soundfont_path: &Path,
        gain: f32,
        bypass: bool,
        settings: PianoSettings,
    ) -> Result<Self, String> {
        let sound_font = soundfont::load_path(soundfont_path)?;
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No hay dispositivo de salida predeterminado")?;
        let config = choose_output_config(&device, None)?;
        let sample_rate = config.sample_rate();
        let state = Arc::new(InstrumentState {
            gain: AtomicU32::new(gain.clamp(0.0, 2.0).to_bits()),
            bypass: AtomicBool::new(bypass),
            failed: AtomicBool::new(false),
            plan_frame: AtomicU64::new(0),
            plan_playing: AtomicBool::new(false),
            plan_finished: AtomicBool::new(false),
        });
        // Adaptador heredado. Sustituir por una cola RT acotada al unificar el motor.
        let (commands, receiver) = mpsc::channel();
        let stream = match config.sample_format() {
            SampleFormat::F32 => render::build_stream::<f32>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
                settings,
            ),
            SampleFormat::I16 => render::build_stream::<i16>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
                settings,
            ),
            SampleFormat::I32 => render::build_stream::<i32>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
                settings,
            ),
            SampleFormat::U16 => render::build_stream::<u16>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
                settings,
            ),
            format => return Err(format!("Formato de salida no compatible: {format:?}")),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Self {
            _stream: stream,
            commands,
            state,
            sample_rate,
        })
    }
    pub fn note_on(&self, note: u8, velocity: u8) {
        let _ = self.commands.send(NoteCommand::On { note, velocity });
    }
    pub fn note_off(&self, note: u8) {
        let _ = self.commands.send(NoteCommand::Off { note });
    }
    pub fn all_notes_off(&self) {
        let _ = self.commands.send(NoteCommand::AllOff);
    }
    pub fn midi(&self, command: u8, data1: u8, data2: u8) {
        let _ = self.commands.send(NoteCommand::Midi {
            command,
            data1,
            data2,
        });
    }
    pub fn set_master_volume(&self, volume: f32) {
        let _ = self.commands.send(NoteCommand::MasterVolume(volume));
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
        let _ = self
            .commands
            .send(NoteCommand::PlayPlan { plan, start_frame });
        Ok(())
    }
    pub fn stop_pattern(&self) {
        self.state.plan_playing.store(false, Ordering::Relaxed);
        self.state.plan_finished.store(false, Ordering::Relaxed);
        let _ = self.commands.send(NoteCommand::StopPlan);
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
    pub fn set_gain(&self, gain: f32) {
        self.state
            .gain
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }
    pub fn set_bypass(&self, bypass: bool) {
        self.state.bypass.store(bypass, Ordering::Relaxed);
    }
    pub fn failed(&self) -> bool {
        self.state.failed.load(Ordering::Relaxed)
    }
}
