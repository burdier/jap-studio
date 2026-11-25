//! Apertura, inicialización y cierre de una instancia VST3.
use super::{
    VstEngine, buffers,
    editor::{self, Edit, Handler, session::Session},
    host, module,
};
use std::path::Path;
use vst3::{
    ComWrapper,
    Steinberg::{Vst::*, *},
};

impl VstEngine {
    pub fn load(bundle: &Path, sample_rate: u32) -> Result<Self, String> {
        editor::session::spawn(bundle, sample_rate, None, true, None)
    }
    pub fn load_instrument_class(
        bundle: &Path,
        sample_rate: u32,
        class: Option<String>,
    ) -> Result<Self, String> {
        editor::session::spawn(bundle, sample_rate, class, true, None)
    }
    pub fn load_effect(bundle: &Path, sample_rate: u32, class: &str) -> Result<Self, String> {
        editor::session::spawn(bundle, sample_rate, Some(class.to_owned()), false, None)
    }
}
impl Session {
    pub(crate) fn load(
        bundle: &Path,
        sample_rate: u32,
        edits: rtrb::Producer<Edit>,
        class: Option<&str>,
        instrument: bool,
        state: Option<super::PluginState>,
    ) -> Result<Self, String> {
        let module = module::Module::open(bundle)?;
        let info = module.audio_class(class, instrument)?;
        let class_id = info.id();
        let (cid, name) = (info.cid, info.name);
        let component = module.create::<IComponent>(&cid)?;
        let processor = component
            .cast::<IAudioProcessor>()
            .ok_or("VST3 sin procesador")?;
        let host = ComWrapper::new(host::Host)
            .to_com_ptr::<IHostApplication>()
            .unwrap();
        let handler = ComWrapper::new(Handler::new(edits));
        let mut engine = Self {
            initial_parameters: Vec::new(),
            path: bundle.to_owned(),
            class_id,
            component,
            processor,
            controller: None,
            separate_controller: false,
            initialized: false,
            active: false,
            processing: false,
            handler,
            mapping: [[None; 130]; 16],
            name,
            instrument,
            rate: sample_rate as f64,
            buffers: buffers::Buffers::empty(),
            host,
            _module: module,
        };
        engine.initialize(state)?;
        Ok(engine)
    }
    fn initialize(&mut self, state: Option<super::PluginState>) -> Result<(), String> {
        unsafe {
            check(
                self.component.initialize(self.host.as_ptr().cast()),
                "initialize",
            )?;
            self.initialized = true;
            if let Some(controller) = self.component.cast::<IEditController>() {
                self.controller = Some(controller);
            } else {
                let mut cid = [0; 16];
                if self.component.getControllerClassId(&mut cid) == kResultOk {
                    let controller = self._module.create::<IEditController>(&cid)?;
                    check(
                        controller.initialize(self.host.as_ptr().cast()),
                        "controller.initialize",
                    )?;
                    self.controller = Some(controller);
                    self.separate_controller = true;
                }
            }
            if self.separate_controller {
                if let (Some(a), Some(b)) = (
                    self.component.cast::<IConnectionPoint>(),
                    self.controller
                        .as_ref()
                        .and_then(|c| c.cast::<IConnectionPoint>()),
                ) {
                    a.connect(b.as_ptr());
                    b.connect(a.as_ptr());
                }
            }
            if let Some(controller) = &self.controller {
                let handler = self.handler.to_com_ptr::<IComponentHandler>().unwrap();
                check(
                    controller.setComponentHandler(handler.as_ptr()),
                    "editor handler",
                )?;
                if self.separate_controller && state.is_none() {
                    editor::session::sync_state(&self.component, controller)?;
                }
            }
            if let Some(state) = state {
                self.restore(state)?;
            }
            if let Some(mapping) = self
                .controller
                .as_ref()
                .and_then(|c| c.cast::<IMidiMapping>())
            {
                for channel in 0..16 {
                    for cc in 0..130 {
                        let mut id = 0;
                        if mapping.getMidiControllerAssignment(
                            0,
                            channel as i16,
                            cc as i16,
                            &mut id,
                        ) == kResultOk
                        {
                            self.mapping[channel][cc] = Some(id);
                        }
                    }
                }
            }
            let input = BusDirections_::kInput as i32;
            self.buffers = buffers::Buffers::prepare(&self.component, !self.instrument)?;
            check(self.processor.canProcessSampleSize(0), "procesamiento f32")?;
            let event = MediaTypes_::kEvent as i32;
            if self.instrument && self.component.getBusCount(event, input) < 1 {
                return Err("VST3 sin entrada MIDI".into());
            }
            if self.component.getBusCount(event, input) > 0 {
                check(
                    self.component.activateBus(event, input, 0, 1),
                    "activar MIDI",
                )?;
            }
            let mut setup = ProcessSetup {
                processMode: 0,
                symbolicSampleSize: 0,
                maxSamplesPerBlock: 512,
                sampleRate: self.rate,
            };
            check(
                self.processor.setupProcessing(&mut setup),
                "setupProcessing",
            )?;
            self.activate()?;
        }
        Ok(())
    }
    fn activate(&mut self) -> Result<(), String> {
        unsafe {
            check(self.component.setActive(1), "setActive")?;
            self.active = true;
            // The SDK's default AudioEffect leaves this notification unimplemented.
            let result = self.processor.setProcessing(1);
            if result != kNotImplemented {
                check(result, "setProcessing")?;
                self.processing = true;
            }
        }
        Ok(())
    }
    pub(super) fn reactivate_latency(&mut self) -> Result<u32, String> {
        unsafe {
            if self.processing {
                check(self.processor.setProcessing(0), "suspender DSP")?;
                self.processing = false;
            }
            check(self.component.setActive(0), "desactivar DSP")?;
            self.active = false;
            self.activate()?;
            Ok(self.processor.getLatencySamples())
        }
    }
}
pub(super) fn check(result: tresult, operation: &str) -> Result<(), String> {
    if result == kResultOk {
        Ok(())
    } else {
        Err(format!("VST3: {operation} falló ({result})"))
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            if self.processing {
                self.processor.setProcessing(0);
            }
            if self.active {
                self.component.setActive(0);
            }
            if let Some(controller) = &self.controller {
                controller.setComponentHandler(std::ptr::null_mut());
            }
            if self.separate_controller {
                if let (Some(a), Some(b)) = (
                    self.component.cast::<IConnectionPoint>(),
                    self.controller
                        .as_ref()
                        .and_then(|c| c.cast::<IConnectionPoint>()),
                ) {
                    a.disconnect(b.as_ptr());
                    b.disconnect(a.as_ptr());
                }
                if let Some(controller) = &self.controller {
                    controller.terminate();
                }
            }
            if self.initialized {
                self.component.terminate();
            }
        }
    }
}
