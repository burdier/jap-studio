//! Host VST3 de instrumentos estéreo; wrapper Linux nativo o yabridge.
//! El dueño es exclusivo: preparación fuera de RT, process en el callback, cierre
//! después de parar el stream. Los objetos COM se liberan antes del módulo.
mod buffers;
mod context;
pub(crate) use context::TransportContext;
mod editor;
mod events;
pub(crate) use editor::{EditorHandle, EditorMidiKey, PendingState, PluginState};
pub(crate) mod environment;
mod host;
mod lifecycle;
mod message;
mod module;
mod note_pitch;
mod parameters;
mod process;
pub(crate) mod scan;
mod shared;
use lifecycle::check;
pub(crate) use shared::{InstrumentVoice, SharedInstrument};

use events::Events;
use parameters::Changes;
use vst3::{ComPtr, ComWrapper, Steinberg::Vst::*};

pub(crate) struct VstEngine {
    processor: Option<ComPtr<IAudioProcessor>>,
    pub editor: EditorHandle,
    edits: rtrb::Consumer<editor::Edit>,
    applied_edit: std::sync::Arc<std::sync::atomic::AtomicU64>,
    events: ComWrapper<Events>,
    events_interface: ComPtr<IEventList>,
    changes: ComWrapper<Changes>,
    changes_interface: ComPtr<IParameterChanges>,
    feedback: ComWrapper<Changes>,
    feedback_interface: ComPtr<IParameterChanges>,
    to_controller: rtrb::Producer<(u32, f64)>,
    mapping: [[Option<u32>; 130]; 16],
    mpe_pitch: bool,
    mpe_voices: [Option<note_pitch::MpeVoice>; 15],
    held: [[bool; 128]; 16],
    pub name: String,
    pub failed: bool,
    export_latency: Option<u32>,
    rate: f64,
    frame: i64,
    buffers: buffers::Buffers,
}
// Se transfiere únicamente el procesador al stream. El controlador y su editor
// permanecen en el hilo propietario; los cambios cruzan por una cola SPSC.
unsafe impl Send for VstEngine {}

impl Drop for VstEngine {
    fn drop(&mut self) {
        // Liberar la referencia RT ANTES de permitir al hilo propietario descargar el módulo.
        self.processor.take();
        self.editor.shutdown();
    }
}
