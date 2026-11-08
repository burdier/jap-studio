//! Coordinación UI → adaptadores de audio; decodificación en background.
use super::{CenterView, DawApp, pattern_editor::PatternAction};
use crate::audio::{AudioClip, AudioFile, BEATS_PER_SECOND, DrumPlayer, Player};
use crate::instruments::piano_params::{self, Applies, ParamId, SPECS};
use elevate::musical::{
    controller::DEFAULT_VELOCITY,
    midi::{MidiMessage, MidiTarget, RoutedMidi},
    midi_map::{MappedEffect, MidiAction, MidiBindTarget},
    pitch::note_name,
    smf::parse_smf_to_clip,
    time::DEFAULT_BPM,
    track::assign_instrument,
};
use gpui::{AppContext, Context};
use std::{path::PathBuf, sync::Arc};

impl DawApp {
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
        let target = match self.center_view {
            CenterView::Instrument => Some(MidiTarget::Instrument(self.instrument.id())),
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
    pub(super) fn focus_midi_instrument(&mut self) {
        let released = self
            .midi_server
            .focus(Some(MidiTarget::Instrument(self.instrument.id())));
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
                    self.track_levels[idx] = (fraction * 100.0).round() as i32;
                    if self.timeline.clip.as_ref().is_some_and(|c| c.track == idx) {
                        let target_gain = (fraction * 2.0).clamp(0.0, 2.0);
                        self.change_gain(target_gain - self.gain);
                    } else {
                        self.status_message =
                            format!("{}: {}%", self.tracks[idx].name, self.track_levels[idx]);
                    }
                }
                MidiBindTarget::TrackPan(idx) if idx < self.track_pans.len() => {
                    self.track_pans[idx] = (fraction * 100.0).round() as i32;
                    let signed = self.track_pans[idx] - 50;
                    self.status_message = format!("{} · Pan: {signed:+}", self.tracks[idx].name);
                }
                MidiBindTarget::MasterGain => {
                    let target_gain = (fraction * 2.0).clamp(0.0, 2.0);
                    self.change_gain(target_gain - self.gain);
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
                    MidiMessage::NoteOn { note, velocity } => self.note_on_velocity(note, velocity),
                    MidiMessage::NoteOff { note } => self.note_off(note),
                    MidiMessage::ControlChange { .. } => {}
                    MidiMessage::PitchBend { bend } => {
                        let norm = (f32::from(bend) / 8192.0).clamp(-1.0, 1.0);
                        self.set_piano_param(ParamId::PitchBend, norm);
                    }
                    MidiMessage::ProgramChange { program } => {
                        self.instrument.select_preset(usize::from(program));
                    }
                    MidiMessage::AllNotesOff => {
                        self.instrument.all_notes_off();
                    }
                },
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
        if note > 127 || self.instrument.held(note) {
            return;
        }
        if !self.instrument.is_running() {
            self.stop_audio();
            self.stop_pattern();
        }
        let now = self.now_ns();
        self.midi_capture
            .record(MidiMessage::NoteOn { note, velocity }, now, DEFAULT_BPM);
        match self
            .instrument
            .note_on(note, velocity, self.gain, self.bypass)
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
        if !self.instrument.note_off(note) {
            return;
        }
        let now = self.now_ns();
        self.midi_capture
            .record(MidiMessage::NoteOff { note }, now, DEFAULT_BPM);
        self.status_message = format!(
            "Note Off {} · {}",
            note_name(note),
            self.instrument.active_name()
        );
    }
    pub(super) fn release_mouse_note(&mut self) {
        if let Some(note) = self.instrument.take_mouse_note() {
            self.note_off(note);
        }
    }
    pub(super) fn all_notes_off(&mut self) {
        let controller_off = self.controller.release_all();
        self.apply_midi(controller_off);
        let server_off = self.midi_server.release_all();
        self.dispatch_routed_midi(server_off);
        let now = self.now_ns();
        self.midi_capture
            .record(MidiMessage::AllNotesOff, now, DEFAULT_BPM);
        if self.instrument.all_notes_off() {
            self.status_message = "Todas las notas liberadas".into();
        }
    }
    /// Recupera el MIDI tocado en el búfer del controlador y lo inserta como `MidiClip`
    /// en la pista seleccionada (o en la pista asignada al instrumento).
    pub(super) fn capture_midi_to_track(&mut self) {
        let now = self.now_ns();
        let target_track = self.cursor_row.min(self.tracks.len() - 1);
        let start_beat = self.timeline.edit_beat();
        let title = format!("MIDI · {}", self.instrument.active_name());
        let Some(clip) = self.midi_capture.to_clip(
            title,
            target_track,
            start_beat,
            self.instrument.id(),
            now,
            DEFAULT_BPM,
        ) else {
            self.status_message =
                "No hay notas MIDI recientes para insertar · toca algo primero".into();
            return;
        };
        let note_count = clip.notes().len();
        let duration = clip.duration_beats();
        assign_instrument(&mut self.tracks, target_track, self.instrument.id());
        self.timeline.insert_midi_clip(clip);
        self.sync_midi_focus();
        self.status_message = format!(
            "Clip MIDI insertado en {} ({note_count} notas, {duration:.1} negras) · Space: reproducir",
            self.tracks[target_track].name
        );
    }
    /// Activa o detiene la grabación explícita de la salida MIDI hacia un `MidiClip`.
    pub(super) fn toggle_midi_record(&mut self) {
        if self.midi_capture.is_recording() {
            let now = self.now_ns();
            self.midi_capture.stop_recording(now, DEFAULT_BPM);
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
    pub(super) fn select_soundfont_bank(&mut self, index: usize) {
        self.all_notes_off();
        match self.instrument.select_bank(index) {
            Ok(()) => {
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
        self.instrument.stop();
    }
    /// Audición en loop del patrón. El plan se compila al pulsar Play.
    pub(super) fn play_pattern(&mut self) {
        self.stop_audio();
        self.stop_instrument();
        self.stop_pattern();
        match DrumPlayer::start(
            &self.pattern_editor.pattern,
            DEFAULT_BPM,
            self.gain,
            self.bypass,
        ) {
            Ok(player) => {
                let events = player.event_count();
                let rate = player.sample_rate;
                let loop_beats = player.loop_beats();
                self.pattern_player = Some(player);
                self.status_message = format!(
                    "Patrón en loop ({loop_beats:.2} negras) · {events} eventos · {rate} Hz · percusión sintetizada"
                );
            }
            Err(error) => self.status_message = format!("Salida de percusión: {error}"),
        }
    }
    pub(super) fn stop_pattern(&mut self) {
        self.pattern_player = None;
    }
    pub(super) fn play_context(&mut self) {
        if self.center_view == CenterView::Session {
            self.play_pattern();
        } else {
            self.play_audio();
        }
    }
    pub(super) fn stop_all(&mut self) {
        self.stop_audio();
        self.stop_pattern();
        self.stop_instrument();
    }
    pub(super) fn import_audio(
        &mut self,
        path: PathBuf,
        track: usize,
        start_beat: f64,
        cx: &mut Context<Self>,
    ) {
        self.stop_audio();
        self.stop_pattern();
        let is_midi = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mid") || ext.eq_ignore_ascii_case("midi"));
        if is_midi {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("MIDI")
                        .to_string();
                    match parse_smf_to_clip(&bytes, name, track, start_beat, self.instrument.id()) {
                        Ok(clip) => {
                            let notes = clip.notes().len();
                            let beats = clip.duration_beats();
                            assign_instrument(&mut self.tracks, track, self.instrument.id());
                            self.timeline.insert_midi_clip(clip);
                            self.cursor_row = track;
                            self.timeline.set_edit_beat(start_beat);
                            self.sync_midi_focus();
                            self.status_message = format!(
                                "Archivo MIDI importado en {} ({notes} notas, {beats:.1} negras) · Space: reproducir",
                                self.tracks[track].name
                            );
                        }
                        Err(err) => self.status_message = format!("Importación MIDI: {err}"),
                    }
                }
                Err(err) => {
                    self.status_message = format!("No se pudo leer {}: {err}", path.display())
                }
            }
            cx.notify();
            return;
        }
        self.status_message = format!("Decodificando {}…", path.display());
        let decode = cx.background_spawn(async move { AudioFile::decode(&path) });
        self.import_task = Some(cx.spawn(async move |this, cx| {
            let result = decode.await;
            if let Err(error) = this.update(cx, |this, cx| {
                match result {
                    Ok(file) => {
                        this.stop_audio();
                        this.status_message = format!(
                            "{} · {:.2}s · {} Hz · Space: play · [/]: Volume",
                            file.name,
                            file.duration(),
                            file.sample_rate
                        );
                        this.timeline.clip = Some(AudioClip {
                            file: Arc::new(file),
                            track,
                            start_beat,
                        });
                        this.cursor_row = track;
                        this.timeline.set_edit_beat(start_beat);
                    }
                    Err(error) => this.status_message = format!("No se pudo importar: {error}"),
                }
                cx.notify();
            }) {
                eprintln!("Importación finalizada tras cerrar la vista: {error}");
            }
        }));
        cx.notify();
    }
    pub(super) fn play_audio(&mut self) {
        self.stop_pattern();
        self.stop_audio();
        let edit_beat = self.timeline.edit_beat();
        let selected_has_midi = self
            .timeline
            .midi_clip
            .as_ref()
            .is_some_and(|clip| clip.track == self.cursor_row && edit_beat < clip.end_beat());
        if selected_has_midi
            || (self.timeline.clip.is_none()
                && self
                    .timeline
                    .midi_clip
                    .as_ref()
                    .is_some_and(|clip| edit_beat < clip.end_beat()))
        {
            if let Some(midi_clip) = self.timeline.midi_clip.clone() {
                let start = edit_beat.max(midi_clip.start_beat);
                match self.instrument.play_clip(
                    &midi_clip,
                    start,
                    DEFAULT_BPM,
                    self.gain,
                    self.bypass,
                ) {
                    Ok(()) => {
                        self.timeline.playhead = Some(start);
                        self.status_message = format!(
                            "▶ Reproduciendo clip MIDI «{}» ({} notas) → {}",
                            midi_clip.name,
                            midi_clip.notes().len(),
                            self.instrument.active_name()
                        );
                    }
                    Err(error) => self.status_message = format!("Clip MIDI: {error}"),
                }
                return;
            }
        }
        self.stop_instrument();
        let Some(clip) = self.timeline.clip.as_ref() else {
            self.status_message =
                "Arrastra un archivo de audio o graba/recupera un clip MIDI".into();
            return;
        };
        if edit_beat >= clip.start_beat + clip.file.duration() * BEATS_PER_SECOND {
            self.status_message = "Coloca el cursor antes del final del clip".into();
            return;
        }
        match Player::start(clip, edit_beat, self.gain, self.bypass) {
            Ok(player) => {
                self.player = Some(player);
                self.status_message = "Reproduciendo · archivo → Volume → CPAL".into();
            }
            Err(error) => self.status_message = format!("Salida de audio: {error}"),
        }
    }
    pub(super) fn stop_audio(&mut self) {
        self.player = None;
        self.instrument.stop_clip();
        self.timeline.playhead = None;
    }
    pub(super) fn change_gain(&mut self, delta: f32) {
        self.gain = (self.gain + delta).clamp(0.0, 2.0);
        if let Some(player) = &self.player {
            player.set_gain(self.gain);
        }
        if let Some(player) = &self.pattern_player {
            player.set_gain(self.gain);
        }
        self.instrument.set_gain(self.gain);
        self.status_message = format!("Volume: {:.2}x", self.gain);
    }
    pub(super) fn toggle_bypass(&mut self) {
        self.bypass = !self.bypass;
        if let Some(player) = &self.player {
            player.set_bypass(self.bypass);
        }
        if let Some(player) = &self.pattern_player {
            player.set_bypass(self.bypass);
        }
        self.instrument.set_bypass(self.bypass);
    }
}
