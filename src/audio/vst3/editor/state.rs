//! Captura en el hilo propietario del plugin. El worker abre una instancia independiente.
use super::super::VstEngine;
use super::{
    session::{Session, spawn},
    stream::Stream,
};
use elevate::musical::fx::VstPreset;
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, mpsc},
};
use vst3::{
    ComWrapper,
    Steinberg::{Vst::*, *},
};

pub(crate) type PendingState = mpsc::Receiver<Result<PluginState, String>>;
pub(crate) struct PluginState {
    path: PathBuf,
    class: String,
    instrument: bool,
    component: Option<Vec<u8>>,
    controller: Option<Vec<u8>>,
    pending: Vec<(ParamID, f64)>,
}
impl PluginState {
    pub fn project_preset(self) -> Result<VstPreset, String> {
        let mut bytes = b"JVS1".to_vec();
        for chunk in [self.component, self.controller] {
            let length = chunk.as_ref().map_or(u32::MAX, |c| c.len() as u32);
            bytes.extend_from_slice(&length.to_le_bytes());
            if let Some(chunk) = chunk {
                bytes.extend_from_slice(&chunk);
            }
        }
        for (id, value) in self.pending {
            if !(0.0..=1.).contains(&value) {
                return Err("Parámetro VST3 fuera de rango".into());
            }
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        VstPreset::new(bytes)
    }

    pub fn from_effect_preset(
        path: PathBuf,
        class: String,
        preset: &VstPreset,
    ) -> Result<Self, String> {
        Self::from_preset(path, class, false, preset)
    }

    pub fn from_instrument_preset(
        path: PathBuf,
        class: String,
        preset: &VstPreset,
    ) -> Result<Self, String> {
        Self::from_preset(path, class, true, preset)
    }

    fn from_preset(
        path: PathBuf,
        class: String,
        instrument: bool,
        preset: &VstPreset,
    ) -> Result<Self, String> {
        let mut data = preset
            .bytes()
            .strip_prefix(b"JVS1")
            .ok_or("Versión de estado VST3 desconocida")?;
        let mut chunk = || -> Result<Option<Vec<u8>>, String> {
            let (size, rest) = data.split_at_checked(4).ok_or("Estado VST3 truncado")?;
            let size = u32::from_le_bytes(size.try_into().unwrap());
            data = rest;
            if size == u32::MAX {
                return Ok(None);
            }
            let (bytes, rest) = data
                .split_at_checked(size as usize)
                .ok_or("Bloque VST3 truncado")?;
            data = rest;
            Ok(Some(bytes.to_vec()))
        };
        let component = chunk()?;
        let controller = chunk()?;
        if data.len() % 12 != 0 || data.len() / 12 > 4096 {
            return Err("Lista de parámetros VST3 inválida".into());
        }
        let mut pending = Vec::new();
        for entry in data.chunks_exact(12) {
            let id = u32::from_le_bytes(entry[..4].try_into().unwrap());
            let value = f64::from_le_bytes(entry[4..].try_into().unwrap());
            if !(0.0..=1.).contains(&value) {
                return Err("Parámetro VST3 fuera de rango".into());
            }
            pending.push((id, value));
        }
        Ok(Self {
            path,
            class,
            instrument,
            component,
            controller,
            pending,
        })
    }
    pub fn instantiate(self, rate: u32) -> Result<VstEngine, String> {
        let path = self.path.clone();
        let class = self.class.clone();
        let instrument = self.instrument;
        spawn(&path, rate, Some(class), instrument, Some(self))
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}
fn read_state(
    get: impl FnOnce(*mut IBStream) -> tresult,
    optional: bool,
) -> Result<Option<Vec<u8>>, String> {
    let stream = ComWrapper::new(Stream::default());
    let interface = stream.to_com_ptr::<IBStream>().unwrap();
    let result = get(interface.as_ptr());
    if result == kResultOk {
        return Ok(Some(stream.take_bytes()));
    }
    if result == kNotImplemented || (optional && result == kResultFalse) {
        return Ok(None);
    }
    Err(format!("getState devolvió {result}"))
}
fn apply_state(
    bytes: Vec<u8>,
    apply: impl FnOnce(*mut IBStream) -> tresult,
    label: &str,
    optional: bool,
) -> Result<(), String> {
    let stream = ComWrapper::new(Stream::from_bytes(bytes))
        .to_com_ptr::<IBStream>()
        .unwrap();
    let result = apply(stream.as_ptr());
    if result == kResultOk || (optional && (result == kNotImplemented || result == kResultFalse)) {
        Ok(())
    } else {
        Err(format!("{label} devolvió {result}"))
    }
}
impl Session {
    pub(super) fn capture(&self) -> Result<PluginState, String> {
        let applied = self.handler.applied.load(Ordering::Acquire);
        let component = read_state(|s| unsafe { self.component.getState(s) }, false)?;
        let controller = if self.separate_controller {
            self.controller
                .as_ref()
                .map(|c| read_state(|s| unsafe { c.getState(s) }, true))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let mut pending = self.handler.pending(applied)?;
        if component.as_ref().is_none_or(Vec::is_empty)
            && let Some(c) = &self.controller
        {
            // Algunos plugins sólo exponen parámetros normalizados y escriben un chunk vacío.
            let mut parameters = Vec::new();
            let count = unsafe { c.getParameterCount() };
            if !(0..=4096).contains(&count) {
                return Err("Cantidad de parámetros fuera de rango".into());
            }
            for index in 0..count {
                let mut info: ParameterInfo = unsafe { std::mem::zeroed() };
                if unsafe { c.getParameterInfo(index, &mut info) } != kResultOk {
                    return Err("No se pudo leer un parámetro VST3".into());
                }
                if info.flags & ParameterInfo_::ParameterFlags_::kIsReadOnly as i32 == 0 {
                    parameters.push((info.id, unsafe { c.getParamNormalized(info.id) }));
                }
            }
            for (id, value) in pending.drain(..) {
                if let Some(p) = parameters.iter_mut().find(|(i, _)| *i == id) {
                    p.1 = value
                } else {
                    parameters.push((id, value));
                }
            }
            pending = parameters;
        }
        Ok(PluginState {
            path: self.path.clone(),
            class: self.class_id.clone(),
            instrument: self.instrument,
            component,
            controller,
            pending,
        })
    }
    pub(crate) fn restore(&mut self, state: PluginState) -> Result<(), String> {
        // Reiniciar el stream para cada lector. El controlador puede tener estado propio.
        let for_controller = if self.separate_controller {
            state.component.clone()
        } else {
            None
        };
        if let Some(bytes) = state.component.filter(|v| !v.is_empty()) {
            apply_state(
                bytes,
                |s| unsafe { self.component.setState(s) },
                "component.setState",
                false,
            )?;
        }
        if let Some(controller) = &self.controller {
            if let Some(bytes) = for_controller.filter(|v| !v.is_empty()) {
                apply_state(
                    bytes,
                    |s| unsafe { controller.setComponentState(s) },
                    "setComponentState",
                    true,
                )?;
            }
            if let Some(bytes) = state.controller.filter(|v| !v.is_empty()) {
                apply_state(
                    bytes,
                    |s| unsafe { controller.setState(s) },
                    "controller.setState",
                    false,
                )?;
            }
        }
        if let Some(controller) = &self.controller {
            for &(id, value) in &state.pending {
                unsafe {
                    controller.setParamNormalized(id, value);
                }
            }
        }
        self.initial_parameters = state.pending;
        Ok(())
    }
}
