//! Adaptador provisional RustySynth → Volume → CPAL. La UI solo usa esta fachada.
mod render;
mod soundfont;

use super::output::choose_output_config;
use cpal::{
    SampleFormat,
    traits::{HostTrait, StreamTrait},
};
pub(crate) use soundfont::preload;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc::{self, Sender},
};

#[derive(Clone, Copy, Debug)]
enum NoteCommand {
    On { note: u8, velocity: u8 },
    Off { note: u8 },
    AllOff,
}

struct InstrumentState {
    gain: AtomicU32,
    bypass: AtomicBool,
    failed: AtomicBool,
}

pub(crate) struct PianoPlayer {
    _stream: cpal::Stream,
    commands: Sender<NoteCommand>,
    state: Arc<InstrumentState>,
    pub sample_rate: u32,
}

impl PianoPlayer {
    pub fn start(gain: f32, bypass: bool) -> Result<Self, String> {
        let sound_font = soundfont::load()?;
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No hay dispositivo de salida predeterminado")?;
        let config = choose_output_config(&device, None)?;
        let sample_rate = config.sample_rate();
        let state = Arc::new(InstrumentState {
            gain: AtomicU32::new(gain.clamp(0.0, 2.0).to_bits()),
            bypass: AtomicBool::new(bypass),
            failed: AtomicBool::new(false),
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
            ),
            SampleFormat::I16 => render::build_stream::<i16>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
            ),
            SampleFormat::I32 => render::build_stream::<i32>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
            ),
            SampleFormat::U16 => render::build_stream::<u16>(
                &device,
                config.into(),
                sound_font,
                receiver,
                state.clone(),
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
