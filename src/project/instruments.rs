//! Estado editable del piano. Los motores y buffers nunca entran en el documento.
use super::{ProjectDoc, read_i64, read_string};
use crate::musical::model::InstrumentId;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectPiano {
    pub id: InstrumentId,
    pub source: String,
    pub preset: u32,
    pub parameters: Vec<f32>,
}

impl ProjectDoc {
    pub fn set_pianos(&self, pianos: &[ProjectPiano]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for piano in pianos {
            if !ids.insert(piano.id) || piano.id.0 == u32::MAX {
                return Err("ID de piano duplicado o agotado".into());
            }
            if piano.parameters.is_empty()
                || piano.parameters.len() > 256
                || piano.parameters.iter().any(|v| !v.is_finite())
            {
                return Err("Parámetros de piano inválidos".into());
            }
        }
        let order = self.doc.get_list("piano_ids");
        // Reemplazar IDs idénticos en dos ramas crea dos inserciones al fusionarlas.
        let same_order = order.len() == pianos.len()
            && pianos.iter().enumerate().all(|(i, piano)| {
                matches!(order.get(i), Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(id)))
                if id == i64::from(piano.id.0))
            });
        if !same_order {
            order.delete(0, order.len()).map_err(|e| e.to_string())?;
            for piano in pianos {
                order
                    .push(i64::from(piano.id.0))
                    .map_err(|e| e.to_string())?;
            }
        }
        for piano in pianos {
            let map = self.doc.get_map(format!("piano:{}", piano.id.0));
            map.insert("source", piano.source.clone())
                .map_err(|e| e.to_string())?;
            map.insert("preset", i64::from(piano.preset))
                .map_err(|e| e.to_string())?;
            map.insert("version", 1_i64).map_err(|e| e.to_string())?;
            map.insert("parameter_count", piano.parameters.len() as i64)
                .map_err(|e| e.to_string())?;
            for (index, value) in piano.parameters.iter().enumerate() {
                map.insert(&format!("parameter:{index}"), f64::from(*value))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn pianos(&self) -> Result<Vec<ProjectPiano>, String> {
        let order = self.doc.get_list("piano_ids");
        let mut result = Vec::new();
        for index in 0..order.len() {
            let Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(id))) = order.get(index)
            else {
                return Err("ID de piano inválido".into());
            };
            let id = InstrumentId(u32::try_from(id).map_err(|_| "ID de piano inválido")?);
            if result.iter().any(|p: &ProjectPiano| p.id == id) {
                return Err("Piano duplicado".into());
            }
            let map = self.doc.get_map(format!("piano:{}", id.0));
            if read_i64(&map, "version")? != 1 {
                return Err("Versión de piano no compatible".into());
            }
            let count = read_i64(&map, "parameter_count")?;
            if !(1..=256).contains(&count) {
                return Err("Cantidad de parámetros inválida".into());
            }
            let mut parameters = Vec::new();
            for index in 0..count {
                let Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(value))) =
                    map.get(&format!("parameter:{index}"))
                else {
                    return Err("Parámetro de piano inválido".into());
                };
                let value = value as f32;
                if !value.is_finite() {
                    return Err("Parámetro de piano no finito".into());
                }
                parameters.push(value);
            }
            result.push(ProjectPiano {
                id,
                source: read_string(&map, "source")?,
                preset: u32::try_from(read_i64(&map, "preset")?).map_err(|_| "Preset inválido")?,
                parameters,
            });
        }
        Ok(result)
    }
}

use crate::devices::{
    filter::FilterMode,
    sampler::{PARAMS, PlayMode, SamplerParams},
};

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSampler {
    pub id: InstrumentId,
    pub source: String,
    pub params: SamplerParams,
}

impl ProjectDoc {
    pub fn set_samplers(&self, samplers: &[ProjectSampler]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for sampler in samplers {
            if !ids.insert(sampler.id) || sampler.source.is_empty() {
                return Err("Estado de Hay Sampler inválido".into());
            }
            let map = self.doc.get_map(format!("sampler:{}", sampler.id.0));
            map.insert("source", sampler.source.as_str())
                .map_err(|e| e.to_string())?;
            let params = &sampler.params;
            for (key, value) in [
                ("one_shot", params.mode == PlayMode::OneShot),
                ("loop", params.looped),
                ("filter", params.filter),
                ("keytrack", params.keytrack),
                ("reverse", params.reverse),
                ("choke", params.choke),
            ] {
                map.insert(key, value).map_err(|e| e.to_string())?;
            }
            let filter = match params.filter_mode {
                FilterMode::LowPass => 0_i64,
                FilterMode::HighPass => 1,
                FilterMode::BandPass => 2,
            };
            map.insert("filter_mode", filter)
                .map_err(|e| e.to_string())?;
            for (i, spec) in PARAMS.iter().enumerate() {
                map.insert(spec.key, f64::from(params.get(i)))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn samplers(&self) -> Result<Vec<ProjectSampler>, String> {
        let mut samplers = Vec::new();
        for id in self
            .tracks()?
            .into_iter()
            .filter_map(|track| track.instrument)
            .map(InstrumentId)
        {
            let map = self.doc.get_map(format!("sampler:{}", id.0));
            if map.get("source").is_none() || samplers.iter().any(|s: &ProjectSampler| s.id == id) {
                continue;
            }
            use super::read_bool;
            let mut params = SamplerParams::default();
            params.mode = if read_bool(&map, "one_shot")? {
                PlayMode::OneShot
            } else {
                PlayMode::Classic
            };
            params.looped = read_bool(&map, "loop")?;
            params.filter = read_bool(&map, "filter")?;
            params.keytrack = read_bool(&map, "keytrack")?;
            params.reverse = read_bool(&map, "reverse")?;
            params.choke = read_bool(&map, "choke")?;
            params.filter_mode = match read_i64(&map, "filter_mode")? {
                0 => FilterMode::LowPass,
                1 => FilterMode::HighPass,
                2 => FilterMode::BandPass,
                _ => return Err("Modo de filtro de Hay Sampler inválido".into()),
            };
            for (i, spec) in PARAMS.iter().enumerate() {
                let value = match map.get(spec.key) {
                    Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(value))) => {
                        value as f32
                    }
                    None if i >= crate::devices::sampler::FIRST_OPTIONAL_PARAM => spec.default,
                    _ => {
                        return Err(format!("Falta {} en el estado de Hay Sampler", spec.key));
                    }
                };
                if !(spec.min..=spec.max).contains(&value) {
                    return Err(format!(
                        "Parámetro de Hay Sampler fuera de rango: {}",
                        spec.key
                    ));
                }
                params.set(i, value);
            }
            samplers.push(ProjectSampler {
                id,
                source: read_string(&map, "source")?,
                params,
            });
        }
        samplers.sort_by_key(|s| s.id.0);
        Ok(samplers)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectVstInstrument {
    pub id: InstrumentId,
    pub name: String,
    pub class_id: String,
    pub bundle: String,
    pub preset: crate::musical::fx::VstPreset,
}

impl ProjectDoc {
    pub fn set_vst_instruments(&self, vsts: &[ProjectVstInstrument]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for vst in vsts {
            if !ids.insert(vst.id) || vst.id.0 == u32::MAX {
                return Err("ID de instrumento VST3 duplicado o agotado".into());
            }
            if vst.bundle.is_empty() && vst.class_id.is_empty() {
                return Err("Instrumento VST3 sin ruta ni clase".into());
            }
        }
        let order = self.doc.get_list("vst_ids");
        let same_order = order.len() == vsts.len()
            && vsts.iter().enumerate().all(|(i, vst)| {
                matches!(
                    order.get(i),
                    Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(id)))
                    if id == i64::from(vst.id.0)
                )
            });
        if !same_order {
            order.delete(0, order.len()).map_err(|e| e.to_string())?;
            for vst in vsts {
                order.push(i64::from(vst.id.0)).map_err(|e| e.to_string())?;
            }
        }
        for vst in vsts {
            let map = self.doc.get_map(format!("vst_inst:{}", vst.id.0));
            map.insert("name", vst.name.as_str())
                .map_err(|e| e.to_string())?;
            map.insert("class_id", vst.class_id.as_str())
                .map_err(|e| e.to_string())?;
            map.insert("bundle", vst.bundle.as_str())
                .map_err(|e| e.to_string())?;
            let stored = map.get("preset").map(|v| v.get_deep_value());
            if stored
                .as_ref()
                .and_then(|v| v.as_binary())
                .is_none_or(|b| b.as_slice() != vst.preset.bytes())
            {
                map.insert("preset", vst.preset.bytes().to_vec())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn vst_instruments(&self) -> Result<Vec<ProjectVstInstrument>, String> {
        let order = self.doc.get_list("vst_ids");
        let mut result = Vec::new();
        for index in 0..order.len() {
            let Some(loro::ValueOrContainer::Value(loro::LoroValue::I64(id))) = order.get(index)
            else {
                return Err("ID de instrumento VST3 inválido".into());
            };
            let id =
                InstrumentId(u32::try_from(id).map_err(|_| "ID de instrumento VST3 inválido")?);
            if result.iter().any(|v: &ProjectVstInstrument| v.id == id) {
                return Err("Instrumento VST3 duplicado".into());
            }
            let map = self.doc.get_map(format!("vst_inst:{}", id.0));
            let preset_bytes = map
                .get("preset")
                .map(|v| v.get_deep_value())
                .and_then(|v| v.as_binary().map(|b| b.to_vec()))
                .ok_or("Falta el estado del instrumento VST3")?;
            let preset = crate::musical::fx::VstPreset::new(preset_bytes)?;
            result.push(ProjectVstInstrument {
                id,
                name: read_string(&map, "name")?,
                class_id: read_string(&map, "class_id")?,
                bundle: read_string(&map, "bundle")?,
                preset,
            });
        }
        result.sort_by_key(|v| v.id.0);
        Ok(result)
    }
}

use crate::devices::synth::{
    CustomWavetable, EnvelopeMode, RoomKind, SynthEngineKind, SynthParams, VoiceParams,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSynth {
    pub id: InstrumentId,
    pub params: SynthParams,
}

impl ProjectDoc {
    pub fn set_synths(&self, synths: &[ProjectSynth]) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for synth in synths {
            if !ids.insert(synth.id) || synth.id.0 == u32::MAX {
                return Err("Estado de Hay Synth inválido".into());
            }
            let p = &synth.params;
            for (_, v) in p.all_param_pairs() {
                if !v.is_finite() {
                    return Err("Parámetro de Hay Synth no finito".into());
                }
            }
            let map = self.doc.get_map(format!("synth:{}", synth.id.0));
            map.insert("version", 2_i64).map_err(|e| e.to_string())?;
            map.insert("engine", i64::from(p.engine as u8))
                .map_err(|e| e.to_string())?;
            map.insert("room", i64::from(p.room as u8))
                .map_err(|e| e.to_string())?;
            map.insert("mode", i64::from(p.mode as u8))
                .map_err(|e| e.to_string())?;
            map.insert("preset_name", p.preset_name.as_str())
                .map_err(|e| e.to_string())?;
            for (key, value) in [
                ("distance", p.distance),
                ("shape", p.shape),
                ("tone", p.tone),
                ("body", p.body),
                ("motion", p.motion),
                ("time", p.time),
                ("touch", p.touch),
                ("glide", p.glide),
                ("gain", p.gain),
            ] {
                map.insert(key, f64::from(value))
                    .map_err(|e| e.to_string())?;
            }
            // Capa Voice Lab + Mod Matrix: pares `p:<id>` estables y compactos.
            for (pid, val) in p.all_param_pairs() {
                if pid >= 20 {
                    map.insert(&format!("p:{pid}"), f64::from(val))
                        .map_err(|e| e.to_string())?;
                }
            }
            for (bin_key, name_key, wt) in [
                ("wt_a", "wt_a_name", &p.custom_wt_a),
                ("wt_b", "wt_b_name", &p.custom_wt_b),
            ] {
                if let Some(table) = wt {
                    map.insert(name_key, table.name.as_str())
                        .map_err(|e| e.to_string())?;
                    map.insert(bin_key, table.to_i16_bytes())
                        .map_err(|e| e.to_string())?;
                } else {
                    let _ = map.delete(bin_key);
                    let _ = map.delete(name_key);
                }
            }
        }
        Ok(())
    }

    pub fn synths(&self) -> Result<Vec<ProjectSynth>, String> {
        let mut synths = Vec::new();
        for id in self
            .tracks()?
            .into_iter()
            .filter_map(|track| track.instrument)
            .map(InstrumentId)
        {
            let map = self.doc.get_map(format!("synth:{}", id.0));
            if map.get("version").is_none() || synths.iter().any(|s: &ProjectSynth| s.id == id) {
                continue;
            }
            let ver = read_i64(&map, "version")?;
            if ver != 1 && ver != 2 {
                return Err("Versión de Hay Synth no compatible".into());
            }
            let read_f = |key: &str, default: f32| -> Result<f32, String> {
                match map.get(key) {
                    Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(v))) => {
                        let v = v as f32;
                        if !v.is_finite() {
                            return Err(format!("Parámetro de Hay Synth inválido: {key}"));
                        }
                        Ok(v.clamp(0.0, 1.0))
                    }
                    None => Ok(default),
                    _ => Err(format!("Parámetro de Hay Synth inválido: {key}")),
                }
            };
            let engine = SynthEngineKind::from_u8(read_i64(&map, "engine")?.clamp(0, 7) as u8);
            let room = RoomKind::from_u8(read_i64(&map, "room")?.clamp(0, 3) as u8);
            let mode = EnvelopeMode::from_u8(read_i64(&map, "mode")?.clamp(0, 2) as u8);
            let preset_name = match map.get("preset_name") {
                Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) => s.to_string(),
                _ => String::new(),
            };
            let mut params = SynthParams {
                engine,
                room,
                distance: read_f("distance", 0.25)?,
                shape: read_f("shape", 0.45)?,
                tone: read_f("tone", 0.55)?,
                body: read_f("body", 0.55)?,
                motion: read_f("motion", 0.30)?,
                mode,
                time: read_f("time", 0.50)?,
                touch: read_f("touch", 0.55)?,
                glide: read_f("glide", 0.0)?,
                gain: read_f("gain", 0.80)?,
                preset_name,
                voice: VoiceParams::default(),
                custom_wt_a: None,
                custom_wt_b: None,
            };
            if ver >= 2 {
                let ids: Vec<u16> = params
                    .all_param_pairs()
                    .into_iter()
                    .filter(|(pid, _)| *pid >= 20)
                    .map(|(pid, _)| pid)
                    .collect();
                for pid in ids {
                    if let Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(v))) =
                        map.get(&format!("p:{pid}"))
                    {
                        let v = v as f32;
                        if v.is_finite() {
                            params.set_param(pid, v);
                        }
                    }
                }
                let read_wt = |bin_key: &str, name_key: &str| {
                    let bytes = map
                        .get(bin_key)
                        .map(|v| v.get_deep_value())
                        .and_then(|v| v.as_binary().map(|b| b.to_vec()))?;
                    let name = match map.get(name_key) {
                        Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) => {
                            s.to_string()
                        }
                        _ => "Vital WT".into(),
                    };
                    CustomWavetable::from_i16_bytes(name, &bytes).map(std::sync::Arc::new)
                };
                params.custom_wt_a = read_wt("wt_a", "wt_a_name");
                params.custom_wt_b = read_wt("wt_b", "wt_b_name");
            }
            params.clamp_all();
            synths.push(ProjectSynth { id, params });
        }
        synths.sort_by_key(|s| s.id.0);
        Ok(synths)
    }
}
