//! Coordinación UI → adaptadores de audio; decodificación en background.
use super::{CenterView, DawApp, pattern_editor::PatternAction};
use crate::instruments::piano_params::{self, Applies, ParamId, SPECS};
use elevate::musical::{
    controller::DEFAULT_VELOCITY,
    drums::DrumPlaybackSource,
    midi::{MidiInputEvent, MidiMessage, MidiTarget, RoutedMidi},
    midi_map::{MappedEffect, MidiAction, MidiBindTarget},
    pitch::note_name,
    track::{HiloMode, TrackKind, UNITY_LEVEL, assign_instrument, effective_mix},
};

impl DawApp {
    pub(super) fn toggle_track_mute(&mut self, track: usize) {
        let (name, muted) = {
            let Some(track_state) = self.tracks.get_mut(track) else {
                return;
            };
            track_state.muted = !track_state.muted;
            (track_state.name.clone(), track_state.muted)
        };
        self.track_revision = self.track_revision.wrapping_add(1);
        self.apply_track_gains();
        if self.pattern_editor.session.source == DrumPlaybackSource::Arrangement {
            self.publish_drum_edit();
        }
        self.status_message = format!("{name}: {}", if muted { "Mute ON" } else { "Mute OFF" });
    }

    pub(super) fn toggle_track_solo(&mut self, track: usize) {
        let (name, solo) = {
            let Some(track_state) = self.tracks.get_mut(track) else {
                return;
            };
            track_state.solo = !track_state.solo;
            (track_state.name.clone(), track_state.solo)
        };
        self.track_revision = self.track_revision.wrapping_add(1);
        self.apply_track_gains();
        if self.pattern_editor.session.source == DrumPlaybackSource::Arrangement {
            self.publish_drum_edit();
        }
        self.status_message = format!("{name}: {}", if solo { "Solo ON" } else { "Solo OFF" });
    }

    pub(super) fn toggle_track_record(&mut self, track: usize) {
        if !self.track_accepts_clips(track) {
            return;
        }
        self.cursor_row = track;
        if self.is_audio_recording()
            || self
                .tracks
                .get(track)
                .is_some_and(|t| t.kind == TrackKind::Audio && t.instrument.is_none())
        {
            self.toggle_audio_record(Some(track));
        } else {
            self.toggle_midi_record();
        }
    }

    pub(super) fn toggle_track_midi(&mut self, track: usize) {
        let Some(entry) = self.tracks.get(track) else {
            return;
        };
        if entry.instrument.is_none() && entry.hilo.is_none() {
            self.status_message =
                "Asigna un instrumento o configura un Hilo para activar MIDI".into();
            return;
        }
        if entry.hilo.is_some()
            && !self
                .tracks
                .iter()
                .any(|child| child.parent == Some(entry.id) && child.instrument.is_some())
        {
            self.status_message = "Añade instrumentos a las pistas hijas del Hilo".into();
            return;
        }
        let id = entry.id;
        let name = entry.name.to_string();
        let enabled = if self.midi_armed_tracks.insert(id) {
            true
        } else {
            self.midi_armed_tracks.remove(&id);
            false
        };
        self.sync_midi_focus();
        self.status_message = format!("{name}: MIDI {}", if enabled { "ON" } else { "OFF" });
    }

    pub(super) fn set_track_pan(&mut self, track: usize, value: i32) {
        let next = value.clamp(0, 100);
        let Some(pan) = self.track_pans.get_mut(track) else {
            return;
        };
        if *pan == next {
            return;
        }
        *pan = next;
        self.track_revision = self.track_revision.wrapping_add(1);
        self.apply_track_gains();
    }

    pub(super) fn set_track_level(&mut self, track: usize, value: i32) {
        let next = value.clamp(0, 100);
        let Some(level) = self.track_levels.get_mut(track) else {
            return;
        };
        if *level == next {
            return;
        }
        *level = next;
        self.track_revision = self.track_revision.wrapping_add(1);
        self.apply_track_gains();
    }

    pub(super) fn set_track_send(
        &mut self,
        track: usize,
        target: elevate::musical::track::TrackId,
        amount: u8,
    ) {
        let Some(entry) = self.tracks.get_mut(track) else {
            return;
        };
        if entry.is_return() || entry.id == target {
            return;
        }
        let clamped = amount.min(100);
        if entry.send_amount(target) == clamped {
            return;
        }
        entry.set_send(target, clamped);
        self.track_revision = self.track_revision.wrapping_add(1);
        self.publish_track_sends(track);
        let src_name = self.tracks[track].name.clone();
        let dst_name = self
            .tracks
            .iter()
            .find(|t| t.id == target)
            .map_or("Return", |t| t.name.as_ref());
        self.status_message = format!("↗ Envío {src_name} → {dst_name}: {clamped}%");
    }

    pub(in crate::ui) fn publish_track_sends(&self, track: usize) {
        let Some(player) = &self.player else {
            return;
        };
        let Some(entry) = self.tracks.get(track) else {
            return;
        };
        let sends: Vec<(elevate::musical::track::TrackId, f32)> = entry
            .sends
            .iter()
            .filter(|s| s.amount > 0)
            .map(|s| (s.target, s.gain()))
            .collect();
        player.set_track_sends(entry.id, &sends);
    }

    fn instrument_track(&self) -> Option<usize> {
        self.tracks
            .iter()
            .position(|track| track.instrument == Some(self.instrument.id()))
            .or_else(|| self.timeline.midi_clip.as_ref().map(|clip| clip.track))
    }

    fn now_ns(&self) -> u64 {
        self.session_epoch.elapsed().as_nanos() as u64
    }
    pub(super) fn note_on(&mut self, note: u8) {
        self.note_on_velocity(note, DEFAULT_VELOCITY);
    }
    /// Determina el cliente MIDI según el contexto actual y actualiza el servidor.
    /// Si el foco cambia desde otro instrumento/secuenciador, el servidor libera
    /// automáticamente las notas que tenía el cliente anterior.
    pub(super) fn sync_midi_focus(&mut self) {
        self.prune_vst_instances();
        if let Some(id) = self
            .tracks
            .get(self.cursor_row)
            .and_then(|track| track.instrument)
        {
            if self.is_piano_id(id) {
                self.activate_piano(id);
            } else if self.is_sampler_id(id) {
                self.activate_sampler(id);
            } else if self.is_synth_id(id) {
                self.activate_synth(id);
            } else if self.is_vst_id(id) {
                self.activate_vst(id);
            }
        }
        let (targets, mode, hilo_id) = match self.center_view {
            CenterView::Session if self.drums_panel_active() => {
                (vec![MidiTarget::Sequencer], HiloMode::Layer, None)
            }
            CenterView::Arrangement | CenterView::Mixer | CenterView::Session => {
                let armed: Vec<_> = self
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(_, track)| self.midi_armed_tracks.contains(&track.id))
                    .map(|(index, _)| index)
                    .collect();
                let selected = if armed.is_empty() {
                    vec![self.cursor_row]
                } else {
                    armed
                };
                let hilo_id = if selected.len() == 1 {
                    self.tracks
                        .get(selected[0])
                        .filter(|track| track.hilo.is_some())
                        .map(|track| track.id)
                } else {
                    None
                };
                let mode = if selected.len() == 1 {
                    self.tracks
                        .get(selected[0])
                        .and_then(|track| track.hilo)
                        .unwrap_or(HiloMode::Layer)
                } else {
                    HiloMode::Layer
                };
                let mut targets = Vec::new();
                for index in selected {
                    if let Some(track) = self.tracks.get(index) {
                        if track.hilo.is_some() {
                            targets.extend(
                                self.tracks
                                    .iter()
                                    .filter(|child| child.parent == Some(track.id))
                                    .filter_map(|child| {
                                        child.instrument.map(MidiTarget::Instrument)
                                    }),
                            );
                        } else if let Some(id) = track.instrument {
                            targets.push(MidiTarget::Instrument(id));
                        }
                    }
                }
                (targets, mode, hilo_id)
            }
        };
        let rule = if mode == HiloMode::RoundRobin {
            match hilo_id
                .map(|id| self.rule_for_hilo(id))
                .unwrap_or_else(|| self.weave_rule.clone())
            {
                Ok(rule) => rule,
                Err(error) => {
                    self.status_message = error;
                    let released = self.midi_server.focus_targets_mode([], HiloMode::Layer);
                    self.dispatch_routed_midi(released);
                    return;
                }
            }
        } else {
            None
        };
        if rule
            .as_ref()
            .is_some_and(|rule| !rule.valid_for(targets.len()))
        {
            self.status_message = "Tejer: la regla menciona una rama inexistente".into();
            let released = self.midi_server.focus_targets_mode([], HiloMode::Layer);
            self.dispatch_routed_midi(released);
            return;
        }
        let released = self.midi_server.focus_targets_mode(targets, mode);
        self.dispatch_routed_midi(released);
        match self.midi_server.set_weave_rule(rule) {
            Ok(released) => self.dispatch_routed_midi(released),
            Err(error) => self.status_message = error,
        }
    }
    pub(super) fn poll_midi_hotplug(&mut self) -> bool {
        let Some((added, removed)) = self.midi_hw.poll_hotplug() else {
            return false;
        };
        if !added.is_empty() {
            self.sync_midi_focus();
            self.status_message = format!("🎹 MIDI conectado: {}", added.join(", "));
        } else if !removed.is_empty() {
            self.status_message = format!("MIDI desconectado: {}", removed.join(", "));
        }
        true
    }

    pub(super) fn midi_live_pitch_badge(&self) -> Option<String> {
        let note = self.midi_last_note;
        let bend = self.midi_pitch_bend;
        if note.is_none() && bend.abs() < 32 {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(n) = note {
            let name = elevate::musical::pitch::note_name(n);
            let oct = i16::from(n / 12) - 1;
            parts.push(format!("{name} (C{oct})"));
        }
        if bend.abs() >= 32 {
            let semis = (f32::from(bend) / 8192.0) * 2.0;
            parts.push(format!("PB {semis:+.1}st"));
        }
        Some(parts.join(" · "))
    }

    pub(super) fn poll_live_midi(&mut self) {
        self.poll_midi_hotplug();
        let messages = self.midi_hw.drain();
        if !messages.is_empty() {
            self.apply_midi_inputs(messages);
        }
    }
    pub(super) fn focus_midi_instrument(&mut self) {
        self.sync_midi_focus();
    }
    pub(super) fn midi_target_label(&self) -> &'static str {
        match self.midi_server.active_target() {
            Some(MidiTarget::Instrument(_)) => "Instrumento",
            Some(MidiTarget::Sequencer) => "Sequencer",
            None => "Sin cliente",
        }
    }
    /// Publica mensajes de cualquier controlador en el servidor MIDI, evalúa mapeos/MIDI Learn
    /// y despacha cada evento enrutado a su cliente correspondiente.
    pub(super) fn apply_midi(&mut self, messages: impl IntoIterator<Item = MidiMessage>) {
        self.apply_midi_inputs(messages.into_iter().map(|message| MidiInputEvent {
            channel: 0,
            message,
        }));
    }

    fn apply_midi_inputs(&mut self, inputs: impl IntoIterator<Item = MidiInputEvent>) {
        let mut to_publish = Vec::new();
        let chord_midi_active = self.piano_roll.chord_pad.midi_chord_mode
            && self.midi_server.active_target() != Some(MidiTarget::Sequencer);
        for input in inputs {
            match input.message {
                MidiMessage::NoteOn { note, velocity } if velocity > 0 => {
                    self.midi_last_note = Some(note);
                    self.midi_last_vel = velocity;
                }
                MidiMessage::PitchBend { bend } => {
                    self.midi_pitch_bend = bend;
                    let semis = (f32::from(bend) / 8192.0) * 2.0;
                    if let Some(last) = self.midi_last_note {
                        let shifted = (i32::from(last) + semis.round() as i32).clamp(0, 127) as u8;
                        let oct = i16::from(shifted / 12) - 1;
                        let note_lbl = elevate::musical::pitch::note_name(shifted);
                        self.status_message = if bend.abs() < 32 {
                            format!("🎹 MIDI · {note_lbl} (C{oct}) · Pitch centro")
                        } else {
                            format!("🎹 MIDI · {note_lbl} (C{oct}) · Pitch {semis:+.1} st")
                        };
                    } else if bend.abs() >= 32 {
                        self.status_message = format!("🎹 MIDI · Pitch {semis:+.1} st");
                    }
                }
                _ => {}
            }
            let effects = self.midi_mapper.process(input.message);
            let mut learned = false;
            for effect in effects {
                if matches!(effect, MappedEffect::Learned { .. }) {
                    learned = true;
                }
                self.apply_mapped_effect(effect);
            }
            if learned {
                continue;
            }
            if chord_midi_active {
                match input.message {
                    MidiMessage::NoteOn { note, velocity } if velocity > 0 => {
                        let scale_root = self.piano_roll.scale_root;
                        let scale_minor = self.piano_roll.scale_minor;
                        let (degree, voices) = self.piano_roll.chord_pad.voices_for_midi_note(
                            note,
                            scale_root,
                            scale_minor,
                        );
                        self.piano_roll.chord_pad.select_degree(degree);
                        let roman = super::piano_roll::chord::ChordPadState::degree_roman(
                            scale_minor,
                            degree,
                        );
                        let name = super::piano_roll::chord::ChordPadState::degree_chord_name(
                            scale_root,
                            scale_minor,
                            degree,
                            self.piano_roll.chord_pad.color,
                        );
                        self.status_message = format!("🎹 Hay Chord · {roman} · {name}");
                        to_publish.extend(
                            self.piano_roll.chord_sequence.midi_expander.expand_note_on(
                                input.channel,
                                note,
                                velocity,
                                &voices,
                            ),
                        );
                        continue;
                    }
                    MidiMessage::NoteOff { note } | MidiMessage::NoteOn { note, velocity: 0 } => {
                        to_publish.extend(
                            self.piano_roll
                                .chord_sequence
                                .midi_expander
                                .expand_note_off(input.channel, note),
                        );
                        continue;
                    }
                    MidiMessage::AllNotesOff => {
                        to_publish
                            .extend(self.piano_roll.chord_sequence.midi_expander.release_all());
                    }
                    _ => {}
                }
            } else if self.piano_roll.chord_sequence.midi_expander.is_active() {
                to_publish.extend(self.piano_roll.chord_sequence.midi_expander.release_all());
            }
            to_publish.push(input);
        }
        let was_full = self.midi_server.capacity_exceeded();
        let routed = self.midi_server.publish_inputs(to_publish);
        self.dispatch_routed_midi(routed);
        if !was_full && self.midi_server.capacity_exceeded() {
            self.status_message = "Tejer: demasiadas notas activas; se liberaron las voces. Pulsa Stop para reanudar MIDI".into();
        }
    }
    fn apply_mapped_effect(&mut self, effect: MappedEffect) {
        match effect {
            MappedEffect::Learned { source, target } => {
                self.status_message = format!(
                    "🎯 MIDI Learn: {} vinculado a {}",
                    source.label(),
                    target.label()
                );
            }
            MappedEffect::SetNormalized { target, fraction } => match target {
                MidiBindTarget::TrackVolume(track_id) => {
                    if let Some(idx) = self.tracks.iter().position(|t| t.id == track_id) {
                        self.set_track_level(idx, (fraction * 100.0).round() as i32);
                        self.status_message =
                            format!("{}: {}%", self.tracks[idx].name, self.track_levels[idx]);
                    }
                }
                MidiBindTarget::TrackPan(track_id) => {
                    if let Some(idx) = self.tracks.iter().position(|t| t.id == track_id) {
                        self.set_track_pan(idx, (fraction * 100.0).round() as i32);
                        let signed = self.track_pans[idx] - 50;
                        self.status_message =
                            format!("{} · Pan: {signed:+}", self.tracks[idx].name);
                    }
                }
                MidiBindTarget::TrackSend { track, target } => {
                    if let Some(idx) = self.tracks.iter().position(|t| t.id == track) {
                        self.set_track_send(idx, target, (fraction * 100.0).round() as u8);
                    }
                }
                MidiBindTarget::MasterGain => {
                    let target_gain = (fraction * 2.0).clamp(0.0, 2.0);
                    self.change_gain(target_gain - self.utility.state().gain);
                }
                MidiBindTarget::InstrumentParam(idx) => {
                    if let Some(spec) = SPECS.get(usize::from(idx)) {
                        self.set_piano_param(spec.id, spec.from_fraction(fraction));
                    }
                }
                MidiBindTarget::FxParam { track, fx, param } => {
                    if let Some(effect) = self.vst_effects.instances.get(&fx) {
                        effect.editor.set_parameter(param, f64::from(fraction));
                    } else if let Some(component) = self.lua_components.instances.get(&fx)
                        && let Some(spec) = component
                            .descriptor()
                            .controls
                            .iter()
                            .find(|c| c.id == param)
                    {
                        let value = spec.from_fraction(fraction);
                        let _ = self.edit_fx(
                            track,
                            elevate::musical::fx::FxEdit::Parameter(fx, param, value),
                        );
                    }
                }
                MidiBindTarget::VstInstrumentParam { instrument, param } => {
                    let editor = if self.vst.id == instrument {
                        self.vst.editor()
                    } else {
                        self.vst_instances
                            .get(&instrument)
                            .and_then(|inst| inst.editor())
                    };
                    if let Some(editor) = editor {
                        editor.set_parameter(param, f64::from(fraction));
                    }
                }
                MidiBindTarget::Action(_) => {}
            },
            MappedEffect::TriggerAction(action) => match action {
                MidiAction::PlayToggle => self.play_context(),
                MidiAction::Stop => self.stop_transport(),
                MidiAction::RecordToggle => self.toggle_midi_record(),
                MidiAction::CaptureMidi => self.capture_midi_to_track(),
                MidiAction::NextBank => self.step_soundfont_bank(1),
                MidiAction::PrevBank => self.step_soundfont_bank(-1),
                MidiAction::NextPreset => self.step_soundfont_preset(1),
                MidiAction::PrevPreset => self.step_soundfont_preset(-1),
            },
        }
    }
    pub(super) fn toggle_midi_learn(&mut self) {
        let on = self.midi_mapper.toggle_learn_mode();
        self.status_message = if on {
            "🎯 MIDI Learn ACTIVO · toca un control (pista, envío, FX) o usa :learn.vst tras mover una perilla VST3"
                .into()
        } else {
            "MIDI Learn desactivado".into()
        };
    }
    pub(in crate::ui) fn arm_midi_learn(&mut self, target: MidiBindTarget) {
        self.midi_mapper.arm_target(target);
        self.status_message = format!(
            "🎯 Esperando CC/Nota MIDI para «{}» (pulsa ' para probar con CC21)…",
            target.label()
        );
    }
    pub(in crate::ui) fn last_touched_vst_target(&self) -> Option<(MidiBindTarget, String)> {
        let mut best: Option<(u64, MidiBindTarget, String)> = None;
        let mut consider = |seq: u64, target: MidiBindTarget, label: String| {
            if best
                .as_ref()
                .is_none_or(|(best_seq, _, _)| seq >= *best_seq)
            {
                best = Some((seq, target, label));
            }
        };
        if let Some(editor) = self.vst.editor()
            && let Some(touched) = editor.last_touched()
        {
            consider(
                touched.sequence,
                MidiBindTarget::VstInstrumentParam {
                    instrument: self.vst.id,
                    param: touched.id,
                },
                format!("{} · {}", self.vst.name, touched.title),
            );
        }
        for (id, inst) in &self.vst_instances {
            if let Some(editor) = inst.editor()
                && let Some(touched) = editor.last_touched()
            {
                consider(
                    touched.sequence,
                    MidiBindTarget::VstInstrumentParam {
                        instrument: *id,
                        param: touched.id,
                    },
                    format!("{} · {}", inst.name, touched.title),
                );
            }
        }
        for (&fx_id, effect) in &self.vst_effects.instances {
            if let Some(touched) = effect.editor.last_touched() {
                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.chain.find(fx_id).is_some())
                    .map(|t| t.id)
                    .unwrap_or(elevate::musical::track::MASTER_TRACK_ID);
                let fx_name = self
                    .tracks
                    .iter()
                    .find_map(|t| t.chain.find(fx_id))
                    .or_else(|| self.master_chain.find(fx_id))
                    .map(|n| n.name.as_str())
                    .unwrap_or("VST3 FX");
                consider(
                    touched.sequence,
                    MidiBindTarget::FxParam {
                        track,
                        fx: fx_id,
                        param: touched.id,
                    },
                    format!("{fx_name} · {}", touched.title),
                );
            }
        }
        best.map(|(_, target, label)| (target, label))
    }
    pub(in crate::ui) fn arm_last_touched_vst_learn(&mut self) {
        if let Some((target, label)) = self.last_touched_vst_target() {
            self.midi_mapper.arm_target(target);
            self.status_message =
                format!("🎯 Esperando CC/Nota MIDI para «{label}» (pulsa ' para probar con CC21)…");
        } else {
            self.status_message =
                "Mueve primero una perilla dentro de un plugin VST3 antes de usar Learn VST3"
                    .into();
        }
    }
    pub(in crate::ui) fn prune_midi_bindings_for(
        &mut self,
        tracks: &[elevate::musical::track::Track],
    ) {
        use std::collections::HashSet;
        let valid_tracks: HashSet<_> = tracks.iter().map(|t| t.id).collect();
        let mut valid_fx: HashSet<_> = self.vst_effects.instances.keys().copied().collect();
        valid_fx.extend(self.lua_components.instances.keys().copied());
        let mut valid_instruments: HashSet<_> =
            tracks.iter().filter_map(|t| t.instrument).collect();
        valid_instruments.insert(self.vst.id);
        valid_instruments.extend(self.vst_instances.keys().copied());
        self.midi_mapper
            .prune_orphans(&valid_tracks, &valid_fx, &valid_instruments);
    }
    pub(in crate::ui) fn dispatch_routed_midi(
        &mut self,
        routed: impl IntoIterator<Item = RoutedMidi>,
    ) {
        let selected_piano = self.instrument.id();
        for RoutedMidi { target, message } in routed {
            match target {
                MidiTarget::Instrument(id) if self.is_piano_id(id) => {
                    self.activate_piano(id);
                    match message {
                        MidiMessage::NoteOn { note, velocity } => {
                            self.piano_note_on(note, velocity)
                        }
                        MidiMessage::NoteOff { note } => self.piano_note_off(note),
                        MidiMessage::ControlChange { .. } => {
                            if let Some(player) = &self.player {
                                player.send_live_midi(id, message);
                            }
                        }
                        MidiMessage::PitchBend { bend } => {
                            if let Some(player) = &self.player {
                                player.send_live_midi(id, message);
                            }
                            let norm = (f32::from(bend) / 8192.0).clamp(-1.0, 1.0);
                            self.set_piano_param(ParamId::PitchBend, norm);
                        }
                        MidiMessage::ProgramChange { program } => {
                            if let Some(player) = &self.player {
                                player.send_live_midi(id, MidiMessage::AllNotesOff);
                                player.send_live_midi(id, message);
                            }
                            self.instrument.select_preset(usize::from(program));
                            self.track_revision = self.track_revision.wrapping_add(1);
                        }
                        MidiMessage::AllNotesOff => {
                            if let Some(player) = &self.player {
                                player.send_live_midi(id, message);
                            }
                            self.instrument.all_notes_off();
                        }
                    }
                }
                MidiTarget::Instrument(id) if self.is_sampler_id(id) => {
                    if id == self.active_sampler_id {
                        self.sampler_midi(message);
                    } else if let Some(sampler) = self.sampler_instances.get_mut(&id) {
                        let now_ns = self.session_epoch.elapsed().as_nanos() as u64;
                        let bpm = self.timeline.tempo.bpm();
                        match message {
                            MidiMessage::NoteOn { note, velocity } => {
                                if let Some(player) = &self.player
                                    && player.has_instrument(id)
                                {
                                    if sampler.note_on_shared(note) {
                                        if player.send_live_midi(id, message) {
                                            self.midi_capture.record(message, now_ns, bpm);
                                        } else {
                                            sampler.note_off_shared(note);
                                        }
                                    }
                                } else {
                                    sampler.note_on(note, velocity);
                                    self.midi_capture.record(message, now_ns, bpm);
                                }
                            }
                            MidiMessage::NoteOff { note } => {
                                if let Some(player) = &self.player
                                    && player.has_instrument(id)
                                {
                                    player.send_live_midi(id, message);
                                    sampler.note_off_shared(note);
                                } else {
                                    sampler.note_off(note);
                                }
                                self.midi_capture.record(message, now_ns, bpm);
                            }
                            MidiMessage::AllNotesOff => {
                                if let Some(player) = &self.player
                                    && player.has_instrument(id)
                                {
                                    player.send_live_midi(id, message);
                                }
                                sampler.all_off();
                                self.midi_capture.record(message, now_ns, bpm);
                            }
                            _ => {}
                        }
                    }
                }
                MidiTarget::Instrument(id) if self.is_synth_id(id) => {
                    if id == self.active_synth_id {
                        self.synth_midi(message);
                    } else if self.synth_instances.contains_key(&id) {
                        let mix = self
                            .tracks
                            .iter()
                            .position(|t| t.instrument == Some(id))
                            .map(|t| {
                                (self.track_gain(t), self.track_pan(t), self.track_fx_gain(t))
                            });
                        let utility = self.utility.clone();
                        let player_has = self
                            .player
                            .as_ref()
                            .is_some_and(|player| player.has_instrument(id));
                        let now_ns = self.session_epoch.elapsed().as_nanos() as u64;
                        let bpm = self.timeline.tempo.bpm();
                        if let Some(synth) = self.synth_instances.get_mut(&id) {
                            match message {
                                MidiMessage::NoteOn { note, velocity } => {
                                    if player_has {
                                        if synth.note_on_shared(note, velocity) {
                                            if self
                                                .player
                                                .as_ref()
                                                .is_some_and(|p| p.send_live_midi(id, message))
                                            {
                                                self.midi_capture.record(message, now_ns, bpm);
                                            } else {
                                                synth.note_off_shared(note);
                                            }
                                        }
                                    } else if let Some((gain, pan, fx)) = mix {
                                        if matches!(
                                            synth.note_on(note, velocity, gain, utility, pan, fx),
                                            Ok(true)
                                        ) {
                                            self.midi_capture.record(message, now_ns, bpm);
                                        }
                                    }
                                }
                                MidiMessage::NoteOff { note } => {
                                    if player_has {
                                        if let Some(player) = &self.player {
                                            player.send_live_midi(id, message);
                                        }
                                        synth.note_off_shared(note);
                                    } else {
                                        synth.note_off(note);
                                    }
                                    self.midi_capture.record(message, now_ns, bpm);
                                }
                                MidiMessage::AllNotesOff => {
                                    if player_has && let Some(player) = &self.player {
                                        player.send_live_midi(id, message);
                                    }
                                    synth.all_notes_off();
                                    self.midi_capture.record(message, now_ns, bpm);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                MidiTarget::Instrument(id) if self.is_vst_id(id) => {
                    self.vst_midi_to(id, message);
                }
                MidiTarget::Instrument(_) => {}
                MidiTarget::Sequencer => {
                    if !self.pattern_editor.mixer_open {
                        if let Some(piece) = self.pattern_editor.roll.piece {
                            if let MidiMessage::NoteOn { note, velocity } = message {
                                self.trigger_drum_audition(piece, note, velocity);
                            }
                        } else if self.pattern_editor.view_mode
                            == super::pattern_editor::DrumViewMode::Pads
                        {
                            match message {
                                MidiMessage::NoteOn { note, velocity } => {
                                    self.handle_mpc_midi_note(note, velocity, true);
                                }
                                MidiMessage::NoteOff { note } => {
                                    self.handle_mpc_midi_note(note, 0, false);
                                }
                                MidiMessage::AllNotesOff => self.mpc_pad_release(),
                                _ => {}
                            }
                        } else if let MidiMessage::NoteOn { note, velocity } = message {
                            if !self.handle_mpc_midi_note(note, velocity, true) {
                                self.apply_pattern_action(PatternAction::Toggle {
                                    lane: self.pattern_editor.cursor_lane,
                                    step: self.pattern_editor.cursor_step,
                                });
                                let editor = &mut self.pattern_editor;
                                editor.cursor_step = (editor.cursor_step + 1) % editor.step_count();
                            }
                        }
                    }
                }
            }
        }
        self.activate_piano(selected_piano);
    }
    pub(super) fn controller_status(&self) -> String {
        if self.controller.enabled() {
            format!(
                "Teclado MIDI activado → {} · octava {} ({}) · velocity {}",
                self.midi_target_label(),
                self.controller.octave(),
                self.controller.base_label(),
                self.controller.velocity()
            )
        } else {
            "Teclado MIDI desactivado".into()
        }
    }
    /// Teclas del controlador para cualquier ventana. `true` si la tecla se consumió.
    pub(super) fn controller_key_down(&mut self, key: &str, repeat: bool) -> bool {
        let Some(messages) = self.controller.key_down(key, repeat) else {
            return false;
        };
        self.apply_midi(messages);
        self.status_message = self.controller_status();
        true
    }
    pub(super) fn toggle_controller(&mut self) {
        let messages = self.controller.set_enabled(!self.controller.enabled());
        self.apply_midi(messages);
        self.status_message = self.controller_status();
    }
    fn note_on_velocity(&mut self, note: u8, velocity: u8) {
        if self.vst_selected() {
            self.vst_midi(MidiMessage::NoteOn { note, velocity });
            return;
        }
        if self.sampler_selected() {
            self.sample_note_on(note, velocity);
            return;
        }
        if self.synth_selected() {
            self.synth_note_on(note, velocity);
            return;
        }
        self.sampler.silence();
        self.piano_note_on(note, velocity);
    }
    fn piano_note_on(&mut self, note: u8, velocity: u8) {
        self.publish_track_fx();
        if note > 127 || self.instrument.held(note) {
            return;
        }
        let id = self.instrument.id();
        if self
            .player
            .as_ref()
            .is_some_and(|player| player.has_instrument(id))
        {
            if self.instrument.note_on_shared(note) {
                let sent = self.player.as_ref().is_some_and(|player| {
                    player.send_live_midi(id, MidiMessage::NoteOn { note, velocity })
                });
                if sent {
                    self.midi_capture.record(
                        MidiMessage::NoteOn { note, velocity },
                        self.now_ns(),
                        self.timeline.tempo.bpm(),
                    );
                    self.status_message = format!(
                        "Note On {} · {}",
                        note_name(note),
                        self.instrument.active_name()
                    );
                } else {
                    self.instrument.note_off_shared(note);
                    self.status_message =
                        "La cola MIDI de Play está llena; vuelve a tocar la nota".into();
                }
            }
            return;
        }
        let now = self.now_ns();
        self.midi_capture.record(
            MidiMessage::NoteOn { note, velocity },
            now,
            self.timeline.tempo.bpm(),
        );
        match self
            .instrument
            .note_on(note, velocity, self.piano_gain(), self.utility.clone())
        {
            Ok(true) => {
                self.status_message = format!(
                    "Note On {} · {}",
                    note_name(note),
                    self.instrument.active_name()
                );
            }
            Ok(false) => {}
            Err(error) => {
                self.status_message = format!(
                    "No se pudo iniciar {}: {error}",
                    self.instrument.active_name()
                );
            }
        }
    }
    pub(super) fn note_off(&mut self, note: u8) {
        if self.vst.held.contains(&note) {
            self.vst_midi_to(self.vst.id, MidiMessage::NoteOff { note });
            return;
        }
        for vst in self.vst_instances.values_mut() {
            if vst.held.contains(&note) {
                vst.midi(0x80, note, 64);
                return;
            }
        }
        if self.sampler.held(note) {
            self.sampler.note_off(note);
            self.midi_capture.record(
                MidiMessage::NoteOff { note },
                self.now_ns(),
                self.timeline.tempo.bpm(),
            );
            return;
        }
        if self.synth.held(note) {
            self.synth_midi(MidiMessage::NoteOff { note });
            return;
        }
        self.piano_note_off(note);
    }
    fn piano_note_off(&mut self, note: u8) {
        let id = self.instrument.id();
        let shared = self
            .player
            .as_ref()
            .is_some_and(|player| player.has_instrument(id));
        let released = if shared {
            self.instrument.note_off_shared(note)
        } else {
            self.instrument.note_off(note)
        };
        if !released {
            return;
        }
        if shared && let Some(player) = &self.player {
            player.send_live_midi(id, MidiMessage::NoteOff { note });
        }
        let now = self.now_ns();
        self.midi_capture.record(
            MidiMessage::NoteOff { note },
            now,
            self.timeline.tempo.bpm(),
        );
        self.status_message = format!(
            "Note Off {} · {}",
            note_name(note),
            self.instrument.active_name()
        );
    }
    pub(super) fn release_mouse_note(&mut self) {
        if let Some(note) = self.vst.mouse.take() {
            self.vst_midi_to(self.vst.id, MidiMessage::NoteOff { note });
        }
        for vst in self.vst_instances.values_mut() {
            if let Some(note) = vst.mouse.take() {
                vst.midi(0x80, note, 64);
            }
        }
        if let Some(note) = self.sampler.mouse.take() {
            if self
                .player
                .as_ref()
                .is_some_and(|player| player.has_instrument(self.active_sampler_id))
            {
                if let Some(player) = &self.player {
                    player.send_live_midi(self.active_sampler_id, MidiMessage::NoteOff { note });
                }
                self.sampler.note_off_shared(note);
            } else {
                self.sampler.note_off(note);
            }
        }
        if let Some(note) = self.synth.mouse.take() {
            self.synth_midi(MidiMessage::NoteOff { note });
        }
        for synth in self.synth_instances.values_mut() {
            if let Some(note) = synth.mouse.take() {
                synth.note_off(note);
            }
        }
        if let Some(note) = self.instrument.take_mouse_note() {
            self.piano_note_off(note);
        }
        let selected_piano = self.instrument.id();
        let released: Vec<_> = self
            .piano_instances
            .iter_mut()
            .filter_map(|(id, piano)| piano.take_mouse_note().map(|note| (*id, note)))
            .collect();
        for (id, note) in released {
            self.activate_piano(id);
            self.piano_note_off(note);
        }
        self.activate_piano(selected_piano);
    }
    pub(super) fn all_notes_off(&mut self) {
        let _ = self.piano_roll.chord_sequence.audition.release();
        if let Some(player) = &self.player {
            player.all_live_notes_off();
        }
        self.vst.all_off();
        for vst in self.vst_instances.values_mut() {
            vst.all_off();
        }
        self.sampler.all_off();
        self.synth.all_notes_off();
        for synth in self.synth_instances.values_mut() {
            synth.all_notes_off();
        }
        let controller_off = self.controller.release_all();
        self.apply_midi(controller_off);
        let server_off = self.midi_server.release_all();
        self.dispatch_routed_midi(server_off);
        let now = self.now_ns();
        self.midi_capture
            .record(MidiMessage::AllNotesOff, now, self.timeline.tempo.bpm());
        if self.instrument.all_notes_off() {
            self.status_message = "Todas las notas liberadas".into();
        }
        for piano in self.piano_instances.values_mut() {
            piano.all_notes_off();
        }
    }
    /// Recupera el MIDI tocado en el búfer del controlador y lo inserta como `MidiClip`
    /// en la pista seleccionada (o en la pista asignada al instrumento).
    pub(super) fn capture_midi_to_track(&mut self) {
        let now = self.now_ns();
        let target_track = self.cursor_row;
        if !self.track_accepts_clips(target_track) {
            self.status_message = "Selecciona una pista de audio/MIDI".into();
            return;
        }
        let start_beat = self.timeline.edit_beat();
        let title = format!("MIDI · {}", self.active_instrument_name());
        let Some(clip) = self.midi_capture.to_clip_with_meter(
            title,
            target_track,
            start_beat,
            self.active_instrument_id(),
            now,
            self.timeline.tempo.bpm(),
            self.timeline.meter,
        ) else {
            self.status_message =
                "No hay notas MIDI recientes para insertar · toca algo primero".into();
            return;
        };
        let note_count = clip.notes().len();
        let duration = clip.duration_beats();
        if self.tracks[target_track].hilo.is_none()
            && self.tracks[target_track].instrument.is_none()
        {
            let id = self.active_instrument_id();
            if self.is_piano_id(id) {
                self.assign_piano_instance(target_track);
            } else {
                assign_instrument(&mut self.tracks, target_track, id);
            }
            self.track_revision = self.track_revision.wrapping_add(1);
        }
        self.timeline.insert_midi_clip(clip);
        self.sync_midi_focus();
        self.status_message = format!(
            "Clip MIDI insertado en {} ({note_count} notas, {duration:.1} negras) · Space: reproducir",
            self.tracks[target_track].name
        );
    }

    pub(in crate::ui) fn clear_midi_capture_buffer(&mut self) {
        let count = self.midi_capture.note_count();
        self.midi_capture.clear();
        self.status_message = if count > 0 {
            format!("Búfer de recuperación MIDI vaciado ({count} notas)")
        } else {
            "El búfer de recuperación MIDI ya estaba vacío".into()
        };
    }
    pub(in crate::ui) fn toggle_metronome(&mut self) {
        let on = self.metronome.toggle();
        let pct = (self.metronome.gain() * 100.0).round() as u32;
        self.status_message = if on {
            format!("♩ Metrónomo ON ({pct}% · Ctrl+K)")
        } else {
            "♩ Metrónomo OFF".into()
        };
    }

    pub(in crate::ui) fn cycle_count_in(&mut self) {
        let bars = self.metronome.cycle_count_in();
        self.status_message = match bars {
            0 => "Pre-cuenta desactivada".into(),
            1 => "Pre-cuenta: 1 compás al grabar".into(),
            n => format!("Pre-cuenta: {n} compases al grabar"),
        };
    }

    pub(in crate::ui) fn set_count_in_bars(&mut self, bars: u32) {
        let clamped = bars.min(2);
        self.metronome.set_count_in_bars(clamped);
        self.status_message = match clamped {
            0 => "Pre-cuenta desactivada".into(),
            1 => "Pre-cuenta: 1 compás al grabar".into(),
            n => format!("Pre-cuenta: {n} compases al grabar"),
        };
    }

    pub(in crate::ui) fn step_metronome_gain(&mut self, delta: f32) {
        let next = (self.metronome.gain() + delta).clamp(0.0, 1.5);
        self.metronome.set_gain(next);
        self.status_message = format!("♩ Volumen del metrónomo: {:.0}%", next * 100.0);
    }

    pub(in crate::ui) fn set_metronome_gain(&mut self, gain: f32) {
        self.metronome.set_gain(gain);
        self.status_message = format!(
            "♩ Volumen del metrónomo: {:.0}%",
            self.metronome.gain() * 100.0
        );
    }

    /// Activa o detiene la grabación explícita de la salida MIDI hacia un `MidiClip`.
    /// Si la pre-cuenta está activa (1 o 2 compases), inicia el transporte con pre-cuenta RT.
    pub(super) fn toggle_midi_record(&mut self) {
        if !self.midi_capture.is_recording() && !self.track_accepts_clips(self.cursor_row) {
            self.status_message = "Selecciona una pista hija para grabar MIDI".into();
            return;
        }
        if self.midi_capture.is_recording() {
            let now = self.now_ns();
            self.recording_waiting_count_in = false;
            self.metronome.set_keep_running(false);
            self.metronome.clear_count_in();
            self.midi_capture
                .stop_recording(now, self.timeline.tempo.bpm());
            if self.player.is_some() || self.file_playback.is_some() {
                self.stop_audio();
            }
            if self.midi_capture.has_notes() {
                self.capture_midi_to_track();
            } else {
                self.status_message = "Grabación MIDI detenida (sin notas)".into();
            }
        } else {
            self.all_notes_off();
            self.midi_capture.start_recording();
            self.metronome.set_keep_running(true);
            if !self.controller.enabled() {
                let _ = self.controller.set_enabled(true);
            }
            self.focus_midi_instrument();
            let bars = self.metronome.count_in_bars();
            if self.player.is_none() && self.file_playback.is_none() {
                self.recording_waiting_count_in = bars > 0;
                self.publish_track_fx();
                self.stop_pattern();
                self.stop_instrument();
                self.request_file_playback_with_count_in(self.timeline.edit_beat(), bars);
            } else {
                self.recording_waiting_count_in = false;
                self.status_message = format!(
                    "● Grabando MIDI en {} · toca y pulsa REC / Espacio para insertar el clip",
                    self.tracks[self.cursor_row].name
                );
            }
        }
    }
    pub(super) fn set_piano_param(&mut self, id: ParamId, value: f32) {
        let restart = self.instrument.set_param(id, value);
        self.track_revision = self.track_revision.wrapping_add(1);
        let spec = piano_params::spec_of(id);
        self.status_message = format!(
            "{} = {}{}",
            spec.label,
            spec.display(self.instrument.param(id)),
            if restart {
                " · se aplica al siguiente note-on (notas cortadas)"
            } else if spec.applies == Applies::Restart {
                " · se aplica al abrir el piano"
            } else {
                ""
            }
        );
    }
    pub(super) fn step_soundfont_bank(&mut self, delta: isize) {
        self.all_notes_off();
        match self.instrument.step_bank(delta) {
            Ok(()) => {
                self.track_revision = self.track_revision.wrapping_add(1);
                self.status_message = format!(
                    "Banco: {} ({}/{}) · {}",
                    self.instrument.active_name(),
                    self.instrument.bank_index() + 1,
                    self.instrument.banks().len(),
                    self.instrument.active_credits()
                );
            }
            Err(error) => self.status_message = format!("Banco SoundFont: {error}"),
        }
    }
    pub(super) fn poll_piano_resources(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(result) = self.instrument.poll_resources() {
            self.status_message = match result {
                Ok(()) => format!("{} · listo", self.instrument.active_name()),
                Err(error) => format!("Piano: {error}"),
            };
            cx.notify();
        }
        for piano in self.piano_instances.values_mut() {
            if let Some(result) = piano.poll_resources() {
                self.status_message = match result {
                    Ok(()) => format!("{} · listo", piano.active_name()),
                    Err(error) => format!("Piano: {error}"),
                };
                cx.notify();
            }
        }
    }
    pub(super) fn select_soundfont_bank(&mut self, index: usize) {
        self.all_notes_off();
        match self.instrument.select_bank(index) {
            Ok(()) => {
                self.track_revision = self.track_revision.wrapping_add(1);
                self.status_message = format!(
                    "Banco: {} · {}",
                    self.instrument.active_name(),
                    self.instrument.active_credits()
                );
            }
            Err(error) => self.status_message = format!("Banco SoundFont: {error}"),
        }
    }
    pub(super) fn step_soundfont_preset(&mut self, delta: isize) {
        self.instrument.step_preset(delta);
        self.track_revision = self.track_revision.wrapping_add(1);
        if let Some(preset) = self
            .instrument
            .presets()
            .get(self.instrument.preset_index())
        {
            self.status_message = format!(
                "Preset: {:03}:{:03} {} ({}/{})",
                preset.bank,
                preset.patch,
                preset.name,
                self.instrument.preset_index() + 1,
                self.instrument.presets().len()
            );
        }
    }
    pub(super) fn stop_instrument(&mut self) {
        let _ = self.piano_roll.chord_sequence.audition.release();
        self.vst.silence();
        for vst in self.vst_instances.values_mut() {
            vst.silence();
        }
        self.sampler.silence();
        for sampler in self.sampler_instances.values_mut() {
            sampler.silence();
        }
        self.synth.silence();
        for synth in self.synth_instances.values_mut() {
            synth.silence();
        }
        self.instrument.stop();
        for piano in self.piano_instances.values_mut() {
            piano.stop();
        }
    }
    /// Audición en loop de la fuente elegida. Las ediciones publican un plan en vivo.
    pub(super) fn play_pattern(&mut self) {
        let length = match self.pattern_editor.session.playback_length() {
            Ok(Some(length)) => length,
            Ok(None) => return self.play_audio(),
            Err(error) => {
                self.status_message = error;
                return;
            }
        };
        self.stop_audio();
        self.stop_instrument();
        self.pattern_loop_beats = Some(length.0 as f64 / elevate::musical::time::PPQ as f64);
        self.request_file_playback(0.0);
    }
    pub(super) fn stop_pattern(&mut self) {
        if self.pattern_loop_beats.take().is_some() {
            self.file_playback = None;
            self.player = None;
            self.timeline.playhead = None;
        }
    }
    pub(super) fn play_context(&mut self) {
        if self.center_view == CenterView::Session {
            self.play_pattern();
        } else {
            self.play_audio();
        }
    }
    pub(super) fn stop_all(&mut self) {
        if self.pattern_loop_beats.is_none()
            && let Some(player) = &self.player
        {
            self.timeline.last_stop_beat = Some(player.beat());
        }
        self.controller.release_all();
        self.midi_server.release_all();
        self.stop_audio();
        self.stop_pattern();
        self.stop_instrument();
    }
    pub(super) fn stop_transport(&mut self) {
        let was_recording = self.midi_capture.is_recording();
        let was_audio_recording = self.is_audio_recording();
        self.recording_waiting_count_in = false;
        self.metronome.set_keep_running(false);
        self.metronome.clear_count_in();
        let player = self.player.take();
        if self.pattern_loop_beats.is_none()
            && let Some(player) = &player
        {
            self.timeline.last_stop_beat = Some(player.beat());
        }
        if let Some(player) = &player {
            player.stop_with_tail();
        }
        self.stop_all();
        self.tail_player = player;
        if was_audio_recording {
            self.finish_audio_recording();
        } else if was_recording {
            let now = self.now_ns();
            self.midi_capture
                .stop_recording(now, self.timeline.tempo.bpm());
            if self.midi_capture.has_notes() {
                self.capture_midi_to_track();
            } else {
                self.status_message = "Grabación detenida (sin notas)".into();
            }
        } else {
            self.status_message = "Detenido".into();
        }
    }
    pub(super) fn play_audio(&mut self) {
        self.play_audio_from(self.timeline.edit_beat());
    }

    pub(super) fn play_audio_from(&mut self, beat: f64) {
        self.publish_track_fx();
        self.stop_pattern();
        self.stop_audio();
        self.stop_instrument();
        self.request_file_playback(beat);
    }

    pub(super) fn stop_audio(&mut self) {
        self.metronome.clear_count_in();
        self.recording_waiting_count_in = false;
        self.vst.stop_clip();
        for vst in self.vst_instances.values() {
            vst.stop_clip();
        }
        self.file_playback = None;
        self.player = None;
        self.tail_player = None;
        self.instrument.stop_clip();
        for piano in self.piano_instances.values_mut() {
            piano.stop_clip();
        }
        self.sampler.stop_clip();
        self.timeline.playhead = None;
        self.pattern_loop_beats = None;
    }
    pub(super) fn change_gain(&mut self, delta: f32) {
        let gain = (self.utility.state().gain + delta).clamp(0.0, 2.0);
        self.apply_utility_action(elevate::devices::utility_slot::UtilityAction::SetGain(gain));
    }
    /// Fader de pista (`TRACK_UNITY_LEVEL` = 1.0x). Utility es una etapa posterior independiente.
    pub(super) fn track_gain(&self, track: usize) -> f32 {
        effective_mix(&self.tracks, &self.track_levels, &self.track_pans, track).gain
    }
    pub(super) fn arrangement_track_mix(&self, index: usize) -> (f32, f32, bool, bool) {
        let track = &self.tracks[index];
        let own = effective_mix(&self.tracks, &self.track_levels, &self.track_pans, index).audible;
        let audible = own
            || (track.kind == TrackKind::Effect && !track.muted)
            || (track.kind == TrackKind::Group
                && !track.muted
                && self
                    .tracks
                    .iter()
                    .any(|child| child.parent == Some(track.id) && child.solo && !child.muted));
        let gain = if audible {
            self.track_levels
                .get(index)
                .copied()
                .unwrap_or(UNITY_LEVEL)
                .clamp(0, 100) as f32
                / UNITY_LEVEL as f32
        } else {
            0.0
        };
        let pan = (self
            .track_pans
            .get(index)
            .copied()
            .unwrap_or(50)
            .clamp(0, 100)
            - 50) as f32
            / 50.0;
        (gain, pan, track.kind != TrackKind::Group || own, audible)
    }
    pub(in crate::ui) fn publish_track_automation(&self, id: elevate::musical::track::TrackId) {
        if let Some(player) = &self.player {
            player.set_track_automation(id, self.compiled_track_automation(id));
        }
    }
    pub(in crate::ui) fn publish_all_automation(&self) {
        if let Some(player) = &self.player {
            for track in &self.tracks {
                player.set_track_automation(track.id, self.compiled_track_automation(track.id));
            }
        }
    }
    fn piano_gain(&self) -> f32 {
        let id = self.instrument.id();
        let track = self
            .tracks
            .iter()
            .position(|t| t.instrument == Some(id))
            .or_else(|| self.timeline.midi_clip.as_ref().map(|c| c.track));
        track.map_or(1.0, |t| self.track_gain(t))
    }
    /// Empuja a los streams activos la ganancia de su pista (el `Volume` lee el atómico por bloque).
    pub(super) fn apply_track_gains(&mut self) {
        if let Some(track) = self.sampler_track() {
            self.sampler.mix(
                self.sampler_gain(track),
                self.track_pan(track),
                self.track_fx_gain(track),
            );
        }
        if let Some(track) = self.synth_track() {
            self.synth.mix(
                self.track_gain(track),
                self.track_pan(track),
                self.track_fx_gain(track),
            );
        }
        let synth_mix: Vec<_> = self
            .synth_instances
            .keys()
            .filter_map(|id| {
                self.tracks
                    .iter()
                    .position(|track| track.instrument == Some(*id))
                    .map(|index| {
                        (
                            *id,
                            self.track_gain(index),
                            self.track_pan(index),
                            self.track_fx_gain(index),
                        )
                    })
            })
            .collect();
        for (id, gain, pan, fx) in synth_mix {
            if let Some(synth) = self.synth_instances.get(&id) {
                synth.mix(gain, pan, fx);
            }
        }
        if let Some(player) = &self.player {
            for (index, track) in self.tracks.iter().enumerate() {
                let (gain, pan, source_audible, audible) = self.arrangement_track_mix(index);
                player.set_gain(track.id, gain);
                player.set_pan(track.id, pan);
                player.set_source_audible(track.id, source_audible);
                player.set_audible(track.id, audible);
            }
        }
        self.instrument.set_gain(self.piano_gain());
        self.instrument.set_pan(self.piano_pan());
        let piano_mix: Vec<_> = self
            .piano_instances
            .keys()
            .filter_map(|id| {
                self.tracks
                    .iter()
                    .position(|track| track.instrument == Some(*id))
                    .map(|index| (*id, self.track_gain(index), self.track_pan(index)))
            })
            .collect();
        for (id, gain, pan) in piano_mix {
            if let Some(piano) = self.piano_instances.get_mut(&id) {
                piano.set_gain(gain);
                piano.set_pan(pan);
            }
        }
        self.publish_track_fx();
    }

    pub(in crate::ui) fn track_pan(&self, index: usize) -> f32 {
        effective_mix(&self.tracks, &self.track_levels, &self.track_pans, index).pan
    }
    fn piano_pan(&self) -> f32 {
        let Some(track) = self.instrument_track() else {
            return 0.0;
        };
        self.track_pan(track)
    }
    pub(super) fn toggle_bypass(&mut self) {
        self.apply_utility_action(elevate::devices::utility_slot::UtilityAction::ToggleBypass);
    }
}
