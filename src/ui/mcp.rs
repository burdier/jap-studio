//! Adaptador de herramientas MCP hacia los controladores en vivo de `DawApp`.
use super::{CenterView, DawApp, fx::FxTarget, pattern_editor::DrumViewMode};
use crate::mcp::{IncomingMcpCall, IpcRequest, IpcResponse, McpHost};
use elevate::musical::{
    clip::MidiClip,
    grid::NoteGrid,
    model::{InstrumentId, NoteEvent, NoteId, Pattern},
    time::{PPQ, Tick},
    track::{MASTER_TRACK_ID, TrackEdit},
};
use gpui::{Context, Task};
use serde_json::{Value, json};
use std::sync::mpsc;

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
                    .spawn(async move { rx.recv().ok().map(|call| (call, rx)) })
                    .await;
                let Some((call, rx)) = next else {
                    break;
                };
                receiver = Some(rx);
                let ok = this
                    .update(cx, |this, cx| {
                        let response = this.handle_mcp_call(call.request);
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

    fn handle_mcp_call(&mut self, req: IpcRequest) -> IpcResponse {
        match self.dispatch_mcp(&req.tool, &req.arguments) {
            Ok(result) => IpcResponse { ok: true, result },
            Err(error) => IpcResponse {
                ok: false,
                result: json!(error),
            },
        }
    }

    fn dispatch_mcp(&mut self, tool: &str, args: &Value) -> Result<Value, String> {
        match tool {
            "get_project_state" => Ok(self.mcp_project_state()),
            "set_transport" => self.mcp_set_transport(args),
            "add_track" => self.mcp_add_track(args),
            "set_track_mix" => self.mcp_set_track_mix(args),
            "add_fx" => self.mcp_add_fx(args),
            "write_midi_clip" => self.mcp_write_midi_clip(args),
            "write_drum_pattern" => self.mcp_write_drum_pattern(args),
            "set_view" => self.mcp_set_view(args),
            "trigger_mpc_pad" => self.mcp_trigger_mpc_pad(args),
            "run_command" => self.mcp_run_command(args),
            other => Err(format!("Herramienta MCP desconocida: {other}")),
        }
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
                    .map(|n| {
                        json!({
                            "id": n.id.0,
                            "name": n.name,
                            "enabled": n.enabled,
                        })
                    })
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
            .map(|n| {
                json!({
                    "id": n.id.0,
                    "name": n.name,
                    "enabled": n.enabled,
                })
            })
            .collect();

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

        let available_fx: Vec<String> = self
            .lua_components
            .registry
            .values()
            .map(|d| format!("{} ({})", d.id, d.name))
            .collect();

        json!({
            "tempo_bpm": self.timeline.tempo.bpm(),
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
            "master_fx": master_fx,
            "available_hey_fx": available_fx,
            "drums": {
                "active_pattern_id": active_pat.id.0,
                "active_pattern_name": active_pat.name,
                "grid": format!("1/{}", active_pat.grid.denominator()),
                "step_count": active_pat.step_count(),
                "swing": active_pat.swing,
                "note_count": active_pat.notes.notes().len(),
                "mpc_bank": self.pattern_editor.mpc.bank,
                "pieces": pieces,
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

        // Alias amigables para que la IA pueda pedir "eq", "compressor", "echo", "reverb", etc.
        let resolved = match raw_id.to_lowercase().as_str() {
            "eq" | "hay eq" | "hey eq" => "hay.fx.eq",
            "comp" | "compressor" | "hay comp" | "hey comp" => "hay.fx.compressor",
            "limiter" | "hay limiter" | "hey limiter" => "hay.fx.limiter",
            "echo" | "delay" | "hay echo" | "hey echo" => "hay.fx.echo",
            "reverb" | "hay reverb" | "hey reverb" => "hay.fx.reverb",
            "double_volume" => "hay.fx.double_volume",
            "filter" => "primitive.filter",
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
        self.refresh_project_dirty();
        Ok(json!({
            "inserted": resolved,
            "target": if track_id == MASTER_TRACK_ID { "master".to_owned() } else { format!("track {}", track_id.0) },
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

        let mut max_end_ticks = 4 * PPQ; // mínimo 1 compás
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
