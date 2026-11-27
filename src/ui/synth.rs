//! Controlador UI de Hay Synth (Stage + Voice Lab + Mod Matrix, presets JSON y audio).
use crate::{
    instruments::synth::{SYNTH_ID, Synth},
    ui::DawApp,
};
use elevate::{
    devices::{
        synth::{
            EnvelopeMode, FACTORY_PRESETS, MATRIX_SLOTS, ModSlot, ModSource, ModTarget, RoomKind,
            SynthEngineKind, SynthParams, VoiceParams, cc, param, vital_factory_presets,
        },
        vital_import::{self, VitalPresetEntry},
    },
    musical::{
        midi::MidiMessage,
        model::InstrumentId,
        track::{TrackEdit, TrackKind},
    },
};
use gpui::{Bounds, Context, PathPromptOptions, Pixels, Point, Task};
use std::{cell::Cell, path::PathBuf, rc::Rc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::ui) enum SynthTab {
    #[default]
    Stage,
    Voice,
    Matrix,
}

#[derive(Clone)]
pub(in crate::ui) struct SynthPresetEntry {
    pub name: String,
    pub category: String,
    pub params: SynthParams,
}

#[derive(Clone, Copy)]
pub(in crate::ui) struct SynthKnobDrag {
    pub param_id: u16,
    pub origin_y: Pixels,
    pub start_val: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Default)]
pub(super) struct SynthUi {
    pub tab: SynthTab,
    /// Arrastre vertical de deslizador/perilla (`u16` del espacio `param::*`).
    pub knob_drag: Option<SynthKnobDrag>,
    /// Arrastre activo sobre el mapa XY (Espacio × Tono).
    pub stage_drag: bool,
    /// Límites en pantalla del mapa XY para traducir posición del ratón.
    pub stage_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Presets guardados por el usuario en `~/.local/share/jap-studio/presets/synth/`.
    pub user_presets: Vec<SynthPresetEntry>,
    pub user_presets_loaded: bool,
    /// Presets `.vital` descubiertos en `~/.local/share/vital`, `~/library` y unidades montadas.
    pub vital_library: Vec<VitalPresetEntry>,
    pub vital_library_loaded: bool,
    pub vital_index: Option<usize>,
    pub vital_picker: Option<Task<()>>,
}

impl SynthUi {
    pub fn dragging(&self) -> bool {
        self.knob_drag.is_some() || self.stage_drag
    }

    pub fn ensure_user_presets_loaded(&mut self) {
        if !self.user_presets_loaded {
            self.user_presets_loaded = true;
            self.user_presets = load_user_presets();
        }
        if !self.vital_library_loaded {
            self.vital_library_loaded = true;
            self.vital_library = vital_import::discover_vital_presets();
        }
    }

    pub fn all_presets(&self) -> Vec<SynthPresetEntry> {
        let mut list: Vec<SynthPresetEntry> = FACTORY_PRESETS
            .iter()
            .map(|p| SynthPresetEntry {
                name: p.name.to_owned(),
                category: p.category.to_owned(),
                params: p.to_params(),
            })
            .collect();
        for (name, cat, params) in vital_factory_presets() {
            list.push(SynthPresetEntry {
                name: name.to_owned(),
                category: cat.to_owned(),
                params,
            });
        }
        list.extend(self.user_presets.iter().cloned());
        list
    }
}

fn user_presets_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("jap-studio").join("presets").join("synth"))
}

fn load_user_presets() -> Vec<SynthPresetEntry> {
    let Some(dir) = user_presets_dir() else {
        return Vec::new();
    };
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let name = val
            .get("preset_name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let read_f = |k: &str, d: f32| {
            val.get(k)
                .and_then(|v| v.as_f64())
                .map(|v| (v as f32).clamp(0.0, 1.0))
                .unwrap_or(d)
        };
        let engine = SynthEngineKind::from_u8(
            val.get("engine").and_then(|v| v.as_u64()).unwrap_or(1) as u8,
        );
        let room = RoomKind::from_u8(val.get("room").and_then(|v| v.as_u64()).unwrap_or(1) as u8);
        let mode =
            EnvelopeMode::from_u8(val.get("mode").and_then(|v| v.as_u64()).unwrap_or(1) as u8);
        let mut params = SynthParams {
            engine,
            room,
            distance: read_f("distance", 0.25),
            shape: read_f("shape", 0.45),
            tone: read_f("tone", 0.55),
            body: read_f("body", 0.55),
            motion: read_f("motion", 0.30),
            mode,
            time: read_f("time", 0.50),
            touch: read_f("touch", 0.55),
            glide: read_f("glide", 0.0),
            gain: read_f("gain", 0.80),
            preset_name: name.clone(),
            voice: VoiceParams::default(),
        };
        if let Some(voice_map) = val.get("voice_params").and_then(|v| v.as_object()) {
            for (k, v) in voice_map {
                if let (Ok(pid), Some(f)) = (k.parse::<u16>(), v.as_f64()) {
                    params.set_param(pid, f as f32);
                }
            }
        }
        params.clamp_all();
        entries.push(SynthPresetEntry {
            name,
            category: "User".into(),
            params,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn live_midi_cc_for(cc_id: u8, value: f32) -> Option<MidiMessage> {
    let (midi_cc, midi_val) = match cc_id {
        cc::ENGINE => (102, (value.round() as i32).clamp(0, 7) as u8),
        cc::ROOM => (103, (value.round() as i32).clamp(0, 3) as u8),
        cc::DISTANCE => (104, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::SHAPE => (105, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::TONE => (106, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::BODY => (107, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::MOTION => (108, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::MODE => (109, (value.round() as i32).clamp(0, 2) as u8),
        cc::TIME => (110, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::TOUCH => (111, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::GLIDE => (112, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        cc::GAIN => (113, (value.clamp(0.0, 1.0) * 127.0).round() as u8),
        _ => return None,
    };
    Some(MidiMessage::ControlChange {
        cc: midi_cc,
        value: midi_val,
    })
}

fn stage_cc_for_param(param_id: u16) -> Option<u8> {
    match param_id {
        param::ENGINE => Some(cc::ENGINE),
        param::ROOM => Some(cc::ROOM),
        param::DISTANCE => Some(cc::DISTANCE),
        param::SHAPE => Some(cc::SHAPE),
        param::TONE => Some(cc::TONE),
        param::BODY => Some(cc::BODY),
        param::MOTION => Some(cc::MOTION),
        param::MODE => Some(cc::MODE),
        param::TIME => Some(cc::TIME),
        param::TOUCH => Some(cc::TOUCH),
        param::GLIDE => Some(cc::GLIDE),
        param::GAIN => Some(cc::GAIN),
        _ => None,
    }
}

impl DawApp {
    pub(in crate::ui) fn is_synth_id(&self, id: InstrumentId) -> bool {
        id == self.active_synth_id || self.synth_instances.contains_key(&id)
    }

    pub(in crate::ui) fn synth_selected(&self) -> bool {
        self.tracks
            .get(self.cursor_row)
            .and_then(|t| t.instrument)
            .is_some_and(|id| self.is_synth_id(id))
    }

    pub(in crate::ui) fn activate_synth(&mut self, id: InstrumentId) {
        if id == self.active_synth_id {
            return;
        }
        let Some(next) = self.synth_instances.remove(&id) else {
            return;
        };
        let previous = std::mem::replace(&mut self.synth, next);
        self.synth_instances.insert(self.active_synth_id, previous);
        self.active_synth_id = id;
    }

    pub(in crate::ui) fn synth_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|t| t.instrument == Some(self.active_synth_id))
    }

    pub(in crate::ui) fn synth_snapshot(&self, id: InstrumentId) -> SynthParams {
        if id == self.active_synth_id {
            self.synth.snapshot()
        } else {
            self.synth_instances
                .get(&id)
                .map(|s| s.snapshot())
                .unwrap_or_default()
        }
    }

    pub(in crate::ui) fn synth_name(&self, id: InstrumentId) -> &str {
        if id == self.active_synth_id {
            self.synth.name()
        } else {
            self.synth_instances
                .get(&id)
                .map_or("Hay Synth", |s| s.name())
        }
    }

    pub(in crate::ui) fn insert_synth_track(&mut self) {
        self.edit_track_structure(TrackEdit::Insert);
        if let Some(track) = self.tracks.get_mut(self.cursor_row) {
            track.name = "Hay Synth".into();
            track.kind = TrackKind::Synth;
            self.add_synth(self.cursor_row);
        }
    }

    pub(in crate::ui) fn add_synth(&mut self, track: usize) {
        if track >= self.tracks.len() {
            return;
        }
        if self.tracks[track].kind == TrackKind::Group {
            self.status_message = "Añade Hay Synth a una pista hija del Hilo".into();
            return;
        }
        self.stop_all();
        self.synth_ui.ensure_user_presets_loaded();
        let id = self
            .tracks
            .get(track)
            .and_then(|t| t.instrument)
            .filter(|id| self.is_synth_id(*id))
            .unwrap_or_else(|| {
                if self.tracks.iter().any(|t| t.instrument == Some(SYNTH_ID)) {
                    InstrumentId::fresh()
                } else {
                    SYNTH_ID
                }
            });
        if id != self.active_synth_id && !self.synth_instances.contains_key(&id) {
            let mut inst = Synth::default();
            inst.configure_audio(self.settings.audio.saved.clone());
            self.synth_instances.insert(id, inst);
        }
        self.tracks[track].instrument = Some(id);
        self.activate_synth(id);
        self.track_revision = self.track_revision.wrapping_add(1);
        self.cursor_row = track;
        self.open_track_instrument(self.tracks[track].id);
        self.sync_midi_focus();
        self.publish_track_fx();
        self.status_message = format!(
            "Hay Synth · {} ({}) · M activa teclado MIDI",
            self.synth.name(),
            self.synth.params.engine.label()
        );
    }

    pub(in crate::ui) fn synth_note_on(&mut self, note: u8, velocity: u8) {
        let Some(track) = self.synth_track() else {
            return;
        };
        let id = self.active_synth_id;
        if self
            .player
            .as_ref()
            .is_some_and(|player| player.has_instrument(id))
        {
            if self.synth.note_on_shared(note, velocity) {
                let sent = self.player.as_ref().is_some_and(|player| {
                    player.send_live_midi(id, MidiMessage::NoteOn { note, velocity })
                });
                if sent {
                    self.midi_capture.record(
                        MidiMessage::NoteOn { note, velocity },
                        self.session_epoch.elapsed().as_nanos() as u64,
                        self.timeline.tempo.bpm(),
                    );
                } else {
                    self.synth.note_off_shared(note);
                    self.status_message =
                        "La cola MIDI de Play está llena; vuelve a tocar la nota".into();
                }
            }
            return;
        }
        match self.synth.note_on(
            note,
            velocity,
            self.track_gain(track),
            self.utility.clone(),
            self.track_pan(track),
            self.track_fx_gain(track),
        ) {
            Ok(true) => {
                self.midi_capture.record(
                    MidiMessage::NoteOn { note, velocity },
                    self.session_epoch.elapsed().as_nanos() as u64,
                    self.timeline.tempo.bpm(),
                );
            }
            Ok(false) => {}
            Err(e) => self.status_message = format!("Hay Synth: {e}"),
        }
    }

    pub(in crate::ui) fn synth_midi(&mut self, message: MidiMessage) {
        match message {
            MidiMessage::NoteOn { note, velocity } => self.synth_note_on(note, velocity),
            MidiMessage::NoteOff { note } => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_synth_id)
                {
                    player.send_live_midi(self.active_synth_id, message);
                    self.synth.note_off_shared(note);
                } else {
                    self.synth.note_off(note);
                }
                self.midi_capture.record(
                    MidiMessage::NoteOff { note },
                    self.session_epoch.elapsed().as_nanos() as u64,
                    self.timeline.tempo.bpm(),
                );
            }
            MidiMessage::AllNotesOff => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_synth_id)
                {
                    player.send_live_midi(self.active_synth_id, message);
                }
                self.synth.all_notes_off();
            }
            MidiMessage::ControlChange { cc: c, value } => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_synth_id)
                {
                    player.send_live_midi(self.active_synth_id, message);
                } else {
                    self.synth.midi(0xb0, c, value);
                }
            }
            MidiMessage::PitchBend { bend } => {
                if let Some(player) = &self.player
                    && player.has_instrument(self.active_synth_id)
                {
                    player.send_live_midi(self.active_synth_id, message);
                } else {
                    let value = (bend as i32 + 8192).clamp(0, 16383) as u16;
                    self.synth
                        .midi(0xe0, (value & 127) as u8, (value >> 7) as u8);
                }
            }
            MidiMessage::ProgramChange { program } => {
                self.apply_synth_preset_index(usize::from(program));
            }
        }
    }

    fn push_synth_live_cc(&self, cc_id: u8, value: f32) {
        if let Some(player) = &self.player
            && player.has_instrument(self.active_synth_id)
            && let Some(msg) = live_midi_cc_for(cc_id, value)
        {
            player.send_live_midi(self.active_synth_id, msg);
        }
    }

    pub(in crate::ui) fn push_all_synth_live(&mut self) {
        self.synth.update_live();
        for (cc_id, value) in self.synth.params.live_controls() {
            self.push_synth_live_cc(cc_id, value);
        }
    }

    /// Actualiza cualquier parámetro `param::*` (Stage, Voice Lab o Mod Matrix) en vivo sin bloqueos.
    pub(in crate::ui) fn set_synth_voice_param(&mut self, param_id: u16, value: f32) {
        self.synth.params.set_param(param_id, value);
        let clamped = self.synth.params.get_param(param_id);
        self.synth.update_live_voice_param(param_id, clamped);
        if let Some(cc_id) = stage_cc_for_param(param_id) {
            self.push_synth_live_cc(cc_id, clamped);
        }
        self.track_revision = self.track_revision.wrapping_add(1);
    }

    pub(in crate::ui) fn set_synth_param(&mut self, cc_id: u8, raw_value: f32) {
        let param_id = match cc_id {
            cc::DISTANCE => param::DISTANCE,
            cc::SHAPE => param::SHAPE,
            cc::TONE => param::TONE,
            cc::BODY => param::BODY,
            cc::MOTION => param::MOTION,
            cc::TIME => param::TIME,
            cc::TOUCH => param::TOUCH,
            cc::GLIDE => param::GLIDE,
            cc::GAIN => param::GAIN,
            _ => return,
        };
        self.set_synth_voice_param(param_id, raw_value);
        let v = self.synth.params.get_param(param_id);
        let label = match cc_id {
            cc::DISTANCE => "Distance",
            cc::SHAPE => self.synth.params.engine.shape_hint(),
            cc::TONE => "Tone",
            cc::BODY => "Body",
            cc::MOTION => "Motion",
            cc::TIME => "Time",
            cc::TOUCH => "Touch",
            cc::GLIDE => "Glide",
            cc::GAIN => "Out",
            _ => "Param",
        };
        self.status_message = format!("Hay Synth · {label}: {:.0}%", v * 100.0);
    }

    pub(in crate::ui) fn set_synth_engine(&mut self, engine: SynthEngineKind) {
        self.set_synth_voice_param(param::ENGINE, engine as u8 as f32);
        self.status_message = format!(
            "Hay Synth · Motor: {} (Shape = {})",
            engine.label(),
            engine.shape_hint()
        );
    }

    pub(in crate::ui) fn set_synth_room(&mut self, room: RoomKind) {
        self.set_synth_voice_param(param::ROOM, room as u8 as f32);
        self.status_message = format!("Hay Synth · Room: {}", room.label());
    }

    pub(in crate::ui) fn set_synth_mode(&mut self, mode: EnvelopeMode) {
        self.set_synth_voice_param(param::MODE, mode as u8 as f32);
        self.status_message = format!("Hay Synth · Mode: {}", mode.label());
    }

    pub(in crate::ui) fn step_matrix_source(&mut self, slot: usize, delta: isize) {
        if slot >= MATRIX_SLOTS {
            return;
        }
        let cur = self.synth.params.voice.matrix[slot].source as u8 as isize;
        let len = ModSource::ALL.len() as isize;
        let next = (cur + delta).rem_euclid(len) as u8;
        self.set_synth_voice_param(param::matrix_source(slot), f32::from(next));
        if self.synth.params.voice.matrix[slot].amount.abs() < 0.01 && next != 0 {
            self.set_synth_voice_param(param::matrix_amount(slot), 0.35);
        }
        let s = self.synth.params.voice.matrix[slot].source.label();
        self.status_message = format!("Matrix #{} · Fuente: {s}", slot + 1);
    }

    pub(in crate::ui) fn step_matrix_target(&mut self, slot: usize, delta: isize) {
        if slot >= MATRIX_SLOTS {
            return;
        }
        let cur = self.synth.params.voice.matrix[slot].target as u8 as isize;
        let len = ModTarget::ALL.len() as isize;
        let next = (cur + delta).rem_euclid(len) as u8;
        self.set_synth_voice_param(param::matrix_target(slot), f32::from(next));
        if self.synth.params.voice.matrix[slot].amount.abs() < 0.01 && next != 0 {
            self.set_synth_voice_param(param::matrix_amount(slot), 0.35);
        }
        let t = self.synth.params.voice.matrix[slot].target.label();
        self.status_message = format!("Matrix #{} · Destino: {t}", slot + 1);
    }

    pub(in crate::ui) fn clear_matrix_slot(&mut self, slot: usize) {
        if slot >= MATRIX_SLOTS {
            return;
        }
        self.set_synth_voice_param(param::matrix_source(slot), 0.0);
        self.set_synth_voice_param(param::matrix_target(slot), 0.0);
        self.set_synth_voice_param(param::matrix_amount(slot), 0.0);
        self.status_message = format!("Matrix #{} limpiada", slot + 1);
    }

    pub(in crate::ui) fn apply_matrix_template(&mut self, template: usize) {
        let slots: &[ModSlot] = match template {
            0 => &[
                ModSlot {
                    source: ModSource::Lfo1,
                    target: ModTarget::FilterCutoff,
                    amount: 0.55,
                },
                ModSlot {
                    source: ModSource::Lfo1,
                    target: ModTarget::OscAPos,
                    amount: 0.40,
                },
                ModSlot {
                    source: ModSource::Velocity,
                    target: ModTarget::DistDrive,
                    amount: 0.35,
                },
            ],
            1 => &[
                ModSlot {
                    source: ModSource::Distance,
                    target: ModTarget::NoiseLevel,
                    amount: 0.42,
                },
                ModSlot {
                    source: ModSource::Distance,
                    target: ModTarget::ChorusMix,
                    amount: 0.50,
                },
                ModSlot {
                    source: ModSource::Distance,
                    target: ModTarget::OscADetune,
                    amount: 0.30,
                },
                ModSlot {
                    source: ModSource::Tone,
                    target: ModTarget::OscAWarp,
                    amount: 0.45,
                },
            ],
            _ => &[
                ModSlot {
                    source: ModSource::Velocity,
                    target: ModTarget::OscAPos,
                    amount: 0.55,
                },
                ModSlot {
                    source: ModSource::Velocity,
                    target: ModTarget::FilterCutoff,
                    amount: 0.45,
                },
                ModSlot {
                    source: ModSource::Env2,
                    target: ModTarget::OscAWarp,
                    amount: 0.50,
                },
                ModSlot {
                    source: ModSource::Random,
                    target: ModTarget::Pan,
                    amount: 0.25,
                },
            ],
        };
        for i in 0..MATRIX_SLOTS {
            let s = slots.get(i).copied().unwrap_or_default();
            self.set_synth_voice_param(param::matrix_source(i), s.source as u8 as f32);
            self.set_synth_voice_param(param::matrix_target(i), s.target as u8 as f32);
            self.set_synth_voice_param(param::matrix_amount(i), s.amount);
        }
        self.status_message = match template {
            0 => "Matrix · Plantilla «Vital Wobble» aplicada".into(),
            1 => "Matrix · Plantilla «XY Space Morph» aplicada (arrastra en Stage XY)".into(),
            _ => "Matrix · Plantilla «Expressive Touch» aplicada".into(),
        };
    }

    pub(in crate::ui) fn current_synth_preset_index(&self) -> Option<usize> {
        let presets = self.synth_ui.all_presets();
        presets
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&self.synth.params.preset_name))
    }

    pub(in crate::ui) fn apply_synth_preset_index(&mut self, index: usize) {
        self.synth_ui.ensure_user_presets_loaded();
        let presets = self.synth_ui.all_presets();
        let Some(entry) = presets.get(index) else {
            return;
        };
        self.synth.params = entry.params.clone();
        self.push_all_synth_live();
        self.track_revision = self.track_revision.wrapping_add(1);
        self.status_message = format!(
            "Hay Synth · {} ({} · {})",
            entry.name,
            entry.category,
            entry.params.engine.label()
        );
    }

    pub(in crate::ui) fn step_synth_preset(&mut self, delta: isize) {
        self.synth_ui.ensure_user_presets_loaded();
        let presets = self.synth_ui.all_presets();
        if presets.is_empty() {
            return;
        }
        let current = self.current_synth_preset_index().unwrap_or(0);
        let len = presets.len() as isize;
        let next = (current as isize + delta).rem_euclid(len) as usize;
        self.apply_synth_preset_index(next);
    }

    pub(in crate::ui) fn step_vital_preset(&mut self, delta: isize) {
        self.synth_ui.ensure_user_presets_loaded();
        let len = self.synth_ui.vital_library.len();
        if len == 0 {
            self.status_message =
                "No se encontraron presets .vital en librería · usa «Importar .vital»".into();
            return;
        }
        let next = match self.synth_ui.vital_index {
            Some(cur) => (cur as isize + delta).rem_euclid(len as isize) as usize,
            None => {
                if delta < 0 {
                    len - 1
                } else {
                    0
                }
            }
        };
        let entry = self.synth_ui.vital_library[next].clone();
        self.synth_ui.vital_index = Some(next);
        self.import_vital_path(entry.path);
    }

    pub(in crate::ui) fn choose_vital_file(&mut self, cx: &mut Context<Self>) {
        if self.synth_ui.vital_picker.is_some() {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importar preset .vital o banco .vitalbank".into()),
        });
        self.synth_ui.vital_picker = Some(cx.spawn(async move |this, cx| {
            let result = prompt.await;
            let _ = this.update(cx, |this, cx| {
                this.synth_ui.vital_picker = None;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            this.import_vital_path(path.clone());
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        this.status_message =
                            "No se pudo abrir el selector de archivos .vital".into()
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(in crate::ui) fn import_vital_path(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "vitalbank" || ext == "bank" || ext == "zip" {
            match vital_import::extract_vitalbank(&path) {
                Ok(entries) if !entries.is_empty() => {
                    let count = entries.len();
                    let first = entries[0].path.clone();
                    self.synth_ui.vital_library = vital_import::discover_vital_presets();
                    self.synth_ui.vital_library_loaded = true;
                    self.synth_ui.vital_index = self
                        .synth_ui
                        .vital_library
                        .iter()
                        .position(|e| e.path == first);
                    self.import_vital_path(first);
                    self.status_message = format!(
                        "Banco .vitalbank importado ({count} presets) · {}",
                        self.synth.name()
                    );
                }
                Ok(_) => {
                    self.status_message =
                        "El archivo .vitalbank no contenía presets .vital".into();
                }
                Err(e) => self.status_message = format!("Importar .vitalbank: {e}"),
            }
            return;
        }
        match vital_import::load_vital_file(&path) {
            Ok((params, summary)) => {
                self.synth.params = params;
                self.push_all_synth_live();
                self.track_revision = self.track_revision.wrapping_add(1);
                self.status_message = format!(
                    "Vital importado · {} ({}) · {} mod(s) · WT: {}",
                    summary.name, summary.style, summary.mapped_mods, summary.osc_a_wt
                );
            }
            Err(e) => self.status_message = format!("Importar .vital: {e}"),
        }
    }

    pub(in crate::ui) fn save_user_synth_preset(&mut self) {
        self.synth_ui.ensure_user_presets_loaded();
        let Some(dir) = user_presets_dir() else {
            self.status_message = "No se encontró el directorio local de presets".into();
            return;
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status_message = format!("No se pudo crear carpeta de presets: {e}");
            return;
        }
        let n = self.synth_ui.user_presets.len() + 1;
        let engine_short = self.synth.params.engine.label().replace('/', "-");
        let name = format!("Custom {engine_short} #{n}");
        self.synth.params.preset_name = name.clone();
        let p = &self.synth.params;
        let mut voice_map = serde_json::Map::new();
        for (pid, val) in p.all_param_pairs() {
            if pid >= 20 {
                voice_map.insert(pid.to_string(), serde_json::json!(val));
            }
        }
        let payload = serde_json::json!({
            "version": 2,
            "preset_name": p.preset_name,
            "engine": p.engine as u8,
            "room": p.room as u8,
            "distance": p.distance,
            "shape": p.shape,
            "tone": p.tone,
            "body": p.body,
            "motion": p.motion,
            "mode": p.mode as u8,
            "time": p.time,
            "touch": p.touch,
            "glide": p.glide,
            "gain": p.gain,
            "voice_params": voice_map,
        });
        let slug: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let file = dir.join(format!("{slug}.json"));
        match serde_json::to_string_pretty(&payload)
            .map_err(|e| e.to_string())
            .and_then(|s| std::fs::write(&file, s).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                self.synth_ui.user_presets = load_user_presets();
                self.track_revision = self.track_revision.wrapping_add(1);
                self.status_message = format!("Preset guardado · {name}");
            }
            Err(e) => self.status_message = format!("No se pudo guardar preset: {e}"),
        }
    }

    pub(in crate::ui) fn update_synth_stage_drag(&mut self, position: Point<Pixels>) -> bool {
        if !self.synth_ui.stage_drag {
            return false;
        }
        let Some(bounds) = self.synth_ui.stage_bounds.get() else {
            return false;
        };
        let w = f32::from(bounds.size.width).max(1.0);
        let h = f32::from(bounds.size.height).max(1.0);
        let nx = ((f32::from(position.x) - f32::from(bounds.origin.x)) / w).clamp(0.0, 1.0);
        let ny = ((f32::from(position.y) - f32::from(bounds.origin.y)) / h).clamp(0.0, 1.0);
        let distance = (1.0 - ny).clamp(0.0, 1.0);
        self.set_synth_voice_param(param::TONE, nx);
        self.set_synth_voice_param(param::DISTANCE, distance);
        self.status_message = format!(
            "Hay Synth · Tone {:.0}% · Distance {:.0}%",
            nx * 100.0,
            distance * 100.0
        );
        true
    }

    pub(in crate::ui) fn update_synth_drag(
        &mut self,
        position: Point<Pixels>,
        dragging: bool,
        fine: bool,
    ) -> bool {
        if !dragging {
            return self.finish_synth_drag();
        }
        if self.synth_ui.stage_drag {
            return self.update_synth_stage_drag(position);
        }
        if let Some(drag) = self.synth_ui.knob_drag {
            let scale = if fine { 900.0 } else { 150.0 };
            let span = (drag.max - drag.min).max(1e-4);
            let delta = (f32::from(drag.origin_y - position.y) / scale) * span;
            let next = (drag.start_val + delta).clamp(drag.min, drag.max);
            self.set_synth_voice_param(drag.param_id, next);
            self.status_message = format!(
                "Hay Synth · {}",
                format_synth_param_value(drag.param_id, next)
            );
            return true;
        }
        false
    }

    pub(in crate::ui) fn finish_synth_drag(&mut self) -> bool {
        let had = self.synth_ui.dragging();
        self.synth_ui.stage_drag = false;
        self.synth_ui.knob_drag = None;
        had
    }
}

pub(in crate::ui) fn format_synth_param_value(param_id: u16, value: f32) -> String {
    match param_id {
        param::OSC_A_SEMI | param::OSC_B_SEMI => format!("{:+.0} st", value.round()),
        param::OSC_B_FINE => format!("{:+.0} ct", value * 50.0),
        param::OSC_A_UNISON | param::OSC_B_UNISON => format!("{}x", value.round() as u8),
        other if other >= param::MATRIX_BASE && (other - param::MATRIX_BASE) % 3 == 2 => {
            format!("{:+.0}%", value * 100.0)
        }
        _ => format!("{:.0}%", value * 100.0),
    }
}
