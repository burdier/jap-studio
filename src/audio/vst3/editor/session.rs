//! Un hilo propietario para lifecycle/editor. El callback solo posee el procesador RT.
use super::super::{
    VstEngine, buffers::Buffers, events::Events, module::Module, parameters::Changes,
};
use super::{Command, EditorHandle, Handler, stream::Stream, window::Window};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use vst3::{
    ComPtr, ComWrapper,
    Steinberg::{Vst::*, *},
};

pub(crate) struct Session {
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
) -> Result<VstEngine, String> {
    let path = bundle.to_owned();
    let (ready, receive) = mpsc::sync_channel(1);
    let (commands, incoming) = mpsc::sync_channel(16);
    let handle = EditorHandle {
        commands,
        closed: Arc::new(AtomicBool::new(false)),
        message: Arc::new(Mutex::new(None)),
        join: Arc::new(Mutex::new(None)),
    };
    let owner = handle.clone();
    let join = std::thread::Builder::new()
        .name("vst3-editor".into())
        .spawn(move || {
            let (producer, consumer) = rtrb::RingBuffer::new(4096);
            let mut session =
                match Session::load(&path, sample_rate, producer, class.as_deref(), instrument) {
                    Ok(session) => session,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
            let events = ComWrapper::new(Events::new());
            let changes = ComWrapper::new(Changes::new());
            let engine = VstEngine {
                processor: Some(session.processor.clone()),
                editor: handle.clone(),
                edits: consumer,
                events_interface: events.to_com_ptr().unwrap(),
                events,
                changes_interface: changes.to_com_ptr().unwrap(),
                changes,
                mapping: session.mapping,
                held: [[false; 128]; 16],
                name: session.name.clone(),
                failed: false,
                rate: sample_rate as f64,
                frame: 0,
                buffers: std::mem::replace(&mut session.buffers, Buffers::empty()),
            };
            if ready.send(Ok(engine)).is_err() {
                return;
            }
            session.run(incoming, &handle);
        })
        .map_err(|e| e.to_string())?;
    *owner.join.lock().map_err(|e| e.to_string())? = Some(join);
    receive
        .recv()
        .map_err(|e| format!("El hilo VST3 terminó durante la carga: {e}"))?
}
impl Session {
    fn run(&mut self, incoming: mpsc::Receiver<Command>, handle: &EditorHandle) {
        let mut window: Option<Window> = None;
        while !handle.closed.load(Ordering::Acquire) {
            match incoming.recv_timeout(Duration::from_millis(8)) {
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
                            Err(error) => self.message(handle, error),
                        }
                    }
                }
                Ok(Command::Hide) => {
                    window = None;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if window.as_mut().is_some_and(|window| !window.tick()) {
                window = None;
            }
            let flags = self.handler.restart.swap(0, Ordering::Relaxed);
            if flags & 4 != 0 {
                self.sync_parameters();
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
    fn sync_parameters(&self) {
        let Some(controller) = &self.controller else {
            return;
        };
        unsafe {
            for index in 0..controller.getParameterCount().clamp(0, 4096) {
                let mut info: ParameterInfo = std::mem::zeroed();
                if controller.getParameterInfo(index, &mut info) == kResultOk {
                    self.handler
                        .performEdit(info.id, controller.getParamNormalized(info.id));
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
