//! Operaciones comunes a clips de audio y MIDI, independientes de su menú.
use super::{DawApp, timeline::DragKind};
use elevate::musical::clip::ClipProps;

impl DawApp {
    /// Clip activo en el punto de edición. El MIDI prevalece si audio y MIDI se solapan,
    /// igual que en el orden de dibujo y picking del Arrangement.
    pub(in crate::ui) fn clip_kind_at_cursor(&mut self) -> Option<DragKind> {
        let track = self.cursor_row;
        let beat = self.timeline.edit_beat();
        if self.timeline.clip.as_ref().is_some_and(|c| {
            Some(c.id) == self.timeline.focused_audio
                && c.track == track
                && beat >= c.start_beat
                && beat < c.end_beat(self.timeline.tempo.bpm())
        }) {
            return Some(DragKind::Audio);
        }
        if self.timeline.midi_clip.as_ref().is_some_and(|clip| {
            clip.track == track && beat >= clip.start_beat && beat < clip.end_beat()
        }) {
            Some(DragKind::Midi)
        } else if let Some(index) = self.timeline.other_midi_clips.iter().rposition(|clip| {
            clip.track == track && beat >= clip.start_beat && beat < clip.end_beat()
        }) {
            let clip = self.timeline.other_midi_clips.remove(index);
            self.timeline.insert_midi_clip(clip);
            Some(DragKind::Midi)
        } else if self.timeline.clip.as_ref().is_some_and(|clip| {
            clip.track == track
                && beat >= clip.start_beat
                && beat < clip.end_beat(self.timeline.tempo.bpm())
        }) {
            Some(DragKind::Audio)
        } else if let Some(index) = self.timeline.other_audio_clips.iter().rposition(|clip| {
            clip.track == track
                && beat >= clip.start_beat
                && beat < clip.end_beat(self.timeline.tempo.bpm())
        }) {
            let clip = self.timeline.other_audio_clips.remove(index);
            self.timeline.insert_audio_clip(clip);
            Some(DragKind::Audio)
        } else {
            None
        }
    }

    /// Despacho de las opciones de clip del catálogo. Las operaciones que necesitan
    /// parámetros de texto/color siguen disponibles en el menú contextual.
    pub(in crate::ui) fn execute_clip_catalog_action(&mut self, command: &str) {
        if command == "clip.materialize" {
            self.materialize_midi_selection();
            return;
        }
        let Some(kind) = self.clip_kind_at_cursor() else {
            self.status_message = "Coloca el cursor sobre un clip para editarlo".into();
            return;
        };
        match command {
            "clip.duplicate" => self.duplicate_active_clip(kind),
            "clip.split" => {
                let beat = self.timeline.edit_beat();
                self.split_arrangement_clip(kind, beat);
            }
            "clip.restore_trim" => {
                self.stop_audio();
                self.timeline.restore_clip_trim(kind);
            }
            "clip.mute" => self.toggle_clip_mute(kind),
            "clip.lock" => self.toggle_clip_lock(kind),
            "clip.gain.down" | "clip.gain.reset" | "clip.gain.up" => {
                if kind == DragKind::Midi {
                    self.stop_audio();
                }
                if let Some(props) = self.active_clip_props_mut(kind) {
                    props.gain_db = match command {
                        "clip.gain.down" => (props.gain_db - 3.0).clamp(-24.0, 12.0),
                        "clip.gain.up" => (props.gain_db + 3.0).clamp(-24.0, 12.0),
                        _ => 0.0,
                    };
                    self.status_message = format!("Ganancia del clip: {:+.1} dB", props.gain_db);
                }
                if kind == DragKind::Audio {
                    self.publish_clip_edit();
                }
            }
            "clip.track.previous" => self.move_active_clip_track(kind, -1),
            "clip.track.next" => self.move_active_clip_track(kind, 1),
            "clip.delete" => self.delete_active_clip(kind),
            _ => self.status_message = format!("Acción de clip desconocida: {command}"),
        }
    }

    pub(in crate::ui) fn split_arrangement_clip(&mut self, kind: DragKind, beat: f64) {
        let follow_editor = match kind {
            DragKind::Audio => self.shown_sample().is_some(),
            DragKind::Midi => self.piano_roll.open,
        };
        if !self.timeline.split_clip(kind, beat) {
            self.status_message = "No se puede dividir el clip aquí".into();
            return;
        }
        if follow_editor {
            self.show_selected_editor(kind);
        }
        self.publish_clip_edit();
        self.status_message = "Clip dividido · segunda mitad seleccionada".into();
    }

    pub(in crate::ui) fn active_clip_name(&self, kind: DragKind) -> &str {
        match kind {
            DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.name.as_str()),
            DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.name.as_str()),
        }
        .unwrap_or("Clip")
    }

    pub(in crate::ui) fn rename_active_clip(&mut self, kind: DragKind, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        match kind {
            DragKind::Midi => {
                if let Some(c) = &mut self.timeline.midi_clip {
                    c.name = name.into();
                }
            }
            DragKind::Audio => {
                if let Some(c) = &mut self.timeline.clip {
                    c.name = name.into();
                }
            }
        }
        self.status_message = format!("Clip renombrado: {name}");
    }

    pub(in crate::ui) fn toggle_clip_mute(&mut self, kind: DragKind) {
        self.stop_audio();
        if kind == DragKind::Midi
            && let Some(props) = self.active_clip_props(kind)
        {
            self.timeline
                .change_selected_midi(|clip| clip.props.muted = !props.muted);
            return;
        }
        if let Some(props) = self.active_clip_props_mut(kind) {
            props.muted = !props.muted;
        }
    }

    pub(in crate::ui) fn toggle_clip_lock(&mut self, kind: DragKind) {
        if kind == DragKind::Midi
            && let Some(props) = self.active_clip_props(kind)
        {
            self.timeline
                .change_selected_midi(|clip| clip.props.locked = !props.locked);
            return;
        }
        if let Some(props) = self.active_clip_props_mut(kind) {
            props.locked = !props.locked;
        }
    }

    pub(in crate::ui) fn toggle_clip_loop(&mut self, kind: DragKind) {
        self.stop_audio();
        match kind {
            DragKind::Midi => {
                let Some(was) = self.active_clip_props(kind).map(|p| p.looped) else {
                    return;
                };
                let next = !was;
                self.timeline.change_selected_midi(|clip| {
                    clip.props.looped = next;
                    if !next {
                        let cycle = clip.cycle_beats();
                        if clip.trim_len.is_some_and(|l| l > cycle) {
                            clip.trim_len = None;
                        }
                    }
                });
                self.status_message = if next {
                    "⟳ Bucle interno MIDI ON · arrastra el borde derecho para repetir".into()
                } else {
                    "Bucle interno MIDI OFF".into()
                };
            }
            DragKind::Audio => {
                if let Some(clip) = self.timeline.clip.as_mut() {
                    clip.props.looped = !clip.props.looped;
                    let next = clip.props.looped;
                    if !next {
                        let cycle = clip.cycle_seconds();
                        if clip.trim_len.is_some_and(|l| l > cycle) {
                            clip.trim_len = None;
                        }
                    }
                    self.status_message = if next {
                        "⟳ Bucle interno Audio ON · arrastra el borde derecho para repetir".into()
                    } else {
                        "Bucle interno Audio OFF".into()
                    };
                }
            }
        }
        self.publish_clip_edit();
    }

    /// Consolida (`Ctrl+J`) los clips seleccionados (o el clip activo recortado/en bucle)
    /// en un único clip limpio de Audio o MIDI.
    pub(in crate::ui) fn consolidate_selection(&mut self, cx: &mut gpui::Context<Self>) {
        let prefer_audio = self.timeline.focused_audio.is_some()
            || (!self.timeline.selected_audio.is_empty() && self.timeline.selected_midi.is_empty());
        if prefer_audio {
            if self.consolidate_audio_clips(cx) {
                return;
            }
            if self.consolidate_midi_clips() {
                return;
            }
        } else {
            if self.consolidate_midi_clips() {
                return;
            }
            if self.consolidate_audio_clips(cx) {
                return;
            }
        }
        self.status_message =
            "Selecciona uno o más clips de Audio o MIDI para consolidar (Ctrl+J)".into();
    }

    fn consolidate_midi_clips(&mut self) -> bool {
        use elevate::musical::{
            clip::MidiClip,
            model::Pattern,
            time::{PPQ, Tick},
        };
        let target_ids = self.timeline.midi_edit_targets();
        if target_ids.is_empty() {
            return false;
        }
        let anchor_track = self
            .timeline
            .midi_clip
            .as_ref()
            .filter(|c| target_ids.contains(&c.id))
            .map(|c| c.track)
            .or_else(|| {
                self.timeline
                    .midi_clips()
                    .find(|c| target_ids.contains(&c.id))
                    .map(|c| c.track)
            });
        let Some(track) = anchor_track else {
            return false;
        };
        let clips: Vec<MidiClip> = self
            .timeline
            .midi_clips()
            .filter(|c| c.track == track && target_ids.contains(&c.id) && !c.props.muted)
            .cloned()
            .collect();
        if clips.is_empty() {
            return false;
        }
        let min_start = clips
            .iter()
            .map(|c| c.start_beat)
            .fold(f64::INFINITY, f64::min);
        let max_end = clips.iter().map(|c| c.end_beat()).fold(0.0_f64, f64::max);
        let total_ticks = ((max_end - min_start) * PPQ as f64).round().max(PPQ as f64) as i64;
        let Ok(mut merged) = Pattern::new(Tick(total_ticks)) else {
            return false;
        };
        let mut note_count = 0usize;
        for clip in &clips {
            let offset_ticks = ((clip.start_beat - min_start) * PPQ as f64).round() as i64;
            let baked = clip.playback_pattern();
            for note in baked.notes() {
                let start = offset_ticks + note.start.0;
                let max_dur = total_ticks - start;
                if start >= 0 && max_dur > 0 {
                    let dur = note.duration.0.min(max_dur).max(1);
                    if merged
                        .add_note_copy(note, Tick(start), Tick(dur), note.instrument)
                        .is_ok()
                    {
                        note_count += 1;
                    }
                }
            }
        }
        let base_name = clips[0].name.trim_end_matches(" (Cons.)").to_owned();
        let mut consolidated =
            MidiClip::new(format!("{base_name} (Cons.)"), track, min_start, merged);
        consolidated.channel = clips[0].channel;
        consolidated.hilo_target = clips[0].hilo_target;
        consolidated.props.color_index = clips[0].props.color_index;
        let new_id = consolidated.id;
        let remove_ids: Vec<_> = clips.iter().map(|c| c.id).collect();

        self.stop_audio();
        if self
            .timeline
            .midi_clip
            .as_ref()
            .is_some_and(|c| remove_ids.contains(&c.id))
        {
            self.timeline.midi_clip = None;
        }
        self.timeline
            .other_midi_clips
            .retain(|c| !remove_ids.contains(&c.id));
        self.timeline.insert_midi_clip(consolidated);
        self.timeline.selected_midi = vec![new_id];
        if self.piano_roll.open {
            self.open_piano_roll();
        }
        self.publish_clip_edit();
        self.refresh_project_dirty();
        self.status_message = format!(
            "Consolidado MIDI: {} clip(s) → «{} (Cons.)» ({note_count} notas)",
            clips.len(),
            base_name
        );
        true
    }

    fn consolidate_audio_clips(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        use crate::audio::{AudioClip, AudioFile};
        use elevate::musical::clip::ClipId;
        use gpui::AppContext;
        use std::sync::Arc;

        let mut selected_ids = self.timeline.selected_audio.clone();
        if selected_ids.is_empty()
            && let Some(active) = &self.timeline.clip
        {
            selected_ids.push(active.id);
        }
        if selected_ids.is_empty() {
            return false;
        }
        let anchor_track = self
            .timeline
            .clip
            .as_ref()
            .filter(|c| selected_ids.contains(&c.id))
            .map(|c| c.track)
            .or_else(|| {
                self.timeline
                    .other_audio_clips
                    .iter()
                    .find(|c| selected_ids.contains(&c.id))
                    .map(|c| c.track)
            });
        let Some(track) = anchor_track else {
            return false;
        };
        let clips: Vec<AudioClip> = self
            .timeline
            .clip
            .iter()
            .chain(&self.timeline.other_audio_clips)
            .filter(|c| c.track == track && selected_ids.contains(&c.id) && !c.props.muted)
            .cloned()
            .collect();
        if clips.is_empty() {
            return false;
        }
        let bpm = self.timeline.tempo.bpm();
        let bps = bpm / 60.0;
        let rate = clips[0].file.sample_rate;
        let min_start = clips
            .iter()
            .map(|c| c.start_beat)
            .fold(f64::INFINITY, f64::min);
        let max_end = clips
            .iter()
            .map(|c| c.end_beat(bpm))
            .fold(0.0_f64, f64::max);
        let total_secs = (max_end - min_start) / bps;
        let total_frames = (total_secs * f64::from(rate)).round() as usize;
        if total_frames == 0 {
            return false;
        }

        let base_name = clips[0].name.trim_end_matches(" (Cons.)").to_owned();
        let color_index = clips[0].props.color_index;
        let remove_ids: Vec<ClipId> = clips.iter().map(|c| c.id).collect();
        let clip_count = clips.len();
        let safe_name: String = base_name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .take(36)
            .collect();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut dir = self.settings.export.saved.directory.clone();
        if let Ok(rest) = dir.strip_prefix("~")
            && let Some(home) = dirs::home_dir()
        {
            dir = home.join(rest);
        }
        dir.push("consolidated");
        let out_path = dir.join(format!("{safe_name}_cons_{stamp}.wav"));

        self.stop_audio();
        self.status_message = format!("Consolidando {clip_count} clip(s) de audio…");
        let work = cx.background_spawn(async move {
            let mut mix = vec![[0.0_f32; 2]; total_frames];
            for clip in &clips {
                let resampled;
                let src_frames: &[[f32; 2]] = if clip.file.sample_rate == rate {
                    &clip.file.frames
                } else {
                    resampled = crate::audio::export::resample(
                        &clip.file.frames,
                        clip.file.sample_rate,
                        rate,
                    )?;
                    &resampled
                };
                let dst_start = (((clip.start_beat - min_start) / bps) * f64::from(rate))
                    .round()
                    .max(0.0) as usize;
                let src_offset = (clip.trim_start * f64::from(rate)).round().max(0.0) as usize;
                let clip_frames =
                    (clip.length_seconds() * f64::from(rate)).round().max(0.0) as usize;
                let cycle_len = src_frames.len().saturating_sub(src_offset);
                if cycle_len == 0 || clip_frames == 0 {
                    continue;
                }
                let gain = clip.props.gain_linear();
                for i in 0..clip_frames {
                    let dst = dst_start + i;
                    if dst >= total_frames {
                        break;
                    }
                    let local = if clip.props.looped {
                        i % cycle_len
                    } else if i < cycle_len {
                        i
                    } else {
                        break;
                    };
                    let s = src_frames[src_offset + local];
                    mix[dst][0] += s[0] * gain;
                    mix[dst][1] += s[1] * gain;
                }
            }
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            write_wav_f32(&out_path, rate, &mix)?;
            AudioFile::decode(&out_path)
        });

        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(file) => {
                        if this
                            .timeline
                            .clip
                            .as_ref()
                            .is_some_and(|c| remove_ids.contains(&c.id))
                        {
                            this.timeline.clip = None;
                        }
                        this.timeline
                            .other_audio_clips
                            .retain(|c| !remove_ids.contains(&c.id));
                        let new_id = ClipId::fresh();
                        let mut props = ClipProps::default();
                        props.color_index = color_index;
                        let consolidated = AudioClip {
                            processing: Default::default(),
                            id: new_id,
                            name: format!("{base_name} (Cons.)"),
                            file: Arc::new(file),
                            source_path_override: None,
                            track: track.min(this.tracks.len().saturating_sub(1)),
                            start_beat: min_start,
                            props,
                            trim_start: 0.0,
                            trim_len: None,
                        };
                        this.timeline.insert_audio_clip(consolidated);
                        this.timeline.selected_audio = vec![new_id];
                        this.timeline.focused_audio = Some(new_id);
                        this.publish_clip_edit();
                        this.refresh_project_dirty();
                        this.status_message = format!(
                            "Consolidado Audio: {clip_count} clip(s) → «{base_name} (Cons.)» (WAV 32-bit float)"
                        );
                    }
                    Err(error) => {
                        this.status_message = format!("Consolidar audio: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        true
    }

    pub(in crate::ui) fn active_clip_props(&self, kind: DragKind) -> Option<ClipProps> {
        match kind {
            DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.props),
            DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.props),
        }
    }

    pub(in crate::ui) fn active_clip_props_mut(
        &mut self,
        kind: DragKind,
    ) -> Option<&mut ClipProps> {
        match kind {
            DragKind::Midi => self.timeline.midi_clip.as_mut().map(|c| &mut c.props),
            DragKind::Audio => self.timeline.clip.as_mut().map(|c| &mut c.props),
        }
    }

    pub(in crate::ui) fn duplicate_active_clip(&mut self, kind: DragKind) {
        match kind {
            DragKind::Midi => {
                let ids = self.timeline.midi_edit_targets();
                self.duplicate_arrangement_midi(&ids);
            }
            DragKind::Audio => {
                if let Some(mut copy) = self.timeline.clip.as_ref().map(|clip| clip.duplicate()) {
                    copy.start_beat = copy.end_beat(self.timeline.tempo.bpm());
                    self.status_message = format!(
                        "Clip de audio «{}» duplicado en {:.2} beats",
                        copy.name, copy.start_beat
                    );
                    self.timeline.insert_audio_clip(copy);
                }
            }
        }
    }

    pub(in crate::ui) fn delete_active_clip(&mut self, kind: DragKind) {
        if self.active_clip_props(kind).is_some_and(|p| p.locked) {
            self.status_message = "Clip bloqueado".into();
            return;
        }
        match kind {
            DragKind::Midi => {
                let ids = self.timeline.midi_edit_targets();
                self.remove_arrangement_midi(&ids);
            }
            DragKind::Audio => {
                self.timeline.clip = self.timeline.other_audio_clips.pop();
                self.timeline.focused_audio = None;
                self.publish_clip_edit();
                self.status_message = "Clip de audio eliminado".into();
            }
        }
    }

    pub(in crate::ui) fn move_active_clip_track(&mut self, kind: DragKind, delta: isize) {
        if self.active_clip_props(kind).is_some_and(|p| p.locked) {
            return;
        }
        let current = match kind {
            DragKind::Midi => self.timeline.midi_clip.as_ref().map(|c| c.track),
            DragKind::Audio => self.timeline.clip.as_ref().map(|c| c.track),
        };
        let Some(current) = current else { return };
        let destination = if delta < 0 {
            (0..current).rev().find(|i| self.track_accepts_clips(*i))
        } else {
            (current + 1..self.tracks.len()).find(|i| self.track_accepts_clips(*i))
        };
        let Some(destination) = destination else {
            return;
        };
        match kind {
            DragKind::Midi => {
                if let Err(error) = self
                    .timeline
                    .shift_midi_selection(destination as isize - current as isize, &self.tracks)
                {
                    self.status_message = error.into();
                } else {
                    self.cursor_row = destination;
                }
            }
            DragKind::Audio => {
                if let Some(clip) = self.timeline.clip.as_mut() {
                    clip.track = destination;
                    self.cursor_row = clip.track;
                }
            }
        }
        self.publish_clip_edit();
    }

    pub(in crate::ui) fn route_active_midi_clip(
        &mut self,
        target: Option<elevate::musical::track::TrackId>,
    ) {
        let Some(clip) = &self.timeline.midi_clip else {
            return;
        };
        if clip.drum_target.is_some() {
            self.status_message = "El clip de batería conserva su pieza de Drums Latex".into();
            return;
        }
        let destination = match target {
            Some(id) => {
                let Some(group) = self
                    .tracks
                    .iter()
                    .find(|track| track.id == id && track.hilo.is_some())
                else {
                    self.status_message = "El Hilo elegido ya no existe".into();
                    return;
                };
                format!("Hilo {}", group.name)
            }
            None => "carril".into(),
        };
        self.stop_audio();
        if let Some(clip) = &mut self.timeline.midi_clip {
            clip.hilo_target = target;
            if self
                .piano_roll
                .opened_clip
                .as_ref()
                .is_some_and(|opened| opened.id == clip.id)
            {
                self.piano_roll.opened_clip = Some(clip.clone());
            }
        }
        self.status_message =
            format!("Salida MIDI: {destination} · el clip permanece en su carril");
    }
}

impl DawApp {
    pub(super) fn handle_arrangement_edit_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        use crate::ui::workspace_focus::Panel;
        if self.command_open
            || !self.arrangement_visible()
            || !matches!(
                self.workspace_focus.active,
                Panel::Arrangement | Panel::Center
            )
            || event.is_held
        {
            return false;
        }
        let mods = event.keystroke.modifiers;
        let key = event.keystroke.key.to_ascii_lowercase();
        if mods.alt {
            return false;
        }
        if mods.control || mods.platform {
            match key.as_str() {
                "a" if mods.shift => {
                    self.timeline.selected_midi.clear();
                    self.timeline.selected_audio.clear();
                    self.timeline.time_selection = None;
                    self.status_message = "Selección limpiada".into();
                }
                "a" => {
                    let row_visible = |track: usize| {
                        self.timeline
                            .layout
                            .as_ref()
                            .is_none_or(|layout| layout.row(track).is_some())
                    };
                    self.timeline.selected_midi = self
                        .timeline
                        .midi_clips()
                        .filter(|clip| row_visible(clip.track))
                        .map(|clip| clip.id)
                        .collect();
                    self.timeline.selected_audio = self
                        .timeline
                        .clip
                        .iter()
                        .chain(&self.timeline.other_audio_clips)
                        .filter(|clip| row_visible(clip.track))
                        .map(|clip| clip.id)
                        .collect();
                    let total =
                        self.timeline.selected_midi.len() + self.timeline.selected_audio.len();
                    self.status_message = format!("{total} clips seleccionados");
                }
                "l" if !mods.shift => self.toggle_loop_from_selection(),
                "j" if !mods.shift => self.consolidate_selection(cx),
                "b" if !mods.shift => self.bounce_midi_to_audio(),
                "d" if mods.shift => self.duplicate_arrangement_time(),
                "delete" | "backspace" if mods.shift => self.delete_arrangement_time(),
                "i" => self.insert_arrangement_time(),
                "m" if mods.shift => {
                    let beat = self
                        .timeline
                        .active_region_ticks()
                        .map(|(s, _)| s as f64 / elevate::musical::time::PPQ as f64)
                        .unwrap_or_else(|| self.timeline.edit_beat());
                    self.edit_arrangement_marker(beat, window, cx);
                }
                "c" if !mods.shift => self.copy_arrangement_midi(false),
                "x" if !mods.shift => self.copy_arrangement_midi(true),
                "v" if !mods.shift => self.paste_arrangement_midi(),
                "d" if !mods.shift => {
                    let ids = self.timeline.selected_midi.clone();
                    self.duplicate_arrangement_midi(&ids);
                }
                "z" => self.arrangement_midi_undo(mods.shift),
                "y" if !mods.shift => self.arrangement_midi_undo(true),
                _ => return false,
            }
            return true;
        }
        match key.as_str() {
            "delete" | "backspace" if !self.timeline.selected_midi.is_empty() => {
                let ids = self.timeline.selected_midi.clone();
                self.remove_arrangement_midi(&ids);
            }
            "delete" | "backspace"
                if self.timeline.clip.as_ref().is_some_and(|c| {
                    Some(c.id) == self.timeline.focused_audio && self.cursor_row == c.track
                }) =>
            {
                self.delete_active_clip(DragKind::Audio)
            }
            "escape" if self.timeline.cancel_midi_gesture() => {}
            "escape" if self.timeline.time_selection.is_some() || self.timeline.region_tool => {
                self.timeline.time_selection = None;
                self.timeline.region_tool = false;
                self.status_message = "Selección normal".into();
            }
            "escape" if !self.timeline.selected_midi.is_empty() => {
                self.timeline.selected_midi.clear()
            }
            _ => return false,
        }
        true
    }

    pub(in crate::ui) fn arrangement_midi_undo(&mut self, redo: bool) {
        self.timeline.end_clip_drag();
        self.stop_audio();
        match self.timeline.undo_midi_edit(redo) {
            Ok(()) => {
                self.reconcile_arrangement_midi_focus();
                self.status_message = if redo {
                    "Edición MIDI rehecha"
                } else {
                    "Edición MIDI deshecha"
                }
                .into();
                self.piano_roll.hint = self.status_message.clone();
            }
            Err(error) => self.status_message = error.into(),
        }
    }

    fn reconcile_arrangement_midi_focus(&mut self) {
        if let Some(clip) = &self.timeline.midi_clip {
            self.cursor_row = clip.track;
            self.piano_roll
                .selected_notes
                .retain(|id| clip.notes().iter().any(|note| note.id == *id));
            if self
                .piano_roll
                .selected
                .is_some_and(|id| !clip.notes().iter().any(|note| note.id == id))
            {
                self.piano_roll.selected = self.piano_roll.selected_notes.first().copied();
            }
        } else {
            self.piano_roll.open = false;
        }
    }

    fn remove_arrangement_midi(&mut self, ids: &[elevate::musical::clip::ClipId]) -> bool {
        match self.timeline.delete_selected_midi(ids) {
            Ok(count) if count > 0 => {
                self.reconcile_arrangement_midi_focus();
                self.publish_clip_edit();
                self.status_message = format!("{count} clips MIDI eliminados");
                true
            }
            Ok(_) => false,
            Err(error) => {
                self.status_message = error.into();
                false
            }
        }
    }

    fn duplicate_arrangement_midi(&mut self, ids: &[elevate::musical::clip::ClipId]) {
        self.stop_audio();
        let count = self.timeline.duplicate_midi_selection(ids);
        if count > 0 {
            self.reconcile_arrangement_midi_focus();
            self.status_message = format!("{count} clips MIDI duplicados");
        }
    }

    pub(in crate::ui) fn copy_arrangement_midi(&mut self, cut: bool) {
        let ids = self.timeline.selected_midi.clone();
        let clipboard: Vec<_> = self
            .timeline
            .midi_clips()
            .filter(|clip| ids.contains(&clip.id))
            .filter_map(|clip| {
                self.tracks
                    .get(clip.track)
                    .map(|track| (track.id, clip.clone()))
            })
            .collect();
        if clipboard.is_empty() || (cut && !self.remove_arrangement_midi(&ids)) {
            return;
        }
        let count = clipboard.len();
        self.timeline.midi_clipboard = clipboard;
        self.status_message = format!(
            "{count} clips MIDI {}",
            if cut { "cortados" } else { "copiados" }
        );
    }

    fn paste_arrangement_midi(&mut self) {
        let Some(first) = self
            .timeline
            .midi_clipboard
            .iter()
            .map(|(_, clip)| clip.start_beat)
            .reduce(f64::min)
        else {
            return;
        };
        let beat = self.timeline.edit_beat();
        let copies = self
            .timeline
            .midi_clipboard
            .iter()
            .map(|(track, clip)| {
                let mut copy = clip.duplicate();
                copy.track = self
                    .tracks
                    .iter()
                    .position(|entry| entry.id == *track && entry.accepts_clips())?;
                copy.start_beat = beat + (clip.start_beat - first);
                Some(copy)
            })
            .collect::<Option<Vec<_>>>();
        match copies {
            Some(copies) => {
                self.stop_audio();
                let count = self.timeline.insert_midi_batch(copies);
                self.reconcile_arrangement_midi_focus();
                self.status_message = format!("{count} clips MIDI pegados");
            }
            None => self.status_message = "Falta una pista de los clips copiados".into(),
        }
    }
}

fn write_wav_f32(
    path: &std::path::Path,
    sample_rate: u32,
    frames: &[[f32; 2]],
) -> Result<(), String> {
    use std::io::Write;
    let channels: u16 = 2;
    let bits_per_sample: u16 = 32;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let data_bytes = u32::try_from(frames.len() * usize::from(block_align))
        .map_err(|_| "Audio consolidado demasiado largo para WAV".to_string())?;
    let riff_size = 36u32
        .checked_add(data_bytes)
        .ok_or_else(|| "Audio consolidado demasiado largo para WAV".to_string())?;

    let mut file = std::fs::File::create(path)
        .map_err(|error| format!("No se pudo crear {}: {error}", path.display()))?;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&riff_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());
    file.write_all(&header)
        .map_err(|error| format!("Error escribiendo cabecera WAV: {error}"))?;

    let mut chunk = Vec::with_capacity(8192);
    for &[left, right] in frames {
        let l = if left.is_finite() { left } else { 0.0 };
        let r = if right.is_finite() { right } else { 0.0 };
        chunk.extend_from_slice(&l.to_le_bytes());
        chunk.extend_from_slice(&r.to_le_bytes());
        if chunk.len() >= 8192 {
            file.write_all(&chunk)
                .map_err(|error| format!("Error escribiendo audio WAV: {error}"))?;
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        file.write_all(&chunk)
            .map_err(|error| format!("Error escribiendo audio WAV: {error}"))?;
    }
    Ok(())
}
