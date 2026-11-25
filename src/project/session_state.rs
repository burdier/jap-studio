//! Tiempo y mezcla editables, separados por campo para fusionar cambios independientes.
use super::{ProjectDoc, order_id, read_bool, read_i64, read_string};
use crate::devices::utility_slot::UtilityState;
use crate::musical::{
    fx::FxId,
    midi_map::{MidiAction, MidiBindTarget, MidiBinding, MidiSource},
    model::InstrumentId,
    time::{Meter, Tempo},
    track::{TrackId, TrackMix},
    track_automation::{ArrangementAutomation, AutomationLane, AutomationParam, AutomationPoint},
};

const DEFAULT_LEVEL: i32 = 75;
const DEFAULT_PAN: i32 = 50;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArrangementNavigation {
    pub markers: std::collections::BTreeMap<i64, String>,
    pub loop_region: Option<(i64, i64)>,
}

impl ProjectDoc {
    pub fn arrangement_navigation(&self) -> Result<ArrangementNavigation, String> {
        use loro::{LoroValue, ValueOrContainer::Value};
        let map = self.doc.get_map("arrangement_markers");
        let mut markers = std::collections::BTreeMap::new();
        for key in map.keys() {
            let tick = key
                .parse::<i64>()
                .map_err(|_| "Posición de marcador inválida")?;
            let label = read_string(&map, &key)?;
            if tick < 0 || label.trim().is_empty() {
                return Err("Marcador inválido".into());
            }
            markers.insert(tick, label);
        }
        let loop_region = match self.doc.get_map("project").get("loop_region") {
            None => None,
            Some(Value(LoroValue::List(ticks))) => match ticks.as_slice() {
                [LoroValue::I64(start), LoroValue::I64(end)] if *start >= 0 && start < end => {
                    Some((*start, *end))
                }
                _ => return Err("Región de loop inválida".into()),
            },
            _ => return Err("Región de loop inválida".into()),
        };
        Ok(ArrangementNavigation {
            markers,
            loop_region,
        })
    }

    pub fn set_arrangement_navigation(
        &self,
        navigation: &ArrangementNavigation,
    ) -> Result<(), String> {
        use loro::LoroValue;
        if navigation
            .markers
            .iter()
            .any(|(tick, label)| *tick < 0 || label.trim().is_empty())
            || navigation
                .loop_region
                .is_some_and(|(start, end)| start < 0 || start >= end)
        {
            return Err("Marcador o región de loop inválidos".into());
        }
        let previous = self.arrangement_navigation()?;
        let map = self.doc.get_map("arrangement_markers");
        for tick in previous
            .markers
            .keys()
            .filter(|tick| !navigation.markers.contains_key(tick))
        {
            map.delete(&tick.to_string()).map_err(|e| e.to_string())?;
        }
        for (tick, label) in &navigation.markers {
            if previous.markers.get(tick) != Some(label) {
                map.insert(&tick.to_string(), label.as_str())
                    .map_err(|e| e.to_string())?;
            }
        }
        if navigation.loop_region != previous.loop_region {
            let meta = self.doc.get_map("project");
            match navigation.loop_region {
                Some((start, end)) => meta.insert(
                    "loop_region",
                    LoroValue::List(vec![LoroValue::I64(start), LoroValue::I64(end)].into()),
                ),
                None => meta.delete("loop_region"),
            }
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn set_timing(&self, tempo: Tempo, meter: Meter) -> Result<(), String> {
        self.set_tempo(tempo)?;
        self.set_meter(meter)
    }

    pub fn set_tempo(&self, tempo: Tempo) -> Result<(), String> {
        let micro_bpm = (tempo.bpm() * 1_000_000.0).round() as i64;
        self.doc
            .get_map("project")
            .insert("tempo_micro_bpm", micro_bpm)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn set_meter(&self, meter: Meter) -> Result<(), String> {
        self.doc
            .get_map("project")
            .insert("meter", meter.label())
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn timing(&self) -> Result<(Tempo, Meter), String> {
        let meta = self.doc.get_map("project");
        let tempo = if meta.get("tempo_micro_bpm").is_some() {
            let bpm = read_i64(&meta, "tempo_micro_bpm")? as f64 / 1_000_000.0;
            Tempo::new(bpm).map_err(|error| error.to_string())?
        } else {
            Tempo::default()
        };
        let meter = if meta.get("meter").is_some() {
            let label = read_string(&meta, "meter")?;
            let (numerator, denominator) =
                label.split_once('/').ok_or("Compás de sesión inválido")?;
            let numerator = numerator
                .parse::<u8>()
                .map_err(|_| "Numerador de compás inválido")?;
            let denominator = denominator
                .parse::<u8>()
                .map_err(|_| "Denominador de compás inválido")?;
            Meter::new(numerator, denominator).map_err(|error| error.to_string())?
        } else {
            Meter::default()
        };
        Ok((tempo, meter))
    }

    pub fn set_track_mix(&self, id: TrackId, mix: TrackMix) -> Result<(), String> {
        if !(0..=100).contains(&mix.level) || !(0..=100).contains(&mix.pan) {
            return Err("Nivel o panorama fuera de rango".into());
        }
        self.require_track(id)?;
        self.set_track_level(id, mix.level)?;
        self.set_track_pan(id, mix.pan)
    }

    pub fn set_track_level(&self, id: TrackId, level: i32) -> Result<(), String> {
        if !(0..=100).contains(&level) {
            return Err("Nivel de pista fuera de rango".into());
        }
        self.require_track(id)?;
        self.doc
            .get_map(format!("track:{}", id.0))
            .insert("level", i64::from(level))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn set_track_pan(&self, id: TrackId, pan: i32) -> Result<(), String> {
        if !(0..=100).contains(&pan) {
            return Err("Panorama de pista fuera de rango".into());
        }
        self.require_track(id)?;
        self.doc
            .get_map(format!("track:{}", id.0))
            .insert("pan", i64::from(pan))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn track_mix(&self, id: TrackId) -> Result<TrackMix, String> {
        self.require_track(id)?;
        let map = self.doc.get_map(format!("track:{}", id.0));
        let level = if map.get("level").is_some() {
            i32::try_from(read_i64(&map, "level")?).map_err(|_| "Nivel de pista fuera de rango")?
        } else {
            DEFAULT_LEVEL
        };
        let pan = if map.get("pan").is_some() {
            i32::try_from(read_i64(&map, "pan")?).map_err(|_| "Panorama de pista fuera de rango")?
        } else {
            DEFAULT_PAN
        };
        if !(0..=100).contains(&level) || !(0..=100).contains(&pan) {
            return Err("Nivel o panorama fuera de rango".into());
        }
        Ok(TrackMix { level, pan })
    }

    pub fn set_master_utility(&self, state: UtilityState) -> Result<(), String> {
        if !state.gain.is_finite() || !(0.0..=2.0).contains(&state.gain) {
            return Err("Ganancia de Utility fuera de rango".into());
        }
        let map = self.doc.get_map("master_utility");
        map.insert("inserted", state.inserted)
            .map_err(|e| e.to_string())?;
        map.insert("bypassed", state.bypassed)
            .map_err(|e| e.to_string())?;
        map.insert("gain", f64::from(state.gain))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn master_utility(&self) -> Result<UtilityState, String> {
        let map = self.doc.get_map("master_utility");
        if map.get("inserted").is_none() {
            return Ok(UtilityState::default());
        }
        let inserted = read_bool(&map, "inserted")?;
        let bypassed = read_bool(&map, "bypassed")?;
        let gain = match map.get("gain") {
            Some(loro::ValueOrContainer::Value(loro::LoroValue::Double(v))) => v as f32,
            _ => return Err("Ganancia de Utility inválida".into()),
        };
        if !gain.is_finite() || !(0.0..=2.0).contains(&gain) {
            return Err("Ganancia de Utility fuera de rango".into());
        }
        Ok(UtilityState {
            inserted,
            bypassed,
            gain,
        })
    }

    pub fn set_master_chain(&self, chain: &crate::musical::fx::FxChain) -> Result<(), String> {
        super::fx::write_chain(&self.doc, crate::musical::track::MASTER_TRACK_ID, chain)
    }

    pub fn master_chain(&self) -> Result<crate::musical::fx::FxChain, String> {
        super::fx::read_chain(&self.doc, crate::musical::track::MASTER_TRACK_ID)
    }

    pub fn track_automation(&self) -> Result<ArrangementAutomation, String> {
        use loro::{LoroValue, ValueOrContainer::Value};
        let map = self.doc.get_map("track_automation");
        let mut automation = ArrangementAutomation::default();
        for key in map.keys() {
            let Some((raw_id, suffix)) = key.split_once(':') else {
                continue;
            };
            let Ok(id) = raw_id.parse::<u32>() else {
                continue;
            };
            let Some(param) = AutomationParam::from_suffix(suffix) else {
                continue;
            };
            let Some(Value(LoroValue::List(values))) = map.get(&key) else {
                return Err("Curva de automatización inválida".into());
            };
            if values.len() % 2 != 0 {
                return Err("Puntos de automatización incompletos".into());
            }
            let mut points = Vec::with_capacity(values.len() / 2);
            for pair in values.chunks_exact(2) {
                let (LoroValue::Double(beat), LoroValue::Double(val)) = (&pair[0], &pair[1]) else {
                    return Err("Valor de automatización inválido".into());
                };
                points.push(AutomationPoint {
                    beat: *beat,
                    value: *val as f32,
                });
            }
            let lane = AutomationLane::restore(points)?;
            if !lane.is_empty() {
                *automation
                    .tracks
                    .entry(TrackId(id))
                    .or_default()
                    .lane_mut(param) = lane;
            }
        }
        Ok(automation)
    }

    pub fn set_track_automation(&self, automation: &ArrangementAutomation) -> Result<(), String> {
        use loro::LoroValue;
        let map = self.doc.get_map("track_automation");
        let mut desired: std::collections::BTreeMap<String, Vec<LoroValue>> =
            std::collections::BTreeMap::new();
        for (&track_id, track_auto) in &automation.tracks {
            track_auto.for_each_non_empty(|param, lane| {
                let mut flat = Vec::with_capacity(lane.len() * 2);
                for point in lane.points() {
                    flat.push(LoroValue::Double(point.beat));
                    flat.push(LoroValue::Double(f64::from(point.value)));
                }
                desired.insert(format!("{}:{}", track_id.0, param.key_suffix()), flat);
            });
        }
        let existing_keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
        for key in existing_keys {
            if !desired.contains_key(&key) {
                map.delete(&key).map_err(|e| e.to_string())?;
            }
        }
        let previous = self.track_automation().unwrap_or_default();
        for (key, flat) in desired {
            let changed = match key.split_once(':') {
                Some((id_str, suf)) => {
                    match (id_str.parse::<u32>(), AutomationParam::from_suffix(suf)) {
                        (Ok(id), Some(param)) => {
                            let prev = previous.lane(TrackId(id), param);
                            let curr = automation.lane(TrackId(id), param);
                            prev != curr
                        }
                        _ => true,
                    }
                }
                None => true,
            };
            if changed {
                map.insert(&key, LoroValue::List(flat.into()))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn midi_bindings(&self) -> Result<Option<Vec<MidiBinding>>, String> {
        use loro::{LoroValue, ValueOrContainer::Value};
        let meta = self.doc.get_map("project");
        if meta.get("midi_bindings_saved").is_none() {
            return Ok(None);
        }
        let map = self.doc.get_map("midi_bindings");
        let mut keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
        keys.sort();
        let mut bindings = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(target) = decode_midi_target(&key) else {
                continue;
            };
            let Some(Value(LoroValue::List(values))) = map.get(&key) else {
                return Err("Enlace MIDI Learn inválido".into());
            };
            let [
                LoroValue::String(kind),
                LoroValue::I64(num),
                LoroValue::Double(min),
                LoroValue::Double(max),
            ] = values.as_slice()
            else {
                return Err("Formato de enlace MIDI Learn inválido".into());
            };
            let num = u8::try_from(*num).map_err(|_| "Número MIDI fuera de rango")?;
            let source = match kind.as_str() {
                "cc" => MidiSource::Cc(num),
                "note" => MidiSource::Note(num),
                _ => return Err("Origen MIDI Learn desconocido".into()),
            };
            bindings.push(MidiBinding {
                source,
                target,
                min: (*min as f32).clamp(0.0, 1.0),
                max: (*max as f32).clamp(0.0, 1.0),
            });
        }
        Ok(Some(bindings))
    }

    pub fn set_midi_bindings(&self, bindings: &[MidiBinding]) -> Result<(), String> {
        use loro::LoroValue;
        self.doc
            .get_map("project")
            .insert("midi_bindings_saved", true)
            .map_err(|e| e.to_string())?;
        let map = self.doc.get_map("midi_bindings");
        let mut desired: std::collections::BTreeMap<String, Vec<LoroValue>> =
            std::collections::BTreeMap::new();
        for binding in bindings {
            let key = encode_midi_target(binding.target);
            let (kind, num) = match binding.source {
                MidiSource::Cc(cc) => ("cc", i64::from(cc)),
                MidiSource::Note(note) => ("note", i64::from(note)),
            };
            desired.insert(
                key,
                vec![
                    LoroValue::String(kind.into()),
                    LoroValue::I64(num),
                    LoroValue::Double(f64::from(binding.min.clamp(0.0, 1.0))),
                    LoroValue::Double(f64::from(binding.max.clamp(0.0, 1.0))),
                ],
            );
        }
        let existing_keys: Vec<String> = map.keys().map(|k| k.to_string()).collect();
        for key in existing_keys {
            if !desired.contains_key(&key) {
                map.delete(&key).map_err(|e| e.to_string())?;
            }
        }
        for (key, payload) in desired {
            map.insert(&key, LoroValue::List(payload.into()))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn require_track(&self, id: TrackId) -> Result<(), String> {
        let order = self.doc.get_movable_list("track_order");
        if (0..order.len()).any(|index| order_id(&order, index) == Some(id)) {
            Ok(())
        } else {
            Err("Pista inexistente en el documento".into())
        }
    }
}

fn encode_midi_target(target: MidiBindTarget) -> String {
    match target {
        MidiBindTarget::TrackVolume(id) => format!("track_vol:{}", id.0),
        MidiBindTarget::TrackPan(id) => format!("track_pan:{}", id.0),
        MidiBindTarget::TrackSend { track, target } => {
            format!("track_send:{}:{}", track.0, target.0)
        }
        MidiBindTarget::MasterGain => "master_gain".into(),
        MidiBindTarget::InstrumentParam(idx) => format!("inst_param:{idx}"),
        MidiBindTarget::FxParam { track, fx, param } => {
            format!("fx_param:{}:{}:{param}", track.0, fx.0)
        }
        MidiBindTarget::VstInstrumentParam { instrument, param } => {
            format!("vst_param:{}:{param}", instrument.0)
        }
        MidiBindTarget::Action(action) => format!(
            "action:{}",
            match action {
                MidiAction::PlayToggle => "play_toggle",
                MidiAction::Stop => "stop",
                MidiAction::RecordToggle => "record_toggle",
                MidiAction::CaptureMidi => "capture_midi",
                MidiAction::NextBank => "next_bank",
                MidiAction::PrevBank => "prev_bank",
                MidiAction::NextPreset => "next_preset",
                MidiAction::PrevPreset => "prev_preset",
            }
        ),
    }
}

fn decode_midi_target(key: &str) -> Option<MidiBindTarget> {
    if key == "master_gain" {
        return Some(MidiBindTarget::MasterGain);
    }
    let parts: Vec<&str> = key.split(':').collect();
    match parts.as_slice() {
        ["track_vol", id] => Some(MidiBindTarget::TrackVolume(TrackId(id.parse().ok()?))),
        ["track_pan", id] => Some(MidiBindTarget::TrackPan(TrackId(id.parse().ok()?))),
        ["track_send", track, target] => Some(MidiBindTarget::TrackSend {
            track: TrackId(track.parse().ok()?),
            target: TrackId(target.parse().ok()?),
        }),
        ["inst_param", idx] => Some(MidiBindTarget::InstrumentParam(idx.parse().ok()?)),
        ["fx_param", track, fx, param] => Some(MidiBindTarget::FxParam {
            track: TrackId(track.parse().ok()?),
            fx: FxId(fx.parse().ok()?),
            param: param.parse().ok()?,
        }),
        ["vst_param", inst, param] => Some(MidiBindTarget::VstInstrumentParam {
            instrument: InstrumentId(inst.parse().ok()?),
            param: param.parse().ok()?,
        }),
        ["action", name] => Some(MidiBindTarget::Action(match *name {
            "play_toggle" => MidiAction::PlayToggle,
            "stop" => MidiAction::Stop,
            "record_toggle" => MidiAction::RecordToggle,
            "capture_midi" => MidiAction::CaptureMidi,
            "next_bank" => MidiAction::NextBank,
            "prev_bank" => MidiAction::PrevBank,
            "next_preset" => MidiAction::NextPreset,
            "prev_preset" => MidiAction::PrevPreset,
            _ => return None,
        })),
        _ => None,
    }
}
