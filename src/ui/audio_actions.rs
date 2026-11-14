//! Coordinación UI → adaptadores de audio; decodificación en background.
use super::{CenterView, DawApp, pattern_editor::PatternAction};
use crate::instruments::piano_params::{self, Applies, ParamId, SPECS};
use elevate::musical::{
    controller::DEFAULT_VELOCITY,
    midi::{MidiMessage, MidiTarget, RoutedMidi},
    midi_map::{MappedEffect, MidiAction, MidiBindTarget},
    pitch::note_name,
    track::{TrackKind, assign_instrument, effective_mix},
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
        self.status_message = format!("{name}: {}", if solo { "Solo ON" } else { "Solo OFF" });
    }

    pub(super) fn toggle_track_record(&mut self, track: usize) {
        if !self.track_accepts_clips(track) {
            return;
        }
        self.cursor_row = track;
        self.toggle_midi_record();
    }

    pub(super) fn set_track_pan(&mut self, track: usize, value: i32) {
        let Some(pan) = self.track_pans.get_mut(track) else {
            return;
        };
        let next = value.clamp(0, 100);
        if *pan == next {
            return;
        }
        *pan = next;
        self.track_revision = self.track_revision.wrapping_add(1);
        self.apply_track_gains();
    }

    pub(super) fn set_track_level(&mut self, track: usize, value: i32) {
        let Some(level) = self.track_levels.get_mut(track) else {
            return;
        };
        let next = value.clamp(0, 100);
        if *level == next {
            return;
        }
        *level = next;
        self.track_revision = self.track_revision.wrapping_add(1);
        self.apply_track_gains();
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
            if self.is_sampler_id(id) {
                self.activate_sampler(id);
            } else if self.is_vst_id(id) {
                self.activate_vst(id);
            }
        }
        let target = match self.center_view {
            CenterView::Session => Some(MidiTarget::Sequencer),
            CenterView::Arrangement | CenterView::Mixer => self
                .tracks
                .get(self.cursor_row)
                .and_then(|track| track.instrument)
                .map(MidiTarget::Instrument),
        };
        let released = self.midi_server.focus(target);
        self.dispatch_routed_midi(released);
    }
    pub(super) fn poll_live_midi(&mut self) {
        let messages = self.midi_hw.drain();
        if !messages.is_empty() {
            self.apply_midi(messages);
        }
    }
    pub(super) fn focus_midi_instrument(&mut self) {
        let released = self
            .midi_server
            .focus(Some(MidiTarget::Instrument(self.active_instrument_id())));
        self.dispatch_routed_midi(released);
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
        let mut to_publish = Vec::new();
        for msg in messages {
            let effects = self.midi_mapper.process(msg);
            let mut learned = false;
            for effect in effects {
                if matches!(effect, MappedEffect::Learned { .. }) {
                    learned = true;
                }
                self.apply_mapped_effect(effect);
            }
            if !learned {
                to_publish.push(msg);
            }
        }
        let routed = self.midi_server.publish(to_publish);
        self.dispatch_routed_midi(routed);
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
                MidiBindTarget::TrackVolume(idx) if idx < self.track_levels.len() => {
                    self.set_track_level(idx, (fraction * 100.0).round() as i32);
                    self.status_message =
                        format!("{}: {}%", self.tracks[idx].name, self.track_levels[idx]);
                }
                MidiBindTarget::TrackPan(idx) if idx < self.track_pans.len() => {
                    self.set_track_pan(idx, (fraction * 100.0).round() as i32);
                    let signed = self.track_pans[idx] - 50;
                    self.status_message = format!("{} · Pan: {signed:+}", self.tracks[idx].name);
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
                _ => {}
            },
            MappedEffect::TriggerAction(action) => match action {
                MidiAction::PlayToggle => self.play_context(),
                MidiAction::Stop => self.stop_all(),
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
            "🎯 MIDI Learn ACTIVO · haz clic en una perilla/slider y mueve un CC o pulsa ' (CC21)"
                .into()
        } else {
            "MIDI Learn desactivado".into()
        };
    }
    pub(super) fn arm_midi_learn(&mut self, target: MidiBindTarget) {
        self.midi_mapper.arm_target(target);
        self.status_message = format!(
            "🎯 Esperando CC/Nota MIDI para «{}» (pulsa ' para probar con CC21)…",
            target.label()
        );
    }
    fn dispatch_routed_midi(&mut self, routed: impl IntoIterator<Item = RoutedMidi>) {
        for RoutedMidi { target, message } in routed {
            match target {
                MidiTarget::Instrument(id) if id == self.instrument.id() => match message {
                    MidiMessage::NoteOn { note, velocity } => self.piano_note_on(note, velocity),
                    MidiMessage::NoteOff { note } => self.piano_note_off(note),
                    MidiMessage::ControlChange { .. } => {}
                    MidiMessage::PitchBend { bend } => {
                        let norm = (f32::from(bend) / 8192.0).clamp(-1.0, 1.0);
                        self.set_piano_param(ParamId::PitchBend, norm);
                    }
                    MidiMessage::ProgramChange { program } => {
                        self.instrument.select_preset(usize::from(program));
                        self.track_revision = self.track_revision.wrapping_add(1);
                    }
                    MidiMessage::AllNotesOff => {
                        self.instrument.all_notes_off();
                    }
                },
                MidiTarget::Instrument(id) if self.is_sampler_id(id) => {
                    if id == self.active_sampler_id {
                        self.sampler_midi(message);
                    } else if let Some(sampler) = self.sampler_instances.get_mut(&id) {
                        match message {
                            MidiMessage::NoteOff { note } => sampler.note_off(note),
                            MidiMessage::AllNotesOff => sampler.all_off(),
                            _ => {}
                        }
                    }
                }
                MidiTarget::Instrument(id) if self.is_vst_id(id) => {
                    self.vst_midi_to(id, message);
                }
                MidiTarget::Instrument(_) => {}
                MidiTarget::Sequencer => {
                    if let MidiMessage::NoteOn { .. } = message {
                        self.apply_pattern_action(PatternAction::Toggle {
                            lane: self.cursor_row,
                            step: self.cursor_col,
                        });
                        let step_count = self.pattern_editor.step_count().max(1);
                        self.cursor_col = (self.cursor_col + 1) % step_count;
                    }
                }
            }
        }
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
        self.track_delete_pending = false;
        self.apply_midi(messages);
        self.status_message = self.controller_status();
        true
    }
    pub(super) fn toggle_controller(&mut self) {
        self.track_delete_pending = false;
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
        self.sampler.silence();
        self.piano_note_on(note, velocity);
    }
    fn piano_note_on(&mut self, note: u8, velocity: u8) {
        self.publish_track_fx();
        if note > 127 || self.instrument.held(note) {
            return;
        }
        if !self.instrument.is_running() {
            self.stop_audio();
            self.stop_pattern();
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
        self.piano_note_off(note);
    }
    fn piano_note_off(&mut self, note: u8) {
        if !self.instrument.note_off(note) {
            return;
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
            self.sampler.note_off(note);
        }
        if let Some(note) = self.instrument.take_mouse_note() {
            self.piano_note_off(note);
        }
    }
    pub(super) fn all_notes_off(&mut self) {
        self.vst.all_off();
        for vst in self.vst_instances.values_mut() {
            vst.all_off();
        }
        self.sampler.all_off();
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
        let id = self.active_instrument_id();
        assign_instrument(&mut self.tracks, target_track, id);
        self.timeline.insert_midi_clip(clip);
        self.sync_midi_focus();
        self.status_message = format!(
            "Clip MIDI insertado en {} ({note_count} notas, {duration:.1} negras) · Space: reproducir",
            self.tracks[target_track].name
        );
    }
    /// Activa o detiene la grabación explícita de la salida MIDI hacia un `MidiClip`.
    pub(super) fn toggle_midi_record(&mut self) {
        if !self.midi_capture.is_recording() && !self.track_accepts_clips(self.cursor_row) {
            self.status_message = "Selecciona una pista hija para grabar MIDI".into();
            return;
        }
        if self.midi_capture.is_recording() {
            let now = self.now_ns();
            self.midi_capture
                .stop_recording(now, self.timeline.tempo.bpm());
            if self.midi_capture.has_notes() {
                self.capture_midi_to_track();
            } else {
                self.status_message = "Grabación MIDI detenida (sin notas)".into();
            }
        } else {
            self.all_notes_off();
            self.midi_capture.start_recording();
            if !self.controller.enabled() {
                let _ = self.controller.set_enabled(true);
            }
            self.focus_midi_instrument();
            self.status_message = format!(
                "● Grabando MIDI en {} · toca y pulsa REC / :record para insertar el clip",
                self.tracks[self.cursor_row].name
            );
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
        self.vst.silence();
        for vst in self.vst_instances.values_mut() {
            vst.silence();
        }
        self.sampler.silence();
        for sampler in self.sampler_instances.values_mut() {
            sampler.silence();
        }
        self.instrument.stop();
    }
    /// Audición en loop del patrón. El plan se compila al pulsar Play.
    pub(super) fn play_pattern(&mut self) {
        self.stop_audio();
        self.stop_instrument();
        self.stop_pattern();
        self.pattern_loop_beats = Some(self.pattern_editor.length_beats());
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
        let stopped_at = self.player.as_ref().map(|player| player.beat());
        self.controller.release_all();
        self.midi_server.release_all();
        self.stop_audio();
        self.stop_pattern();
        self.stop_instrument();
        if let Some(beat) = stopped_at {
            self.timeline.set_edit_beat(beat);
        }
    }
    pub(super) fn play_audio(&mut self) {
        self.publish_track_fx();
        self.stop_pattern();
        self.stop_audio();
        let edit_beat = self.timeline.edit_beat();
        let selected_has_midi = self
            .timeline
            .midi_clip
            .as_ref()
            .is_some_and(|clip| clip.track == self.cursor_row && edit_beat < clip.end_beat());
        if (selected_has_midi
            || (self.timeline.clip.is_none()
                && self
                    .timeline
                    .midi_clip
                    .as_ref()
                    .is_some_and(|clip| edit_beat < clip.end_beat())))
            && self.timeline.midi_clip.as_ref().is_some_and(|clip| {
                self.tracks
                    .get(clip.track)
                    .and_then(|track| track.instrument)
                    .is_some_and(|id| self.is_vst_id(id))
            })
        {
            if let Some(midi_clip) = self.timeline.midi_clip.clone() {
                if !self.track_is_audible(midi_clip.track) {
                    self.status_message =
                        format!("Pista {} silenciada por Mute/Solo", midi_clip.track + 1);
                    return;
                }
                if midi_clip.props.muted {
                    self.status_message =
                        format!("Clip MIDI «{}» está silenciado (Mute)", midi_clip.name);
                    return;
                }
                self.play_vst_clip(&midi_clip, edit_beat.max(midi_clip.start_beat));
                return;
            }
        }
        self.stop_instrument();
        self.request_file_playback(edit_beat);
    }

    pub(super) fn stop_audio(&mut self) {
        self.vst.stop_clip();
        for vst in self.vst_instances.values() {
            vst.stop_clip();
        }
        self.file_playback = None;
        self.player = None;
        self.instrument.stop_clip();
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
    pub(super) fn track_is_audible(&self, track: usize) -> bool {
        effective_mix(&self.tracks, &self.track_levels, &self.track_pans, track).audible
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
    /// El plan de percusión es único: usa la media de los faders de pistas Drum.
    pub(super) fn drum_gain(&self) -> f32 {
        let drums: Vec<usize> = (0..self.tracks.len())
            .filter(|&i| self.tracks[i].kind == TrackKind::Drum)
            .collect();
        if drums.is_empty() {
            return 1.0;
        }
        drums.iter().map(|&i| self.track_gain(i)).sum::<f32>() / drums.len() as f32
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
        if let Some(player) = &self.player {
            for (index, track) in self.tracks.iter().enumerate() {
                player.set_gain(track.id, self.track_gain(index));
                player.set_pan(track.id, self.track_pan(index));
            }
        }
        if let Some(player) = &self.player {
            player.set_drum_gain(self.drum_gain());
        }
        self.instrument.set_gain(self.piano_gain());
        self.instrument.set_pan(self.piano_pan());
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
