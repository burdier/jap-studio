//! Adaptador de herramientas MCP hacia los controladores en vivo de `DawApp`.
use super::{CenterView, DawApp, fx::FxTarget, pattern_editor::DrumViewMode};
use crate::{
    audio::{
        AudioClip, AudioFile,
        sample_analysis::{SampleProfile, analyze_path},
    },
    mcp::{IncomingMcpCall, IpcRequest, IpcResponse, McpHost},
};
use elevate::musical::{
    clip::MidiClip,
    drums::{DrumClipTarget, DrumPatternId, DrumPlaybackSource, DrumSongCell, piece_pattern},
    fx::{FxEdit, FxId, FxKind, FxNode, SidechainSource},
    grid::NoteGrid,
    model::{InstrumentId, NoteEvent, NoteId, Pattern},
    time::{PPQ, Tick},
    track::{MASTER_TRACK_ID, Track, TrackDevice, TrackEdit, TrackKind},
};
use gpui::{Context, Task};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

impl DawApp {
    pub(super) fn start_mcp_bridge(cx: &mut Context<Self>) -> (Option<McpHost>, Option<Task<()>>) {
        let Ok((host, rx)) = McpHost::start() else {
            return (None, None);
        };
        let background = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            let mut receiver: Option<mpsc::Receiver<IncomingMcpCall>> = Some(rx);
            while let Some(rx) = receiver.take() {
                let next = background
                    .spawn(async move {
                        loop {
                            let Ok(call) = rx.recv() else {
                                return None;
                            };
                            // Las herramientas puramente de disco/DSP (Explorer + análisis)
                            // corren 100% en segundo plano sin tocar ni frenar el hilo de UI.
                            if let Some(resp) = handle_background_mcp_call(&call.request) {
                                let _ = call.reply.send(resp);
                                continue;
                            }
                            return Some((call, rx));
                        }
                    })
                    .await;
                let Some((call, rx)) = next else {
                    break;
                };
                receiver = Some(rx);
                let ok = this
                    .update(cx, |this, cx| {
                        let response = this.handle_mcp_call(call.request, cx);
                        let _ = call.reply.send(response);
                        cx.notify();
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
        });
        (Some(host), Some(task))
    }

    fn handle_mcp_call(&mut self, req: IpcRequest, cx: &mut Context<Self>) -> IpcResponse {
        match self.dispatch_mcp(&req.tool, &req.arguments, cx) {
            Ok(result) => IpcResponse { ok: true, result },
            Err(error) => IpcResponse {
                ok: false,
                result: json!(error),
            },
        }
    }

    fn dispatch_mcp(
        &mut self,
        tool: &str,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        match tool {
            "get_project_state" => Ok(self.mcp_project_state()),
            "set_transport" => self.mcp_set_transport(args),
            "add_track" => self.mcp_add_track(args),
            "set_track_mix" => self.mcp_set_track_mix(args),
            "add_fx" => self.mcp_add_fx(args),
            "set_fx_param" => self.mcp_set_fx_param(args),
            "write_midi_clip" => self.mcp_write_midi_clip(args),
            "write_drum_pattern" => self.mcp_write_drum_pattern(args),
            "list_samples" => mcp_list_samples_bg(args),
            "browse_explorer" => mcp_browse_explorer_bg(args),
            "analyze_sample" => mcp_analyze_sample_bg(args),
            "add_drum_piece" => self.mcp_add_drum_piece(args),
            "load_drum_kit" => self.mcp_load_drum_kit(args),
            "place_drum_pattern" => self.mcp_place_drum_pattern(args),
            "load_sampler_sample" => self.mcp_load_sampler_sample(args, cx),
            "insert_audio_clip" => self.mcp_insert_audio_clip(args),
            "delete_track" => self.mcp_delete_track(args),
            "delete_clip" => self.mcp_delete_clip(args),
            "export_audio" => self.mcp_export_audio(args),
            "set_view" => self.mcp_set_view(args),
            "trigger_mpc_pad" => self.mcp_trigger_mpc_pad(args),
            "run_command" => self.mcp_run_command(args),
            other => Err(format!("Herramienta MCP desconocida: {other}")),
        }
    }

    fn serialize_fx_node(&self, n: &FxNode) -> Value {
        let mut params = Vec::new();
        let mut catalog_id: Option<String> = None;
        if let FxKind::Effect {
            plugin_id,
            parameters,
        } = &n.kind
        {
            if let TrackDevice::Hay { catalog_id: cid } = plugin_id {
                catalog_id = Some(cid.clone());
                if let Some(desc) = self.lua_components.registry.get(cid) {
                    for ctrl in &desc.controls {
                        let val = parameters.get(&ctrl.id).copied().unwrap_or(ctrl.default);
                        params.push(json!({
                            "id": ctrl.id,
                            "key": ctrl.key,
                            "name": ctrl.label,
                            "value": val,
                            "min": ctrl.min,
                            "max": ctrl.max,
                            "unit": ctrl.unit,
                        }));
                    }
                } else if let Some(dev) = elevate::devices::find(cid) {
                    for spec in dev.parameters {
                        let val = parameters.get(&spec.id.0).copied().unwrap_or(spec.default);
                        params.push(json!({
                            "id": spec.id.0,
                            "key": spec.key,
                            "name": spec.name,
                            "value": val,
                            "min": spec.min,
                            "max": spec.max,
                            "unit": spec.unit,
                        }));
                    }
                } else {
                    for (&pid, &val) in parameters {
                        params.push(json!({ "id": pid, "value": val }));
                    }
                }
            }
        }
        let sidechain = n.sidechain.map(|sc| match sc {
            SidechainSource::Drums => json!("drums"),
            SidechainSource::Track(tid) => self
                .tracks
                .iter()
                .position(|t| t.id == tid)
                .map(|i| json!(i))
                .unwrap_or_else(|| json!(tid.0)),
        });
        json!({
            "id": n.id.0,
            "name": n.name,
            "catalog_id": catalog_id,
            "enabled": n.enabled,
            "sidechain": sidechain,
            "params": params,
        })
    }

    fn mcp_project_state(&self) -> Value {
        let tracks: Vec<Value> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let level = self.track_levels.get(i).copied().unwrap_or(75);
                let pan = self.track_pans.get(i).copied().unwrap_or(50);
                let instrument = t.instrument.map(|id| {
                    if self.is_sampler_id(id) {
                        "sampler"
                    } else if self.is_vst_id(id) {
                        "vst3"
                    } else {
                        "piano"
                    }
                });
                let fx: Vec<Value> = t
                    .chain
                    .nodes()
                    .iter()
                    .map(|n| self.serialize_fx_node(n))
                    .collect();
                json!({
                    "index": i,
                    "id": t.id.0,
                    "name": t.name,
                    "kind": format!("{:?}", t.kind),
                    "level": level,
                    "pan": pan,
                    "muted": t.muted,
                    "solo": t.solo,
                    "instrument": instrument,
                    "fx": fx,
                })
            })
            .collect();

        let master_fx: Vec<Value> = self
            .master_chain
            .nodes()
            .iter()
            .map(|n| self.serialize_fx_node(n))
            .collect();

        let mut clips = Vec::new();
        for c in self
            .timeline
            .midi_clip
            .iter()
            .chain(&self.timeline.other_midi_clips)
        {
            clips.push(json!({
                "id": c.id.value(),
                "kind": "midi",
                "track": c.track,
                "name": c.name,
                "start_beat": c.start_beat,
                "duration_beats": c.duration_beats(),
                "notes": c.notes().len(),
                "drum_pattern": c.drum_target.map(|t| t.pattern.0),
            }));
        }
        let bpm = self.timeline.tempo.bpm();
        for c in self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
        {
            clips.push(json!({
                "id": c.id.value(),
                "kind": "audio",
                "track": c.track,
                "name": c.name,
                "start_beat": c.start_beat,
                "duration_beats": c.length_seconds() * bpm / 60.0,
            }));
        }

        let session = &self.pattern_editor.session;
        let active_pat = session.active_pattern();
        let pieces: Vec<Value> = session
            .kit
            .pieces
            .iter()
            .enumerate()
            .map(|(lane, p)| {
                json!({
                    "lane": lane,
                    "id": p.id.0,
                    "name": p.name,
                    "choke_group": p.choke_group,
                    "timing_offset": p.timing_offset,
                    "filter": p.sampler.filter,
                })
            })
            .collect();
        let patterns: Vec<Value> = session
            .patterns
            .iter()
            .map(|pat| {
                json!({
                    "id": pat.id.0,
                    "name": pat.name,
                    "bars": pat.bar_count(),
                    "grid": format!("1/{}", pat.grid.denominator()),
                    "swing": pat.swing,
                    "note_count": pat.notes.notes().len(),
                })
            })
            .collect();

        let available_fx: Vec<String> = self
            .lua_components
            .registry
            .values()
            .map(|d| format!("{} ({})", d.id, d.name))
            .collect();

        let library_roots: Vec<String> = discover_library_roots(None)
            .into_iter()
            .map(|p| p.display().to_string())
            .collect();

        json!({
            "tempo_bpm": bpm,
            "meter": format!("{}/{}", self.timeline.meter.numerator(), self.timeline.meter.denominator()),
            "playing": self.player.is_some(),
            "metronome": self.metronome.is_enabled(),
            "center_view": match self.center_view {
                CenterView::Arrangement => "arrangement",
                CenterView::Session => match self.pattern_editor.view_mode {
                    DrumViewMode::Steps => "drums",
                    DrumViewMode::Pads => "mpc",
                },
                CenterView::Mixer => "mixer",
            },
            "status_message": self.status_message,
            "tracks": tracks,
            "clips": clips,
            "master_fx": master_fx,
            "available_hey_fx": available_fx,
            "library_roots": library_roots,
            "drums": {
                "source": format!("{:?}", session.source),
                "active_pattern_id": active_pat.id.0,
                "active_pattern_name": active_pat.name,
                "grid": format!("1/{}", active_pat.grid.denominator()),
                "step_count": active_pat.step_count(),
                "swing": active_pat.swing,
                "note_count": active_pat.notes.notes().len(),
                "mpc_bank": self.pattern_editor.mpc.bank,
                "pieces": pieces,
                "patterns": patterns,
            }
        })
    }

    fn mcp_set_transport(&mut self, args: &Value) -> Result<Value, String> {
        if let Some(bpm) = args.get("tempo_bpm").and_then(Value::as_f64) {
            self.set_tempo(bpm);
        }
        if let Some(metro) = args.get("metronome").and_then(Value::as_bool)
            && self.metronome.is_enabled() != metro
        {
            self.toggle_metronome();
        }
        if let Some(action) = args.get("action").and_then(Value::as_str) {
            match action {
                "play" => {
                    if self.player.is_none() {
                        self.play_context();
                    }
                }
                "stop" => self.stop_transport(),
                "toggle" => {
                    if self.player.is_some() {
                        self.stop_transport();
                    } else {
                        self.play_context();
                    }
                }
                other => return Err(format!("Acción de transporte inválida: {other}")),
            }
        }
        Ok(json!({
            "playing": self.player.is_some(),
            "tempo_bpm": self.timeline.tempo.bpm(),
            "metronome": self.metronome.is_enabled(),
            "status": self.status_message,
        }))
    }

    fn mcp_add_track(&mut self, args: &Value) -> Result<Value, String> {
        self.edit_track_structure(TrackEdit::Insert);
        let index = self.cursor_row.min(self.tracks.len().saturating_sub(1));
        if let Some(name) = args
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            && let Some(track) = self.tracks.get_mut(index)
        {
            track.name = name.to_owned().into();
        }
        if let Some(inst) = args.get("instrument").and_then(Value::as_str) {
            match inst {
                "piano" => self.add_piano(index),
                "sampler" => self.add_sampler(index),
                "none" | "" => {}
                other => return Err(format!("Instrumento desconocido: {other}")),
            }
        }
        self.refresh_project_dirty();
        let track = &self.tracks[index];
        Ok(json!({
            "index": index,
            "id": track.id.0,
            "name": track.name,
            "status": self.status_message,
        }))
    }

    fn mcp_set_track_mix(&mut self, args: &Value) -> Result<Value, String> {
        let index = args
            .get("track_index")
            .and_then(Value::as_u64)
            .ok_or("Falta track_index")? as usize;
        if index >= self.tracks.len() {
            return Err(format!("Pista {index} fuera de rango"));
        }
        if let Some(level) = args.get("level").and_then(Value::as_i64) {
            self.set_track_level(index, level.clamp(0, 100) as i32);
        }
        if let Some(pan) = args.get("pan").and_then(Value::as_i64) {
            self.set_track_pan(index, pan.clamp(0, 100) as i32);
        }
        if let Some(mute) = args.get("mute").and_then(Value::as_bool)
            && self.tracks[index].muted != mute
        {
            self.toggle_track_mute(index);
        }
        if let Some(solo) = args.get("solo").and_then(Value::as_bool)
            && self.tracks[index].solo != solo
        {
            self.toggle_track_solo(index);
        }
        if let Some(name) = args
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            self.tracks[index].name = name.to_owned().into();
        }
        self.refresh_project_dirty();
        Ok(json!({
            "index": index,
            "name": self.tracks[index].name,
            "level": self.track_levels.get(index).copied().unwrap_or(75),
            "pan": self.track_pans.get(index).copied().unwrap_or(50),
            "muted": self.tracks[index].muted,
            "solo": self.tracks[index].solo,
        }))
    }

    fn mcp_add_fx(&mut self, args: &Value) -> Result<Value, String> {
        let raw_id = args
            .get("fx_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta fx_id")?;

        let track_id = if let Some(idx) = args.get("track_index").and_then(Value::as_u64) {
            let i = idx as usize;
            self.tracks
                .get(i)
                .map(|t| t.id)
                .ok_or_else(|| format!("Pista {i} fuera de rango"))?
        } else {
            MASTER_TRACK_ID
        };

        let target = FxTarget {
            track: track_id,
            node: None,
        };

        let resolved = match raw_id.to_lowercase().as_str() {
            "eq" | "hay eq" | "hey eq" => "hay.fx.eq",
            "comp" | "compressor" | "hay comp" | "hey comp" => "hay.fx.compressor",
            "limiter" | "hay limiter" | "hey limiter" => "hay.fx.limiter",
            "echo" | "delay" | "hay echo" | "hey echo" => "hay.fx.echo",
            "reverb" | "hay reverb" | "hey reverb" => "hay.fx.reverb",
            "double_volume" => "hay.fx.double_volume",
            "filter" | "hay filter" | "hey filter" => "hay.fx.filter",
            "volume" => "primitive.volume",
            "convolver" | "cabinet" => elevate::musical::fx::cabinet::ID,
            _ => raw_id,
        };

        if self.lua_components.registry.contains_key(resolved) {
            self.insert_lua_component(target, resolved)?;
            self.show_fx();
        } else {
            self.insert_fx(target, resolved)?;
        }
        let fx_node = self
            .fx_chain(track_id)
            .and_then(|c| c.nodes().last())
            .map(|n| self.serialize_fx_node(n));
        self.refresh_project_dirty();
        Ok(json!({
            "inserted": resolved,
            "target": if track_id == MASTER_TRACK_ID { "master".to_owned() } else { format!("track {}", track_id.0) },
            "fx": fx_node,
            "status": self.status_message,
        }))
    }

    fn resolve_fx_param_id(&self, node: &FxNode, key: &Value) -> Result<u32, String> {
        if let Some(id) = key.as_u64() {
            return Ok(id as u32);
        }
        let name = key
            .as_str()
            .map(|s| s.trim().trim_matches('"').trim())
            .filter(|s| !s.is_empty())
            .ok_or("param debe ser un ID numérico o nombre/key de parámetro")?;
        if let Ok(parsed) = name.parse::<u32>() {
            return Ok(parsed);
        }
        let needle = name.to_lowercase();
        if let FxKind::Effect {
            plugin_id: TrackDevice::Hay { catalog_id },
            ..
        } = &node.kind
        {
            if let Some(desc) = self.lua_components.registry.get(catalog_id) {
                // Priorizar `key` exacta antes que `label` (ej. `decay` vs `decay_div` en Reverb).
                if let Some(ctrl) = desc
                    .controls
                    .iter()
                    .find(|c| c.key.eq_ignore_ascii_case(&needle))
                {
                    return Ok(ctrl.id);
                }
                if let Some(ctrl) = desc
                    .controls
                    .iter()
                    .find(|c| c.label.eq_ignore_ascii_case(&needle))
                {
                    return Ok(ctrl.id);
                }
                if let Some(ctrl) = desc
                    .controls
                    .iter()
                    .find(|c| c.key.to_lowercase().contains(&needle))
                {
                    return Ok(ctrl.id);
                }
                if let Some(ctrl) = desc
                    .controls
                    .iter()
                    .find(|c| c.label.to_lowercase().contains(&needle))
                {
                    return Ok(ctrl.id);
                }
            } else if let Some(dev) = elevate::devices::find(catalog_id) {
                if let Some(spec) = dev
                    .parameters
                    .iter()
                    .find(|p| p.key.eq_ignore_ascii_case(&needle))
                {
                    return Ok(spec.id.0);
                }
                if let Some(spec) = dev
                    .parameters
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(&needle))
                {
                    return Ok(spec.id.0);
                }
                if let Some(spec) = dev
                    .parameters
                    .iter()
                    .find(|p| p.key.to_lowercase().contains(&needle))
                {
                    return Ok(spec.id.0);
                }
                if let Some(spec) = dev
                    .parameters
                    .iter()
                    .find(|p| p.name.to_lowercase().contains(&needle))
                {
                    return Ok(spec.id.0);
                }
            }
        }
        Err(format!("Parámetro «{name}» no encontrado en {}", node.name))
    }

    fn mcp_set_fx_param(&mut self, args: &Value) -> Result<Value, String> {
        let fx_id = FxId(
            args.get("fx_id")
                .and_then(Value::as_u64)
                .ok_or("Falta fx_id")?,
        );
        let track_id = if let Some(idx) = args.get("track_index").and_then(Value::as_u64) {
            let i = idx as usize;
            self.tracks
                .get(i)
                .map(|t| t.id)
                .ok_or_else(|| format!("Pista {i} fuera de rango"))?
        } else {
            MASTER_TRACK_ID
        };
        let node = self
            .fx_chain(track_id)
            .and_then(|chain| chain.find(fx_id))
            .cloned()
            .ok_or("Nodo FX no encontrado en ese destino")?;

        let mut updates: Vec<(u32, f32)> = Vec::new();
        if let (Some(param_key), Some(val)) =
            (args.get("param"), args.get("value").and_then(Value::as_f64))
        {
            let pid = self.resolve_fx_param_id(&node, param_key)?;
            updates.push((pid, val as f32));
        }
        if let Some(map) = args.get("params").and_then(Value::as_object) {
            for (k, v) in map {
                let Some(val) = v.as_f64() else {
                    return Err(format!("Valor inválido para parámetro «{k}»"));
                };
                let pid = self.resolve_fx_param_id(&node, &Value::String(k.clone()))?;
                updates.push((pid, val as f32));
            }
        }
        for (pid, val) in updates {
            self.edit_fx(track_id, FxEdit::Parameter(fx_id, pid, val))?;
        }
        if let Some(enabled) = args.get("enabled").and_then(Value::as_bool)
            && node.enabled != enabled
        {
            self.edit_fx(track_id, FxEdit::Toggle(fx_id))?;
        }
        if let Some(sc_val) = args.get("sidechain") {
            let source = if sc_val.is_null()
                || sc_val
                    .as_str()
                    .is_some_and(|s| matches!(s.to_lowercase().as_str(), "none" | "off" | ""))
            {
                None
            } else if sc_val
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("drums"))
            {
                Some(SidechainSource::Drums)
            } else if let Some(idx) = sc_val.as_u64() {
                let tid = self
                    .tracks
                    .get(idx as usize)
                    .map(|t| t.id)
                    .ok_or_else(|| format!("Pista de sidechain {idx} fuera de rango"))?;
                Some(SidechainSource::Track(tid))
            } else {
                return Err("sidechain debe ser 'none', 'drums' o un índice de pista".into());
            };
            self.edit_fx(track_id, FxEdit::Sidechain(fx_id, source))?;
        }
        self.refresh_project_dirty();
        let updated = self
            .fx_chain(track_id)
            .and_then(|chain| chain.find(fx_id))
            .map(|n| self.serialize_fx_node(n));
        Ok(json!({ "fx": updated }))
    }

    fn mcp_add_drum_piece(&mut self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta path del archivo de audio")?;
        let path = Path::new(path_str);
        AudioFile::decode(path)
            .map_err(|e| format!("No se pudo decodificar {}: {e}", path.display()))?;
        let id = self
            .pattern_editor
            .session
            .kit
            .add_sample(path_str.to_owned())?;
        let lane = self.pattern_editor.session.kit.pieces.len() - 1;
        if let Some(piece) = self.pattern_editor.session.kit.pieces.get_mut(lane) {
            if let Some(name) = args
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                piece.name = name.to_owned();
            }
            if let Some(choke) = args.get("choke_group").and_then(Value::as_u64)
                && choke > 0
            {
                piece.choke_group = Some(choke.min(u16::MAX as u64) as u16);
            }
        }
        self.pattern_editor.cursor_lane = lane;
        self.publish_drum_edit();
        self.sync_drum_audition_kit();
        self.refresh_project_dirty();
        let piece = &self.pattern_editor.session.kit.pieces[lane];
        self.status_message = format!("MCP: pieza «{}» añadida (lane {lane})", piece.name);
        Ok(json!({
            "lane": lane,
            "id": id.0,
            "name": piece.name,
            "choke_group": piece.choke_group,
        }))
    }

    fn mcp_load_drum_kit(&mut self, args: &Value) -> Result<Value, String> {
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta path del kit (.haykit)")?;
        self.load_drum_kit_from_path(Path::new(path_str));
        self.sync_drum_audition_kit();
        Ok(json!({
            "pieces": self.pattern_editor.session.kit.pieces.len(),
            "status": self.status_message,
        }))
    }

    fn mcp_place_drum_pattern(&mut self, args: &Value) -> Result<Value, String> {
        let session_active = self.pattern_editor.session.active;
        let pat_id = args
            .get("pattern_id")
            .and_then(Value::as_u64)
            .map(|id| DrumPatternId(id as u32))
            .unwrap_or(session_active);
        let start_beat = args
            .get("start_beat")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0);
        let repeats = args
            .get("repeats")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 64) as usize;
        let target = args
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("arrangement");

        let source_pat = self
            .pattern_editor
            .session
            .pattern(pat_id)
            .cloned()
            .ok_or_else(|| format!("Patrón {} inexistente", pat_id.0))?;
        let pat_ticks = source_pat.notes.length().0.max(1);
        let pat_beats = pat_ticks as f64 / PPQ as f64;

        if target == "song" {
            let base_tick = (start_beat * PPQ as f64).round() as i64;
            for r in 0..repeats {
                let cell = DrumSongCell {
                    at: Tick(base_tick + r as i64 * pat_ticks),
                    pattern: pat_id,
                };
                self.pattern_editor.session.set_song_cell(cell, true)?;
            }
            self.pattern_editor.session.source = DrumPlaybackSource::Song;
            self.publish_drum_edit();
            self.refresh_project_dirty();
            return Ok(json!({
                "target": "song",
                "pattern_id": pat_id.0,
                "start_beat": start_beat,
                "repeats": repeats,
            }));
        }

        // Volcar al Arrangement creando/reutilizando carriles Drum por pieza del kit.
        if self.pattern_editor.session.kit.pieces.is_empty() {
            return Err("El kit de Drums está vacío; añade piezas primero con add_drum_piece o load_drum_kit".into());
        }
        let kit_id = self.pattern_editor.session.kit.id;
        let pieces = self.pattern_editor.session.kit.pieces.clone();
        let mut tracks = self.tracks.clone();
        let mut used = Vec::with_capacity(pieces.len());
        for piece in &pieces {
            let is_piece = |clip: &MidiClip| {
                clip.drum_target
                    .is_some_and(|t| t.kit == kit_id && t.piece == piece.id)
            };
            let existing = self
                .timeline
                .midi_clip
                .iter()
                .chain(&self.timeline.other_midi_clips)
                .filter(|c| is_piece(c))
                .map(|c| c.track)
                .find(|idx| {
                    !used.contains(idx)
                        && tracks.get(*idx).is_some_and(|t| t.kind == TrackKind::Drum)
                });
            let track_idx = existing.unwrap_or_else(|| {
                let idx = tracks.len();
                tracks.push(Track::new(piece.name.clone(), TrackKind::Drum));
                idx
            });
            used.push(track_idx);
        }
        if tracks.len() != self.tracks.len() {
            let source_count = self.tracks.len();
            let sources = (0..tracks.len())
                .map(|i| (i < source_count).then_some(i))
                .collect();
            let mut levels = self.track_levels.clone();
            let mut pans = self.track_pans.clone();
            levels.resize(tracks.len(), 75);
            pans.resize(tracks.len(), 50);
            self.commit_track_plan(
                tracks,
                sources,
                levels,
                pans,
                self.selected_tracks.clone(),
                true,
                false,
                None,
            );
        }
        let swung = source_pat.swung_notes()?;
        let mut inserted = 0usize;
        for r in 0..repeats {
            let beat = start_beat + r as f64 * pat_beats;
            for (&track_idx, piece) in used.iter().zip(&pieces) {
                let pat = piece_pattern(&swung, piece.id)?;
                if pat.notes().is_empty() {
                    continue;
                }
                let mut clip = MidiClip::new(
                    format!("{} · {}", source_pat.name, piece.name),
                    track_idx,
                    beat,
                    pat,
                );
                clip.drum_target = Some(DrumClipTarget {
                    kit: kit_id,
                    piece: piece.id,
                    pattern: source_pat.id,
                    linked: true,
                });
                self.timeline.insert_midi_clip(clip);
                inserted += 1;
            }
        }
        self.pattern_editor.session.source = DrumPlaybackSource::Arrangement;
        self.publish_drum_edit();
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!(
            "MCP: patrón «{}» colocado en beat {start_beat:.1} ({inserted} clips)",
            source_pat.name
        );
        Ok(json!({
            "target": "arrangement",
            "pattern_id": pat_id.0,
            "start_beat": start_beat,
            "repeats": repeats,
            "clips_inserted": inserted,
        }))
    }

    fn mcp_load_sampler_sample(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let index = args
            .get("track_index")
            .and_then(Value::as_u64)
            .ok_or("Falta track_index")? as usize;
        if index >= self.tracks.len() {
            return Err(format!("Pista {index} fuera de rango"));
        }
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta path del sample")?;
        let path = PathBuf::from(path_str);
        if !path.is_file() {
            return Err(format!("Archivo no encontrado: {}", path.display()));
        }
        if let Some(id) = self.tracks[index].instrument
            && self.is_sampler_id(id)
        {
            self.activate_sampler(id);
            self.cursor_row = index;
        } else {
            self.add_sampler(index);
        }
        self.load_sample(path.clone(), cx);
        Ok(json!({
            "track_index": index,
            "path": path.display().to_string(),
            "status": self.status_message,
        }))
    }

    fn mcp_insert_audio_clip(&mut self, args: &Value) -> Result<Value, String> {
        let index = args
            .get("track_index")
            .and_then(Value::as_u64)
            .ok_or("Falta track_index")? as usize;
        if index >= self.tracks.len() {
            return Err(format!("Pista {index} fuera de rango"));
        }
        let path_str = args
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta path del archivo de audio")?;
        let start_beat = args
            .get("start_beat")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0);
        let file = AudioFile::decode(Path::new(path_str))?;
        let name = file.name.clone();
        let dur_sec = file.frames.len() as f64 / f64::from(file.sample_rate.max(1));
        let clip = AudioClip {
            id: elevate::musical::clip::ClipId::fresh(),
            name: name.clone(),
            file: Arc::new(file),
            processing: Default::default(),
            source_path_override: None,
            track: index,
            start_beat,
            props: Default::default(),
            trim_start: 0.0,
            trim_len: None,
        };
        let clip_id = clip.id.value();
        self.timeline.insert_audio_clip(clip);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message =
            format!("MCP: clip de audio «{name}» insertado en beat {start_beat:.1}");
        Ok(json!({
            "clip_id": clip_id,
            "track_index": index,
            "name": name,
            "start_beat": start_beat,
            "duration_seconds": dur_sec,
        }))
    }

    fn mcp_delete_track(&mut self, args: &Value) -> Result<Value, String> {
        let index = args
            .get("track_index")
            .and_then(Value::as_u64)
            .ok_or("Falta track_index")? as usize;
        if index >= self.tracks.len() {
            return Err(format!("Pista {index} fuera de rango"));
        }
        let removed_name = self.tracks[index].name.to_string();
        self.select_track(index, false, false);
        self.edit_track_structure(TrackEdit::Delete);
        self.refresh_project_dirty();
        Ok(json!({
            "deleted_track": removed_name,
            "remaining_tracks": self.tracks.len(),
            "status": self.status_message,
        }))
    }

    fn mcp_delete_clip(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clip_id")
            .and_then(Value::as_u64)
            .ok_or("Falta clip_id")?;
        let mut removed = false;
        if self
            .timeline
            .midi_clip
            .as_ref()
            .is_some_and(|c| c.id.value() == clip_id)
        {
            self.timeline.midi_clip = self.timeline.other_midi_clips.pop();
            removed = true;
        } else if let Some(pos) = self
            .timeline
            .other_midi_clips
            .iter()
            .position(|c| c.id.value() == clip_id)
        {
            self.timeline.other_midi_clips.remove(pos);
            removed = true;
        } else if self
            .timeline
            .clip
            .as_ref()
            .is_some_and(|c| c.id.value() == clip_id)
        {
            self.timeline.clip = self.timeline.other_audio_clips.pop();
            removed = true;
        } else if let Some(pos) = self
            .timeline
            .other_audio_clips
            .iter()
            .position(|c| c.id.value() == clip_id)
        {
            self.timeline.other_audio_clips.remove(pos);
            removed = true;
        }
        if !removed {
            return Err(format!("Clip {clip_id} no encontrado"));
        }
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!("MCP: clip {clip_id} eliminado");
        Ok(json!({ "deleted_clip_id": clip_id }))
    }

    fn mcp_export_audio(&mut self, args: &Value) -> Result<Value, String> {
        let filename = args
            .get("filename")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta filename (ej. 'lofi-beat.wav')")?;
        let mut cmd = format!("export -n \"{filename}\" -master");
        if let Some(sr) = args.get("sample_rate").and_then(Value::as_u64) {
            cmd.push_str(&format!(" -sr {sr}"));
        }
        if let Some(bits) = args.get("bits").and_then(Value::as_u64) {
            cmd.push_str(&format!(" -bits {bits}"));
        }
        if let Some(lufs) = args.get("lufs").and_then(Value::as_f64) {
            cmd.push_str(&format!(" -lufs {lufs:.1}"));
        }
        if let Some(out_dir) = args
            .get("output_dir")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            cmd.push_str(&format!(" -out \"{out_dir}\""));
        }
        self.start_export(&cmd);
        Ok(json!({
            "command": cmd,
            "exporting": self.export_job.is_some(),
            "status": self.status_message,
        }))
    }

    fn mcp_write_midi_clip(&mut self, args: &Value) -> Result<Value, String> {
        let track = args
            .get("track_index")
            .and_then(Value::as_u64)
            .ok_or("Falta track_index")? as usize;
        if track >= self.tracks.len() {
            return Err(format!("Pista {track} fuera de rango"));
        }
        let clip_start = args
            .get("start_beat")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0);
        let raw_notes = args
            .get("notes")
            .and_then(Value::as_array)
            .ok_or("Falta el arreglo notes")?;

        let mut max_end_ticks = 4 * PPQ;
        let mut parsed = Vec::with_capacity(raw_notes.len());
        for item in raw_notes {
            let pitch = item
                .get("pitch")
                .and_then(Value::as_u64)
                .ok_or("Cada nota requiere pitch (0..127)")?
                .min(127) as u8;
            let start_b = item
                .get("start_beat")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .max(0.0);
            let dur_b = item
                .get("duration_beats")
                .and_then(Value::as_f64)
                .unwrap_or(0.5)
                .max(0.05);
            let velocity = item
                .get("velocity")
                .and_then(Value::as_u64)
                .unwrap_or(100)
                .clamp(1, 127) as u8;
            let start = (start_b * PPQ as f64).round() as i64;
            let dur = ((dur_b * PPQ as f64).round() as i64).max(1);
            max_end_ticks = max_end_ticks.max(start + dur);
            parsed.push((pitch, start, dur, velocity));
        }

        let bar = self.timeline.meter.ticks_per_bar().max(PPQ);
        let length_ticks = if let Some(len_b) = args.get("length_beats").and_then(Value::as_f64) {
            ((len_b.max(0.25) * PPQ as f64).round() as i64).max(max_end_ticks)
        } else {
            ((max_end_ticks + bar - 1) / bar).max(1) * bar
        };

        let instrument = self.tracks[track].instrument.unwrap_or(InstrumentId(1));
        let mut pattern = Pattern::new(Tick(length_ticks)).map_err(|e| e.to_string())?;
        for (pitch, start, dur, velocity) in parsed {
            let clamped_dur = dur.min(length_ticks - start).max(1);
            pattern
                .add_note(instrument, Tick(start), Tick(clamped_dur), pitch, velocity)
                .map_err(|e| e.to_string())?;
        }

        let name = args
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("MCP MIDI")
            .to_owned();
        let note_count = pattern.notes().len();
        let clip = MidiClip::new(name.clone(), track, clip_start, pattern);
        let clip_id = clip.id.value();
        self.timeline.insert_midi_clip(clip);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!("MCP: clip «{name}» insertado ({note_count} notas)");

        Ok(json!({
            "clip_id": clip_id,
            "track_index": track,
            "start_beat": clip_start,
            "length_beats": length_ticks as f64 / PPQ as f64,
            "notes": note_count,
        }))
    }

    fn mcp_write_drum_pattern(&mut self, args: &Value) -> Result<Value, String> {
        let mode = args
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("replace");
        match mode {
            "new" => {
                self.pattern_editor
                    .session
                    .set_active_meter(self.timeline.meter);
                self.pattern_editor.session.new_pattern()?;
            }
            "clear" | "replace" => {
                self.pattern_editor.session.clear_active_pattern()?;
            }
            "merge" => {}
            other => return Err(format!("Modo de patrón inválido: {other}")),
        }

        if let Some(denom) = args.get("grid").and_then(Value::as_u64) {
            let grid = NoteGrid::new(denom as u16)
                .map_err(|_| format!("Subdivisión no soportada: 1/{denom}"))?;
            self.pattern_editor.session.active_pattern_mut().grid = grid;
            self.pattern_editor.roll.snap = grid;
        }
        if let Some(swing) = args.get("swing").and_then(Value::as_u64) {
            self.pattern_editor.session.active_pattern_mut().swing =
                (swing as u8).min(elevate::musical::drums::DrumPattern::MAX_SWING);
        }

        let mut added = 0usize;
        if let Some(steps) = args.get("steps").and_then(Value::as_array) {
            let step_ticks = self.pattern_editor.ticks_per_step().max(1);
            let step_count = self.pattern_editor.step_count();
            let pieces: Vec<(InstrumentId, u8)> = self
                .pattern_editor
                .session
                .kit
                .pieces
                .iter()
                .map(|p| (p.id, p.base_note()))
                .collect();
            let pattern = &mut self.pattern_editor.session.active_pattern_mut().notes;
            let length = pattern.length().0;
            let mut notes = pattern.notes().to_vec();
            let mut next_id = pattern.next_id();

            for item in steps {
                let lane = item
                    .get("lane")
                    .and_then(Value::as_u64)
                    .ok_or("Cada paso requiere lane")? as usize;
                let step = item
                    .get("step")
                    .and_then(Value::as_u64)
                    .ok_or("Cada paso requiere step")? as usize;
                let Some(&(piece_id, base_pitch)) = pieces.get(lane) else {
                    return Err(format!(
                        "Fila (lane) {lane} inexistente (el kit tiene {} piezas)",
                        pieces.len()
                    ));
                };
                if step >= step_count {
                    continue;
                }
                let velocity = item
                    .get("velocity")
                    .and_then(Value::as_u64)
                    .unwrap_or(100)
                    .clamp(1, 127) as u8;
                let probability = item
                    .get("probability")
                    .and_then(Value::as_u64)
                    .unwrap_or(100)
                    .clamp(5, 100) as u8;
                let pitch_offset = item
                    .get("pitch_offset")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .clamp(-24, 24) as i16;
                let pitch = (i16::from(base_pitch) + pitch_offset).clamp(0, 127) as u8;
                let start = step as i64 * step_ticks;
                if start >= length {
                    continue;
                }
                let dur = step_ticks.min(length - start).max(1);
                let mut expr = elevate::musical::note_expression::NoteExpression::default();
                expr.probability = probability;
                notes.push(NoteEvent {
                    id: NoteId(next_id),
                    instrument: piece_id,
                    start: Tick(start),
                    duration: Tick(dur),
                    pitch,
                    velocity,
                    expression: expr,
                    pitch_curve: Default::default(),
                });
                next_id += 1;
                added += 1;
            }
            *pattern = pattern
                .with_notes(notes, next_id)
                .map_err(|e| e.to_string())?;
        }

        let active = self.pattern_editor.session.active;
        let _ = self
            .timeline
            .refresh_drum_links(&self.pattern_editor.session, active);
        self.publish_drum_edit();
        self.refresh_project_dirty();
        self.status_message = format!("MCP: patrón actualizado (+{added} pasos)");

        Ok(json!({
            "active_pattern_id": active.0,
            "steps_added": added,
            "total_notes": self.pattern_editor.pattern().notes().len(),
            "swing": self.pattern_editor.session.active_pattern().swing,
        }))
    }

    fn mcp_set_view(&mut self, args: &Value) -> Result<Value, String> {
        let view = args
            .get("view")
            .and_then(Value::as_str)
            .ok_or("Falta view")?;
        match view {
            "arrangement" => self.set_center_view(CenterView::Arrangement),
            "drums" => {
                self.pattern_editor.mixer_open = false;
                self.pattern_editor.view_mode = DrumViewMode::Steps;
                self.set_center_view(CenterView::Session);
            }
            "mpc" => {
                self.pattern_editor.mixer_open = false;
                self.pattern_editor.roll.piece = None;
                self.pattern_editor.view_mode = DrumViewMode::Pads;
                self.set_center_view(CenterView::Session);
            }
            "mixer" => self.set_center_view(CenterView::Mixer),
            other => return Err(format!("Vista desconocida: {other}")),
        }
        Ok(json!({ "view": view }))
    }

    fn mcp_trigger_mpc_pad(&mut self, args: &Value) -> Result<Value, String> {
        let pad = args
            .get("pad")
            .and_then(Value::as_u64)
            .ok_or("Falta pad (0..15)")? as usize;
        if pad >= 16 {
            return Err("pad debe estar entre 0 y 15".into());
        }
        let velocity = args
            .get("velocity")
            .and_then(Value::as_u64)
            .unwrap_or(112)
            .clamp(1, 127) as u8;
        self.mpc_pad_press(pad, velocity, None);
        self.mpc_pad_release();
        Ok(json!({ "pad": pad, "velocity": velocity }))
    }

    fn mcp_run_command(&mut self, args: &Value) -> Result<Value, String> {
        let cmd = args
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Falta command")?;
        self.execute_command(cmd);
        Ok(json!({
            "command": cmd,
            "status": self.status_message,
        }))
    }
}

fn handle_background_mcp_call(req: &IpcRequest) -> Option<IpcResponse> {
    let result = match req.tool.as_str() {
        "list_samples" => mcp_list_samples_bg(&req.arguments),
        "browse_explorer" => mcp_browse_explorer_bg(&req.arguments),
        "analyze_sample" => mcp_analyze_sample_bg(&req.arguments),
        _ => return None,
    };
    Some(match result {
        Ok(value) => IpcResponse {
            ok: true,
            result: value,
        },
        Err(error) => IpcResponse {
            ok: false,
            result: json!(error),
        },
    })
}

fn serialize_profile(p: &SampleProfile) -> Value {
    json!({
        "summary": p.summary,
        "role_hint": p.role_hint,
        "shape": p.shape,
        "duration_seconds": p.duration_seconds,
        "sample_rate": p.sample_rate,
        "transient_label": p.transient_label,
        "transient_strength": p.transient_strength,
        "attack_ms": p.attack_ms,
        "decay_ms": p.decay_ms,
        "brightness_label": p.brightness_label,
        "spectral_centroid_hz": p.spectral_centroid_hz,
        "bands": {
            "sub_pct": p.bands.sub_pct,
            "low_pct": p.bands.low_pct,
            "mid_pct": p.bands.mid_pct,
            "high_pct": p.bands.high_pct,
        },
        "peak_dbfs": p.peak_dbfs,
        "rms_dbfs": p.rms_dbfs,
        "crest_db": p.crest_db,
        "stereo_width": p.stereo_width,
        "tonal": p.tonal,
        "pitch_note": p.pitch_note,
        "pitch_midi": p.pitch_midi,
        "pitch_cents": p.pitch_cents,
    })
}

fn matches_character(p: &SampleProfile, character: &str) -> bool {
    match character {
        "soft" => p.transient_strength < 0.45 || p.attack_ms >= 10.0,
        "punchy" => p.transient_strength >= 0.56,
        "dark" => p.spectral_centroid_hz < 1800.0,
        "bright" => p.spectral_centroid_hz >= 3200.0 || p.bands.high_pct >= 35.0,
        "tonal" => p.tonal,
        "oneshot" => p.duration_seconds < 2.2,
        "loop" => p.duration_seconds >= 2.2,
        _ => true,
    }
}

fn mcp_list_samples_bg(args: &Value) -> Result<Value, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let kind = args.get("kind").and_then(Value::as_str).unwrap_or("all");
    let extra_dir = args
        .get("directory")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let character = args
        .get("character")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let analyze = args
        .get("analyze")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || character.is_some();
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(if analyze { 16 } else { 60 })
        .clamp(1, 200) as usize;
    let scan_limit = if character.is_some() {
        (limit * 4).min(80)
    } else {
        limit
    };

    let roots = discover_library_roots(extra_dir.as_deref());
    let mut raw_items = Vec::new();
    for root in &roots {
        if raw_items.len() >= scan_limit {
            break;
        }
        collect_media_files(root, kind, query.as_deref(), scan_limit, 0, &mut raw_items);
    }

    let mut items = Vec::new();
    let mut analyzed_count = 0usize;
    let max_analyze = if character.is_some() { 40 } else { 16 };
    for mut item in raw_items {
        if items.len() >= limit {
            break;
        }
        let is_audio = item.get("kind").and_then(Value::as_str) == Some("audio");
        if analyze && is_audio && analyzed_count < max_analyze {
            if let Some(path_str) = item.get("path").and_then(Value::as_str) {
                analyzed_count += 1;
                match analyze_path(Path::new(path_str)) {
                    Ok(profile) => {
                        if let Some(ref ch) = character
                            && !matches_character(&profile, ch)
                        {
                            continue;
                        }
                        if let Some(obj) = item.as_object_mut() {
                            obj.insert("analysis".to_owned(), serialize_profile(&profile));
                        }
                    }
                    Err(err) => {
                        if character.is_some() {
                            continue;
                        }
                        if let Some(obj) = item.as_object_mut() {
                            obj.insert("analysis_error".to_owned(), json!(err));
                        }
                    }
                }
            }
        } else if character.is_some() {
            continue;
        }
        items.push(item);
    }

    Ok(json!({
        "roots": roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "count": items.len(),
        "analyzed": analyzed_count,
        "items": items,
    }))
}

fn mcp_browse_explorer_bg(args: &Value) -> Result<Value, String> {
    let dir_arg = args
        .get("directory")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let Some(dir_str) = dir_arg else {
        let roots = discover_library_roots(None);
        let mut entries = Vec::with_capacity(roots.len());
        for root in &roots {
            let (mut dirs_count, mut audio_count, mut kit_count) = (0usize, 0usize, 0usize);
            if let Ok(read) = std::fs::read_dir(root) {
                for entry in read.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        if !entry.file_name().to_string_lossy().starts_with('.') {
                            dirs_count += 1;
                        }
                    } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                        match ext.to_lowercase().as_str() {
                            "wav" | "wave" | "flac" | "mp3" | "ogg" | "oga" | "aiff" | "aif"
                            | "m4a" | "aac" => audio_count += 1,
                            "haykit" => kit_count += 1,
                            _ => {}
                        }
                    }
                }
            }
            entries.push(json!({
                "name": root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string()),
                "path": root.display().to_string(),
                "subfolders": dirs_count,
                "audio_files": audio_count,
                "kits": kit_count,
            }));
        }
        return Ok(json!({
            "mode": "roots",
            "count": entries.len(),
            "roots": entries,
        }));
    };

    let dir = Path::new(dir_str);
    if !dir.is_dir() {
        return Err(format!("La carpeta no existe: {dir_str}"));
    }
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let analyze = args
        .get("analyze")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(60)
        .clamp(1, 200) as usize;

    let read = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut folders = Vec::new();
    let mut files = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if let Some(ref q) = query
            && !name.to_lowercase().contains(q)
        {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            folders.push((name, path));
        } else {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            let kind = match ext.as_str() {
                "wav" | "wave" | "flac" | "mp3" | "ogg" | "oga" | "aiff" | "aif" | "m4a"
                | "aac" => "audio",
                "haykit" => "kit",
                "mid" | "midi" => "midi",
                _ => continue,
            };
            files.push((name, path, kind));
        }
    }
    folders.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let mut entries = Vec::new();
    for (name, path) in folders {
        if entries.len() >= limit {
            break;
        }
        entries.push(json!({
            "kind": "directory",
            "name": name,
            "path": path.display().to_string(),
        }));
    }

    let mut analyzed_count = 0usize;
    for (name, path, kind) in files {
        if entries.len() >= limit {
            break;
        }
        let mut obj = json!({
            "kind": kind,
            "name": name,
            "path": path.display().to_string(),
        });
        if analyze && kind == "audio" && analyzed_count < 12 {
            analyzed_count += 1;
            match analyze_path(&path) {
                Ok(profile) => {
                    if let Some(m) = obj.as_object_mut() {
                        m.insert("analysis".to_owned(), serialize_profile(&profile));
                    }
                }
                Err(err) => {
                    if let Some(m) = obj.as_object_mut() {
                        m.insert("analysis_error".to_owned(), json!(err));
                    }
                }
            }
        }
        entries.push(obj);
    }

    Ok(json!({
        "mode": "directory",
        "directory": dir.display().to_string(),
        "parent": dir.parent().map(|p| p.display().to_string()),
        "count": entries.len(),
        "analyzed": analyzed_count,
        "entries": entries,
    }))
}

fn mcp_analyze_sample_bg(args: &Value) -> Result<Value, String> {
    let mut paths: Vec<String> = Vec::new();
    if let Some(p) = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        paths.push(p.to_owned());
    }
    if let Some(arr) = args.get("paths").and_then(Value::as_array) {
        for item in arr {
            if let Some(p) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                if !paths.iter().any(|existing| existing == p) {
                    paths.push(p.to_owned());
                }
            }
        }
    }
    if paths.is_empty() {
        return Err("Falta path o paths (hasta 24 archivos de audio)".into());
    }
    paths.truncate(24);

    let mut results = Vec::with_capacity(paths.len());
    for path_str in &paths {
        let p = Path::new(path_str);
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_str.clone());
        match analyze_path(p) {
            Ok(profile) => results.push(json!({
                "name": name,
                "path": path_str,
                "ok": true,
                "analysis": serialize_profile(&profile),
            })),
            Err(err) => results.push(json!({
                "name": name,
                "path": path_str,
                "ok": false,
                "error": err,
            })),
        }
    }

    if results.len() == 1 {
        Ok(results.into_iter().next().unwrap())
    } else {
        Ok(json!({
            "count": results.len(),
            "samples": results,
        }))
    }
}

fn discover_library_roots(extra: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut push_unique = |p: PathBuf| {
        if p.is_dir() && !roots.contains(&p) {
            roots.push(p);
        }
    };
    if let Some(dir) = extra {
        push_unique(dir.to_path_buf());
    }
    if let Some(cfg) = dirs::config_dir() {
        let libs = cfg.join("hay-daw/libraries");
        // Cargar con el mismo formato hex UTF-8 del Explorer visual.
        if let Ok(saved) = elevate::explorer::persistence::load(&libs) {
            for p in saved {
                push_unique(p);
            }
        }
        push_unique(cfg.join("hay-daw/kits"));
    }
    if let Some(music) = dirs::audio_dir() {
        push_unique(music);
    }
    if let Some(home) = dirs::home_dir() {
        push_unique(home.join("Samples"));
        push_unique(home.join("Music"));
    }
    roots
}

fn collect_media_files(
    dir: &Path,
    kind: &str,
    query: Option<&str>,
    limit: usize,
    depth: usize,
    out: &mut Vec<Value>,
) {
    if depth > 5 || out.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= limit {
            break;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            collect_media_files(&path, kind, query, limit, depth + 1, out);
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        let item_kind = match ext.as_str() {
            "wav" | "flac" | "mp3" | "ogg" | "aiff" | "aif" | "m4a" => "audio",
            "haykit" => "kit",
            _ => continue,
        };
        if kind != "all" && kind != item_kind {
            continue;
        }
        let path_str = path.display().to_string();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(q) = query
            && !path_str.to_lowercase().contains(q)
        {
            continue;
        }
        out.push(json!({
            "kind": item_kind,
            "name": name,
            "path": path_str,
        }));
    }
}
