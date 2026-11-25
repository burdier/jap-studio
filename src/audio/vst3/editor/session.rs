//! Un hilo propietario para lifecycle/editor. El callback solo posee el procesador RT.
use super::super::{
    VstEngine, buffers::Buffers, events::Events, module::Module, parameters::Changes,
};
use super::{Command, EditorHandle, Handler, stream::Stream, window::Window};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use vst3::{
    ComPtr, ComWrapper,
    Steinberg::{Vst::*, *},
};

pub(crate) struct Session {
    pub initial_parameters: Vec<(ParamID, f64)>,
    pub path: std::path::PathBuf,
    pub class_id: String,
    pub component: ComPtr<IComponent>,
    pub processor: ComPtr<IAudioProcessor>,
    pub controller: Option<ComPtr<IEditController>>,
    pub separate_controller: bool,
    pub initialized: bool,
    pub active: bool,
    pub processing: bool,
    pub name: String,
    pub instrument: bool,
    pub rate: f64,
    pub buffers: Buffers,
    pub mapping: [[Option<u32>; 130]; 16],
    pub host: ComPtr<IHostApplication>,
    pub handler: ComWrapper<Handler>,
    pub _module: Module,
}
pub(crate) fn spawn(
    bundle: &Path,
    sample_rate: u32,
    class: Option<String>,
    instrument: bool,
    state: Option<super::PluginState>,
) -> Result<VstEngine, String> {
    let path = bundle.to_owned();
    let (ready, receive) = mpsc::sync_channel(1);
    let (commands, incoming) = mpsc::sync_channel(64);
    let handle = EditorHandle {
        project_preset: Default::default(),
        last_touched: Default::default(),
        latency: Default::default(),
        commands,
        closed: Arc::new(AtomicBool::new(false)),
        message: Arc::new(Mutex::new(None)),
        transport_toggles: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        midi_enabled: Arc::new(AtomicBool::new(false)),
        midi_keys: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(128))),
        join: Arc::new(Mutex::new(None)),
    };
    let owner = handle.clone();
    let join = std::thread::Builder::new()
        .name("vst3-editor".into())
        .spawn(move || {
            let (producer, consumer) = rtrb::RingBuffer::new(4096);
            let mut session = match Session::load(
                &path,
                sample_rate,
                producer,
                class.as_deref(),
                instrument,
                state,
            ) {
                Ok(session) => session,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            let restore_parameters = !session.initial_parameters.is_empty();
            let events = ComWrapper::new(Events::new());
            let changes = ComWrapper::new(Changes::new(256 + session.initial_parameters.len()));
            for (id, value) in session.initial_parameters.drain(..) {
                changes.set(id, value);
            }
            let feedback = ComWrapper::new(Changes::new(256));
            let (to_controller, feedback_rx) = rtrb::RingBuffer::new(4096);
            let mut engine = VstEngine {
                export_latency: None,
                feedback_interface: feedback.to_com_ptr().unwrap(),
                feedback,
                to_controller,
                processor: Some(session.processor.clone()),
                editor: handle.clone(),
                edits: consumer,
                applied_edit: session.handler.applied.clone(),
                events_interface: events.to_com_ptr().unwrap(),
                events,
                changes_interface: changes.to_com_ptr().unwrap(),
                changes,
                mapping: session.mapping,
                mpe_pitch: session.instrument
                    && super::super::note_pitch::supports_mpe_pitch(&session.mapping),
                mpe_voices: [None; 15],
                held: [[false; 128]; 16],
                name: session.name.clone(),
                failed: false,
                rate: sample_rate as f64,
                frame: 0,
                buffers: std::mem::replace(&mut session.buffers, Buffers::empty()),
            };
            // Los presets basados en parámetros llegan al DSP por process. Prepara un
            // frame silencioso y reinicia sus buffers antes de entregar la instancia.
            let latency = if restore_parameters {
                engine.render(&mut [0.], &mut [0.]);
                engine.frame = 0;
                if engine.failed {
                    Err(format!(
                        "{}: fallo al restaurar parámetros VST3",
                        session.name
                    ))
                } else {
                    session.reactivate_latency()
                }
            } else {
                Ok(unsafe { session.processor.getLatencySamples() })
            };
            match latency {
                Ok(samples) => handle.latency.set_initial(samples),
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            }
            // Una notificación emitida al activar ya está incluida en la lectura inicial.
            session
                .handler
                .restart
                .fetch_and(!RestartFlags_::kLatencyChanged, Ordering::AcqRel);
            if ready.send(Ok(engine)).is_err() {
                return;
            }
            session.run(incoming, &handle, feedback_rx);
        })
        .map_err(|e| e.to_string())?;
    *owner.join.lock().map_err(|e| e.to_string())? = Some(join);
    receive
        .recv()
        .map_err(|e| format!("El hilo VST3 terminó durante la carga: {e}"))?
}
impl Session {
    fn run(
        &mut self,
        incoming: mpsc::Receiver<Command>,
        handle: &EditorHandle,
        mut feedback: rtrb::Consumer<(u32, f64)>,
    ) {
        let mut preset_revision = None;
        let mut last_touch_seq = 0u64;
        let mut watched: Option<(ParamID, Arc<AtomicU64>)> = None;
        let mut window: Option<Window> = None;
        while !handle.closed.load(Ordering::Acquire) {
            match incoming.recv_timeout(Duration::from_millis(8)) {
                Ok(Command::WatchParameter(title, reply)) => {
                    let id = self.controller.as_ref().and_then(|c| unsafe {
                        (0..c.getParameterCount().clamp(0, 4096)).find_map(|index| {
                            let mut info: ParameterInfo = std::mem::zeroed();
                            if c.getParameterInfo(index, &mut info) != kResultOk {
                                return None;
                            }
                            let end = info
                                .title
                                .iter()
                                .position(|&v| v == 0)
                                .unwrap_or(info.title.len());
                            info.title[..end]
                                .iter()
                                .copied()
                                .eq(title.encode_utf16())
                                .then_some(info.id)
                        })
                    });
                    let result = id
                        .map(|id| {
                            let value = Arc::new(AtomicU64::new(0));
                            watched = Some((id, value.clone()));
                            value
                        })
                        .ok_or_else(|| format!("El VST3 no publica {title}"));
                    let _ = reply.send(result);
                }
                Ok(Command::WatchState(saved, reply)) => {
                    let rev = self.handler.revision();
                    let result = saved
                        .map(Ok)
                        .unwrap_or_else(|| self.capture().and_then(|s| s.project_preset()));
                    let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
                    *handle.project_preset.lock().unwrap() = Some(result);
                    preset_revision = Some(rev);
                    let _ = reply.send(status);
                }
                Ok(Command::Capture(reply)) => {
                    let _ = reply.send(self.capture().map_err(|e| format!("{}: {e}", self.name)));
                }
                Ok(Command::ListParameters(reply)) => {
                    let _ = reply.send(self.list_parameters());
                }
                Ok(Command::SetParameter(id, value)) => {
                    let value = value.clamp(0.0, 1.0);
                    if let Some(controller) = &self.controller {
                        unsafe {
                            controller.setParamNormalized(id, value);
                        }
                    }
                    self.handler.push_host_edit(id, value);
                }
                Ok(Command::Show) => {
                    if let Some(window) = &window {
                        window.raise();
                    } else {
                        let result = self
                            .controller
                            .as_ref()
                            .ok_or("Plugin sin controlador de editor".to_owned())
                            .and_then(|controller| Window::open(controller, &self.name));
                        match result {
                            Ok(created) => {
                                window = Some(created);
                                self.message(handle, "Editor original abierto".into());
                            }
                            Err(error) => {
                                self.message(handle, error);
                            }
                        }
                    }
                }
                Ok(Command::Hide) => {
                    if window.take().is_some() {
                        handle.push_midi_key(super::EditorMidiKey::Release);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            for _ in 0..feedback.slots() {
                let Ok((id, normalized)) = feedback.pop() else {
                    break;
                };
                if let Some(controller) = &self.controller {
                    unsafe {
                        controller.setParamNormalized(id, normalized);
                        if let Some((param, value)) = &watched
                            && *param == id
                        {
                            value.store(
                                controller.normalizedParamToPlain(id, normalized).to_bits(),
                                Ordering::Relaxed,
                            );
                        }
                    }
                }
            }
            if window.as_mut().is_some_and(|window| !window.tick(handle)) {
                window = None;
                handle.push_midi_key(super::EditorMidiKey::Release);
            }
            let flags = self.handler.restart.swap(0, Ordering::Relaxed);
            if flags & RestartFlags_::kLatencyChanged != 0 {
                if let Err(error) = handle.latency.restart(|| {
                    let samples = self.reactivate_latency()?;
                    // Algunos plugins vuelven a anunciar su latencia en setActive.
                    self.handler
                        .restart
                        .fetch_and(!RestartFlags_::kLatencyChanged, Ordering::AcqRel);
                    Ok(samples)
                }) {
                    self.message(handle, error);
                }
            }
            if flags & 4 != 0 {
                self.sync_parameters();
            }
            if let Some((seq, id, normalized)) = self.handler.last_touched()
                && seq != last_touch_seq
            {
                last_touch_seq = seq;
                let (title, units) = self.parameter_meta(id);
                if let Ok(mut slot) = handle.last_touched.lock() {
                    *slot = Some(super::VstTouchedParam {
                        sequence: seq,
                        id,
                        title,
                        units,
                        normalized,
                    });
                }
            }
            if let Some(last) = preset_revision {
                let revision = self.handler.revision();
                if revision != last || flags & 4 != 0 {
                    let result = self.capture().and_then(|s| s.project_preset());
                    *handle.project_preset.lock().unwrap() = Some(result);
                    preset_revision = Some(revision);
                }
            }
            if flags & (1 | 2 | 16) != 0 {
                self.message(
                    handle,
                    "El plugin solicita cambiar su configuración DSP; vuelve a cargarlo desde [+]"
                        .into(),
                );
            }
        }
        // Destruir view/frame antes de desconectar y terminar componente/controlador.
        drop(window);
    }
    fn list_parameters(&self) -> Result<Vec<super::VstParamInfo>, String> {
        let Some(controller) = &self.controller else {
            return Ok(Vec::new());
        };
        let mut list = Vec::new();
        unsafe {
            for index in 0..controller.getParameterCount().clamp(0, 4096) {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(index, &mut info) != kResultOk {
                    continue;
                }
                let hidden_or_ro = (ParameterInfo_::ParameterFlags_::kIsReadOnly
                    | ParameterInfo_::ParameterFlags_::kIsHidden)
                    as i32;
                if info.flags & hidden_or_ro != 0 {
                    continue;
                }
                let title = utf16_cstr(&info.title);
                if title.is_empty() {
                    continue;
                }
                list.push(super::VstParamInfo {
                    id: info.id,
                    title,
                    units: utf16_cstr(&info.units),
                    normalized: controller.getParamNormalized(info.id).clamp(0.0, 1.0),
                });
            }
        }
        Ok(list)
    }

    fn parameter_meta(&self, id: ParamID) -> (String, String) {
        let Some(controller) = &self.controller else {
            return (format!("Param #{id}"), String::new());
        };
        unsafe {
            for index in 0..controller.getParameterCount().clamp(0, 4096) {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(index, &mut info) == kResultOk && info.id == id {
                    let title = utf16_cstr(&info.title);
                    let units = utf16_cstr(&info.units);
                    return (
                        if title.is_empty() {
                            format!("Param #{id}")
                        } else {
                            title
                        },
                        units,
                    );
                }
            }
        }
        (format!("Param #{id}"), String::new())
    }

    fn sync_parameters(&self) {
        let Some(controller) = &self.controller else {
            return;
        };
        unsafe {
            for index in 0..controller.getParameterCount().clamp(0, 4096) {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(index, &mut info) == kResultOk {
                    self.handler
                        .push_host_edit(info.id, controller.getParamNormalized(info.id));
                }
            }
        }
    }
    fn message(&self, handle: &EditorHandle, text: String) {
        if let Ok(mut message) = handle.message.lock() {
            *message = Some(text);
        }
    }
}
fn utf16_cstr(slice: &[u16]) -> String {
    let end = slice.iter().position(|&v| v == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end]).trim().to_string()
}

pub(crate) fn sync_state(
    component: &ComPtr<IComponent>,
    controller: &ComPtr<IEditController>,
) -> Result<(), String> {
    let stream = ComWrapper::new(Stream::default())
        .to_com_ptr::<IBStream>()
        .unwrap();
    unsafe {
        if component.getState(stream.as_ptr()) == kResultOk {
            stream.seek(0, 0, std::ptr::null_mut());
            let result = controller.setComponentState(stream.as_ptr());
            if result != kResultOk && result != kNotImplemented {
                return Err(format!("VST3: sincronizar controlador ({result})"));
            }
        }
    }
    Ok(())
}
