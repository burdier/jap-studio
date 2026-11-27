//! Grabación de audio real en pistas del Arrangement: coordina `AudioRecorder`,
//! pre-cuenta con el metrónomo, transporte e inserción del clip WAV resultante.
use super::DawApp;
use crate::audio::{
    AudioClip, AudioFile,
    input_monitor::{InputMonitor, MonitorMode},
    recorder::AudioRecorder,
};
use elevate::musical::track::{TrackId, TrackKind, effective_mix};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) struct ActiveInputMonitor {
    pub monitor: InputMonitor,
    pub track_id: TrackId,
    pub mode: MonitorMode,
}

pub(super) struct ActiveAudioRecording {
    pub recorder: AudioRecorder,
    pub track_index: usize,
    pub track_id: TrackId,
    pub track_name: String,
    pub start_beat: f64,
    pub waiting_count_in: bool,
}

impl DawApp {
    pub(in crate::ui) fn is_audio_recording(&self) -> bool {
        self.audio_recorder.is_some()
    }

    pub(in crate::ui) fn is_any_recording(&self) -> bool {
        self.midi_capture.is_recording() || self.audio_recorder.is_some()
    }

    pub(in crate::ui) fn toggle_audio_record_stereo(&mut self) {
        self.audio_record_stereo = !self.audio_record_stereo;
        if let Some(mon) = &self.input_monitor {
            mon.monitor.set_stereo(self.audio_record_stereo);
        }
        self.status_message = format!(
            "Entrada de audio: modo {}",
            if self.audio_record_stereo {
                "Estéreo (canales L+R)"
            } else {
                "Mono centrado (canal de entrada)"
            }
        );
    }

    pub(in crate::ui) fn track_monitor_mode(&self, track_id: TrackId) -> MonitorMode {
        self.input_monitor
            .as_ref()
            .filter(|m| m.track_id == track_id)
            .map_or(MonitorMode::Off, |m| m.mode)
    }

    pub(in crate::ui) fn active_monitor_mode(&self) -> MonitorMode {
        self.input_monitor
            .as_ref()
            .map_or(MonitorMode::Off, |m| m.mode)
    }

    pub(in crate::ui) fn input_monitor_peak(&self) -> f32 {
        self.input_monitor
            .as_ref()
            .map_or(0.0, |m| m.monitor.input_peak().max(m.monitor.output_peak()))
    }

    pub(in crate::ui) fn input_monitor_latency(
        &self,
    ) -> Option<crate::audio::input_monitor::MonitorLatency> {
        self.input_monitor.as_ref().map(|m| m.monitor.latency())
    }

    pub(in crate::ui) fn estimated_monitor_latency(
        &self,
    ) -> crate::audio::input_monitor::MonitorLatency {
        if let Some(live) = self.input_monitor_latency() {
            return live;
        }
        let rate = self.settings.audio.saved.sample_rate.unwrap_or(48_000);
        let buf = self.settings.audio.saved.buffer_frames.unwrap_or(256);
        let fx_pdc = self
            .effect_rack(self.cursor_row)
            .map_or(0, |r| r.latency_samples().min(u64::from(u32::MAX)) as u32);
        crate::audio::input_monitor::MonitorLatency {
            sample_rate: rate,
            io_buffer_frames: buf.saturating_mul(2),
            ring_frames: buf,
            fx_pdc_frames: fx_pdc,
        }
    }

    pub(in crate::ui) fn toggle_input_monitor(&mut self) {
        let current = self.active_monitor_mode();
        if current == MonitorMode::Off {
            self.set_track_monitor_mode(None, MonitorMode::In);
        } else {
            self.set_track_monitor_mode(None, MonitorMode::Off);
        }
    }

    pub(in crate::ui) fn cycle_track_monitor(&mut self, track_index: Option<usize>) {
        let target = track_index.unwrap_or(self.cursor_row);
        let current = self
            .tracks
            .get(target)
            .map_or(MonitorMode::Off, |t| self.track_monitor_mode(t.id));
        self.set_track_monitor_mode(Some(target), current.next());
    }

    pub(in crate::ui) fn set_track_monitor_mode(
        &mut self,
        track_index: Option<usize>,
        mode: MonitorMode,
    ) {
        if mode == MonitorMode::Off {
            if self.input_monitor.take().is_some() {
                self.status_message = "🎧 Monitoreo de entrada desactivado".into();
            }
            return;
        }
        let Some(target) = self.resolve_audio_record_track(track_index) else {
            self.status_message = "Selecciona una pista de audio para monitorear".into();
            return;
        };
        let track_id = self.tracks[target].id;
        let track_name = self.tracks[target].name.to_string();
        self.sync_vst_effects();
        let rack = self.effect_rack(target);
        let fx_gain = self.own_track_fx_gain(target);
        let mix = effective_mix(&self.tracks, &self.track_levels, &self.track_pans, target);

        if let Some(active) = &mut self.input_monitor {
            active.track_id = track_id;
            active.mode = mode;
            let device_name = active.monitor.input_device_name.clone();
            self.sync_input_monitor();
            let mode_txt = match mode {
                MonitorMode::In => "IN (siempre activo con FX)",
                MonitorMode::Auto => "AUTO (activo en Stop / Grabación)",
                MonitorMode::Off => "OFF",
            };
            self.status_message =
                format!("🎧 Monitoreo {mode_txt} en «{track_name}» · {device_name}");
            return;
        }

        match InputMonitor::start(
            &self.settings.audio.saved,
            self.audio_record_stereo,
            rack,
            fx_gain,
            mix.gain,
            mix.pan,
        ) {
            Ok(monitor) => {
                let device_name = monitor.input_device_name.clone();
                let ch_label = if self.audio_record_stereo {
                    "Estéreo"
                } else {
                    "Mono"
                };
                self.input_monitor = Some(ActiveInputMonitor {
                    monitor,
                    track_id,
                    mode,
                });
                self.sync_input_monitor();
                let mode_txt = match mode {
                    MonitorMode::In => "IN (siempre activo con FX)",
                    MonitorMode::Auto => "AUTO (activo en Stop / Grabación)",
                    MonitorMode::Off => "OFF",
                };
                self.status_message = format!(
                    "🎧 Monitoreo {mode_txt} en «{track_name}» · {device_name} ({ch_label})"
                );
            }
            Err(error) => {
                self.status_message = format!("Monitoreo de entrada: {error}");
            }
        }
    }

    pub(in crate::ui) fn sync_input_monitor(&mut self) {
        let Some(active) = &self.input_monitor else {
            return;
        };
        if active.monitor.failed() {
            self.input_monitor = None;
            self.status_message = "La entrada de audio del monitoreo se desconectó".into();
            return;
        }
        let Some(index) = self.tracks.iter().position(|t| t.id == active.track_id) else {
            self.input_monitor = None;
            return;
        };
        let rack = self.effect_rack(index);
        let fx_gain = self.own_track_fx_gain(index);
        let mix = effective_mix(&self.tracks, &self.track_levels, &self.track_pans, index);
        let playing = self.player.is_some();
        let recording_this = self
            .audio_recorder
            .as_ref()
            .is_some_and(|r| r.track_id == active.track_id);
        let should_hear = mix.audible
            && match active.mode {
                MonitorMode::Off => false,
                MonitorMode::In => true,
                MonitorMode::Auto => !playing || recording_this,
            };
        active.monitor.set_fx(rack, fx_gain);
        active.monitor.set_mix(mix.gain, mix.pan);
        active.monitor.set_stereo(self.audio_record_stereo);
        active.monitor.set_active(should_hear);
        let bpm = self.timeline.tempo.bpm() as f32;
        for effect in self.lua_components.instances.values() {
            effect.set_bpm(bpm);
        }
    }

    fn recordings_dir(&self) -> PathBuf {
        if let Some(project_dir) = self.project.path.as_ref().and_then(|p| p.parent()) {
            return project_dir.join("recordings");
        }
        let mut dir = self.settings.export.saved.directory.clone();
        if let Ok(rest) = dir.strip_prefix("~")
            && let Some(home) = dirs::home_dir()
        {
            dir = home.join(rest);
        }
        dir.join("recordings")
    }

    fn resolve_audio_record_track(&mut self, requested: Option<usize>) -> Option<usize> {
        if self.tracks.is_empty() {
            self.edit_track_structure(elevate::musical::track::TrackEdit::Insert);
        }
        let candidate = requested.unwrap_or(self.cursor_row);
        if let Some(track) = self.tracks.get(candidate)
            && track.accepts_clips()
            && track.kind != TrackKind::Group
        {
            return Some(candidate);
        }
        self.tracks
            .iter()
            .position(|t| t.kind == TrackKind::Audio && t.accepts_clips())
            .or_else(|| {
                self.tracks
                    .iter()
                    .position(|t| t.accepts_clips() && t.kind != TrackKind::Group)
            })
    }

    pub(in crate::ui) fn toggle_audio_record(&mut self, track_index: Option<usize>) {
        if self.audio_recorder.is_some() {
            self.finish_audio_recording();
            if self.player.is_some() || self.file_playback.is_some() {
                self.stop_audio();
            }
            return;
        }
        if self.midi_capture.is_recording() {
            self.toggle_midi_record();
        }
        let Some(target) = self.resolve_audio_record_track(track_index) else {
            self.status_message = "Selecciona una pista de audio para grabar".into();
            return;
        };
        self.cursor_row = target;
        let track = &self.tracks[target];
        let track_id = track.id;
        let track_name = track.name.to_string();
        let safe_name: String = track_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let out_path = self
            .recordings_dir()
            .join(format!("Rec_{safe_name}_{stamp}.wav"));

        let bars = if self.player.is_none() && self.file_playback.is_none() {
            self.metronome.count_in_bars()
        } else {
            0
        };
        let start_beat = self
            .player
            .as_ref()
            .map_or_else(|| self.timeline.edit_beat(), |player| player.beat());
        let recorder = match AudioRecorder::start(
            &self.settings.audio.saved,
            out_path,
            self.audio_record_stereo,
            bars == 0,
        ) {
            Ok(rec) => rec,
            Err(error) => {
                self.status_message = format!("Micrófono / entrada: {error}");
                return;
            }
        };
        let mode_label = if recorder.stereo { "Estéreo" } else { "Mono" };
        let device_name = recorder.device_name.clone();
        self.audio_recorder = Some(ActiveAudioRecording {
            recorder,
            track_index: target,
            track_id,
            track_name: track_name.clone(),
            start_beat,
            waiting_count_in: bars > 0,
        });
        self.metronome.set_keep_running(true);
        if self.player.is_none() && self.file_playback.is_none() {
            self.publish_track_fx();
            self.stop_pattern();
            self.stop_instrument();
            self.request_file_playback_with_count_in(start_beat, bars);
        }
        self.status_message = if bars > 0 {
            format!("🎙 Pre-cuenta ({bars} comp.) en «{track_name}» · {device_name} ({mode_label})…")
        } else {
            format!(
                "🎙 Grabando audio en «{track_name}» · {device_name} ({mode_label}) · pulsa 🎙 o ■ para guardar"
            )
        };
    }

    pub(in crate::ui) fn on_count_in_finished_for_audio(&mut self, beat: f64) {
        if let Some(active) = &mut self.audio_recorder
            && active.waiting_count_in
        {
            active.waiting_count_in = false;
            active.start_beat = beat;
            active.recorder.set_capturing(true);
            let mode_label = if active.recorder.stereo {
                "Estéreo"
            } else {
                "Mono"
            };
            self.status_message = format!(
                "🎙 Grabando audio en «{}» ({mode_label}) · pulsa 🎙 o ■ para guardar",
                active.track_name
            );
        }
    }

    pub(in crate::ui) fn poll_audio_recording(&mut self) {
        self.sync_input_monitor();
        if let Some(active) = &self.audio_recorder
            && active.recorder.failed()
        {
            self.finish_audio_recording();
            self.status_message =
                "La entrada de audio se desconectó; toma guardada hasta el corte".into();
        }
    }

    pub(in crate::ui) fn finish_audio_recording(&mut self) {
        let Some(active) = self.audio_recorder.take() else {
            return;
        };
        self.metronome
            .set_keep_running(self.midi_capture.is_recording());
        self.metronome.clear_count_in();
        let take = match active.recorder.finish() {
            Ok(take) => take,
            Err(error) => {
                self.status_message = format!("Error al guardar grabación: {error}");
                return;
            }
        };
        if take.frames == 0 {
            self.status_message = "Grabación de audio detenida (sin audio capturado)".into();
            return;
        }
        let decoded = match AudioFile::decode(&take.path) {
            Ok(file) => Arc::new(file),
            Err(error) => {
                self.status_message = format!("WAV guardado pero no se pudo decodificar: {error}");
                return;
            }
        };
        let track_index = self
            .tracks
            .iter()
            .position(|t| t.id == active.track_id)
            .unwrap_or(active.track_index.min(self.tracks.len().saturating_sub(1)));
        let name = take
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Grabación")
            .to_string();
        let id = elevate::musical::clip::ClipId::fresh();
        let clip = AudioClip {
            processing: Default::default(),
            id,
            name: name.clone(),
            file: decoded,
            source_path_override: None,
            track: track_index,
            start_beat: active.start_beat,
            props: elevate::musical::clip::ClipProps::default(),
            trim_start: 0.0,
            trim_len: None,
        };
        self.timeline.insert_audio_clip(clip);
        self.timeline.selected_midi.clear();
        self.timeline.selected_audio = vec![id];
        self.timeline.focused_audio = Some(id);
        self.publish_clip_edit();
        self.refresh_project_dirty();
        let secs = take.frames as f64 / f64::from(take.sample_rate);
        let mode = if take.stereo { "Estéreo" } else { "Mono" };
        let drop_note = if take.dropped_frames > 0 {
            format!(" · {} frames perdidos", take.dropped_frames)
        } else {
            String::new()
        };
        self.status_message = format!(
            "🎙 Clip grabado en «{}»: {name} ({secs:.1}s · {} Hz · {mode}{drop_note})",
            active.track_name, take.sample_rate
        );
    }

    pub(in crate::ui) fn live_audio_record_preview(
        &self,
    ) -> Option<crate::ui::timeline::LiveRecordPreview> {
        let active = self.audio_recorder.as_ref()?;
        let track = self
            .tracks
            .iter()
            .position(|t| t.id == active.track_id)
            .unwrap_or(active.track_index);
        let secs = active.recorder.captured_seconds();
        let captured_beats = secs * (self.timeline.tempo.bpm() / 60.0);
        let current_beat = self
            .timeline
            .playhead
            .unwrap_or(active.start_beat + captured_beats)
            .max(active.start_beat + captured_beats);
        Some(crate::ui::timeline::LiveRecordPreview {
            track,
            start_beat: active.start_beat,
            end_beat: current_beat,
            peak: active.recorder.peak(),
            counting_in: active.waiting_count_in,
        })
    }
}
