//! Control del editor independiente de GPUI y de la representación de parámetros.
#[cfg(target_os = "linux")]
mod frame;
mod handler;
mod latency;
pub(super) mod session;
#[cfg(target_os = "linux")]
mod shortcut;
mod state;
mod stream;
pub(crate) use state::{PendingState, PluginState};
mod window;
#[cfg(target_os = "linux")]
mod x11;
use elevate::musical::fx::VstPreset;
pub(super) use handler::{Edit, Handler};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    mpsc,
};

#[derive(Clone, Copy)]
pub(crate) enum EditorMidiKey {
    Key { key: u8, down: bool, repeat: bool },
    Release,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VstParamInfo {
    pub id: u32,
    pub title: String,
    pub units: String,
    pub normalized: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VstTouchedParam {
    pub sequence: u64,
    pub id: u32,
    pub title: String,
    pub units: String,
    pub normalized: f64,
}

pub(super) enum Command {
    WatchParameter(&'static str, mpsc::Sender<Result<Arc<AtomicU64>, String>>),
    WatchState(Option<VstPreset>, mpsc::Sender<Result<(), String>>),
    ListParameters(mpsc::Sender<Result<Vec<VstParamInfo>, String>>),
    SetParameter(u32, f64),
    Show,
    Hide,
    Capture(mpsc::Sender<Result<PluginState, String>>),
}
#[derive(Clone)]
pub(crate) struct EditorHandle {
    project_preset: Arc<Mutex<Option<Result<VstPreset, String>>>>,
    pub(super) last_touched: Arc<Mutex<Option<VstTouchedParam>>>,
    pub(super) latency: Arc<latency::ProcessorLatency>,
    pub(super) commands: mpsc::SyncSender<Command>,
    pub(super) closed: Arc<AtomicBool>,
    pub(super) message: Arc<Mutex<Option<String>>>,
    pub(super) transport_toggles: Arc<AtomicU32>,
    pub(super) midi_enabled: Arc<AtomicBool>,
    pub(super) midi_keys: Arc<Mutex<VecDeque<EditorMidiKey>>>,
    pub(super) join: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
impl EditorHandle {
    pub fn last_touched(&self) -> Option<VstTouchedParam> {
        self.last_touched.lock().ok()?.clone()
    }
    pub fn list_parameters(&self) -> Result<Vec<VstParamInfo>, String> {
        let (send, receive) = mpsc::channel();
        self.commands
            .try_send(Command::ListParameters(send))
            .map_err(|e| e.to_string())?;
        receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?
    }
    pub fn set_parameter(&self, id: u32, normalized: f64) {
        if normalized.is_finite() {
            let _ = self
                .commands
                .try_send(Command::SetParameter(id, normalized.clamp(0.0, 1.0)));
        }
    }
    pub fn latency_samples(&self) -> u32 {
        self.latency.samples()
    }
    pub fn take_latency_change(&self) -> bool {
        self.latency.take_change()
    }
    pub fn watch_state(&self, saved: Option<VstPreset>) -> Result<(), String> {
        let (send, receive) = mpsc::channel();
        self.commands
            .try_send(Command::WatchState(saved, send))
            .map_err(|e| e.to_string())?;
        receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?
    }
    pub fn project_preset(&self) -> Result<VstPreset, String> {
        self.project_preset
            .lock()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or("Estado VST3 sin capturar")?
    }
    pub fn capture(&self) -> Result<PendingState, String> {
        let (send, receive) = mpsc::channel();
        self.commands
            .try_send(Command::Capture(send))
            .map_err(|e| format!("VST3: solicitar estado ({e})"))?;
        Ok(receive)
    }
    /// Se solicita desde el cargador de efectos. La UI solo lee el valor publicado.
    pub fn watch_parameter(&self, title: &'static str) -> Result<Arc<AtomicU64>, String> {
        let (send, receive) = mpsc::channel();
        self.commands
            .try_send(Command::WatchParameter(title, send))
            .map_err(|e| e.to_string())?;
        receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?
    }
    pub fn open(&self) {
        let _ = self.commands.try_send(Command::Show);
    }
    pub fn set_midi_enabled(&self, enabled: bool) {
        self.midi_enabled.store(enabled, Ordering::Release);
    }
    pub fn drain_midi_keys(&self, output: &mut Vec<EditorMidiKey>) {
        if let Ok(mut keys) = self.midi_keys.lock() {
            output.extend(keys.drain(..));
        }
    }
    pub(super) fn push_midi_key(&self, key: EditorMidiKey) {
        if let Ok(mut keys) = self.midi_keys.lock() {
            if keys.len() == keys.capacity() {
                keys.clear();
                keys.push_back(EditorMidiKey::Release);
            }
            keys.push_back(key);
        }
    }
    pub fn close(&self) {
        let _ = self.commands.try_send(Command::Hide);
    }
    pub fn take_message(&self) -> Option<String> {
        self.message.lock().ok()?.take()
    }
    pub fn take_transport_toggles(&self) -> u32 {
        self.transport_toggles.swap(0, Ordering::AcqRel)
    }
    /// Diagnóstico síncrono, después de parar el stream. Nunca desde GPUI o RT.
    #[allow(dead_code)]
    pub fn wait_for_shutdown(&self) {
        if self.closed.load(Ordering::Acquire) {
            if let Some(join) = self.join.lock().ok().and_then(|mut join| join.take()) {
                let _ = join.join();
            }
        }
    }
    pub(super) fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
    }
}
