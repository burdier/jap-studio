//! Control del editor independiente de GPUI y de la representación de parámetros.
mod frame;
mod handler;
pub(super) mod session;
mod stream;
mod window;
mod x11;
pub(super) use handler::{Edit, Handler};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub(super) enum Command {
    Show,
    Hide,
}
#[derive(Clone)]
pub(crate) struct EditorHandle {
    pub(super) commands: mpsc::SyncSender<Command>,
    pub(super) closed: Arc<AtomicBool>,
    pub(super) message: Arc<Mutex<Option<String>>>,
    pub(super) join: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
impl EditorHandle {
    pub fn open(&self) {
        let _ = self.commands.try_send(Command::Show);
    }
    pub fn close(&self) {
        let _ = self.commands.try_send(Command::Hide);
    }
    pub fn take_message(&self) -> Option<String> {
        self.message.lock().ok()?.take()
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
